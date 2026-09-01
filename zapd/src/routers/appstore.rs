//! AppStore 路由：仓库管理 / 包安装卸载升级 / 脚本管理 / 运行记录与实时日志。

use std::net::SocketAddr;

use axum::{
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        Extension, Path, Query,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures_util::SinkExt;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{error, info};

use crate::{
    config,
    zap::{
        appstore as ast,
        audit,
        jwt::{self, Claims, ValidatedClaims},
        ZapError, ZapJsonResult,
    },
    zapexec,
};
use zap_proto::Request;

fn require_admin(claims: &Claims) -> Result<(), ZapError> {
    if jwt::is_admin(claims) {
        Ok(())
    } else {
        Err(ZapError::New(-1, "权限不足，需要管理员权限".to_string()))
    }
}

/// 校验脚本路径：必须位于 custom/scripts/ 下；非管理员只能操作自己的目录。
fn validate_script_path(claims: &Claims, path: &str) -> Result<(), ZapError> {
    if !path.starts_with("scripts/") {
        return Err(ZapError::New(-1, "只允许操作 scripts/ 下的脚本".to_string()));
    }
    if !jwt::is_admin(claims) {
        let prefix = format!("scripts/{}/", claims.sub);
        if !path.starts_with(&prefix) {
            return Err(ZapError::New(-1, "只能操作自己的脚本".to_string()));
        }
    }
    Ok(())
}

// ── 仓库信息 / 更新 ─────────────────────────────────────────

pub async fn repo_info(_claims: ValidatedClaims) -> ZapJsonResult {
    let repo = ast::read_repo_yaml_value().await;
    let data = match repo {
        Some(v) => json!({
            "exists": true,
            "source_type": v.get("source_type").and_then(|x| x.as_str()).unwrap_or("git"),
            "source_url": v.get("source_url").and_then(|x| x.as_str()).unwrap_or(""),
            "version": v.get("version").and_then(|x| x.as_str()).unwrap_or(""),
            "updated_at": v.get("updated_at").and_then(|x| x.as_i64()).unwrap_or(0),
            "commit": v.get("commit").and_then(|x| x.as_str()).unwrap_or(""),
        }),
        None => json!({
            "exists": false,
            "source_type": "git",
            "source_url": "",
            "version": "",
            "updated_at": 0,
            "commit": "",
        }),
    };
    Ok(Json(json!({ "code": 0, "message": "OK", "data": data })))
}

#[derive(Debug, Deserialize)]
pub struct RepoUpdatePayload {
    pub source_type: Option<String>,
    pub source_url: Option<String>,
    pub sha256: Option<String>,
}

pub async fn repo_update(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<RepoUpdatePayload>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let repo = ast::read_repo_yaml_value().await;
    let (source_type, source_url) = match repo {
        Some(v) => (
            payload
                .source_type
                .unwrap_or_else(|| v.get("source_type").and_then(|x| x.as_str()).unwrap_or("git").to_string()),
            payload.source_url.unwrap_or_else(|| {
                v.get("source_url").and_then(|x| x.as_str()).unwrap_or_default().to_string()
            }),
        ),
        None => (
            payload.source_type.unwrap_or_else(|| "git".to_string()),
            payload.source_url.unwrap_or_default(),
        ),
    };
    if source_url.is_empty() {
        return Err(ZapError::New(-1, "缺少软件库地址 source_url".to_string()));
    }

    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, "repo_update", &source_url, &claims.sub, &log_path).await?;

    let resp = zapexec::call(Request::AppstoreRepoUpdate {
        source_type: source_type.clone(),
        source_url: source_url.clone(),
        sha256: payload.sha256,
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_repo_update",
        &source_type,
        &source_url,
    )
    .await;
    info!("AppStore repo update started: {source_url} ({source_type})");
    Ok(Json(json!({
        "code": 0,
        "message": "软件库更新已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

// ── 包列表 / 安装 / 卸载 / 升级 ─────────────────────────────

pub async fn packages(_claims: ValidatedClaims) -> ZapJsonResult {
    let pkgs = ast::scan_packages().await;
    let installed = ast::scan_installed().await;
    let mut installed_map = std::collections::HashMap::new();
    for inst in &installed {
        if let Some(p) = inst.get("pkg_path").and_then(|x| x.as_str()) {
            installed_map.insert(p.to_string(), inst.clone());
        }
    }
    let mut items: Vec<Value> = Vec::new();
    for mut pkg in pkgs {
        let pkg_path = pkg.get("pkg_path").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        if let Some(inst) = installed_map.get(&pkg_path) {
            pkg["installed"] = json!(true);
            pkg["installed_version"] = inst.get("version").cloned().unwrap_or(Value::Null);
            pkg["installed_source"] = inst.get("source").cloned().unwrap_or(Value::Null);
            pkg["installed_at"] = inst.get("installed_at").cloned().unwrap_or(Value::Null);
            pkg["upgraded_from"] = inst.get("upgraded_from").cloned().unwrap_or(Value::Null);
        } else {
            pkg["installed"] = json!(false);
        }
        items.push(pkg);
    }
    items.sort_by(|a, b| {
        a.get("category")
            .and_then(|x| x.as_str())
            .cmp(&b.get("category").and_then(|x| x.as_str()))
            .then_with(|| a.get("name").and_then(|x| x.as_str()).cmp(&b.get("name").and_then(|x| x.as_str())))
    });
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": { "packages": items, "installed": installed }
    })))
}

#[derive(Debug, Deserialize)]
pub struct InstallPayload {
    pub pkg_path: String,
    pub source: String,
    pub version: String,
}

pub async fn install(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<InstallPayload>,
) -> ZapJsonResult {
    // 自定义包包含任意脚本，仅管理员可安装
    if payload.source == "custom" {
        require_admin(&claims)?;
    }
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, "install", &payload.pkg_path, &claims.sub, &log_path).await?;

    let resp = zapexec::call(Request::AppstoreInstall {
        pkg_path: payload.pkg_path.clone(),
        source: payload.source.clone(),
        version: payload.version.clone(),
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_install",
        &payload.pkg_path,
        &format!("source={} version={}", payload.source, payload.version),
    )
    .await;
    info!("AppStore install started: {} ({})", payload.pkg_path, payload.source);
    Ok(Json(json!({
        "code": 0,
        "message": "安装已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

#[derive(Debug, Deserialize)]
pub struct UninstallPayload {
    pub pkg_path: String,
}

pub async fn uninstall(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<UninstallPayload>,
) -> ZapJsonResult {
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, "uninstall", &payload.pkg_path, &claims.sub, &log_path).await?;

    let resp = zapexec::call(Request::AppstoreUninstall {
        pkg_path: payload.pkg_path.clone(),
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_uninstall",
        &payload.pkg_path,
        "",
    )
    .await;
    info!("AppStore uninstall started: {}", payload.pkg_path);
    Ok(Json(json!({
        "code": 0,
        "message": "卸载已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpgradePayload {
    pub pkg_path: String,
    pub source: String,
    pub version: String,
}

pub async fn upgrade(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<UpgradePayload>,
) -> ZapJsonResult {
    let old_version = ast::installed_version_of(&payload.pkg_path)
        .await
        .unwrap_or_default();
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, "upgrade", &payload.pkg_path, &claims.sub, &log_path).await?;

    let resp = zapexec::call(Request::AppstoreUpgrade {
        pkg_path: payload.pkg_path.clone(),
        source: payload.source.clone(),
        version: payload.version.clone(),
        old_version,
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_upgrade",
        &payload.pkg_path,
        &format!("source={} version={}", payload.source, payload.version),
    )
    .await;
    info!("AppStore upgrade started: {}", payload.pkg_path);
    Ok(Json(json!({
        "code": 0,
        "message": "升级已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

// ── 脚本管理 ────────────────────────────────────────────────

/// 递归构建自定义脚本树（路径相对 custom/）。
fn build_script_tree(dir: &std::path::Path, rel_base: &std::path::Path) -> Value {
    let name = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(entry);
            } else if path.is_file() {
                files.push(entry);
            }
        }
    }
    dirs.sort_by_key(|e| e.file_name());
    files.sort_by_key(|e| e.file_name());
    let mut children: Vec<Value> = Vec::new();
    for entry in dirs {
        children.push(build_script_tree(&entry.path(), rel_base));
    }
    for entry in files {
        let path = entry.path();
        let rel = path
            .strip_prefix(rel_base)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        children.push(json!({
            "type": "file",
            "name": entry.file_name().to_string_lossy(),
            "path": rel,
        }));
    }
    json!({
        "type": "dir",
        "name": name,
        "path": dir.strip_prefix(rel_base).map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
        "children": children,
    })
}

pub async fn scripts_tree(claims: ValidatedClaims) -> ZapJsonResult {
    // 路径统一相对 custom/（如 scripts/admin/backup.sh），与 script_read/write 契约一致
    let base = ast::appstore_dir().join("custom");
    let root = if jwt::is_admin(&claims) {
        base.join("scripts")
    } else {
        base.join("scripts").join(&claims.sub)
    };
    let tree = tokio::task::spawn_blocking(move || {
        if root.is_dir() {
            build_script_tree(&root, &base)
        } else {
            Value::Null
        }
    })
    .await
    .unwrap_or(Value::Null);
    Ok(Json(json!({ "code": 0, "message": "OK", "data": { "tree": tree } })))
}

#[derive(Debug, Deserialize)]
pub struct ScriptPathQuery {
    pub path: String,
}

pub async fn script_read(claims: ValidatedClaims, Query(q): Query<ScriptPathQuery>) -> ZapJsonResult {
    validate_script_path(&claims, &q.path)?;
    let resp = zapexec::call(Request::AppstoreScriptRead { path: q.path.clone() }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "OK", "data": resp.data })))
}

#[derive(Debug, Deserialize)]
pub struct ScriptWritePayload {
    pub path: String,
    pub content: String,
}

pub async fn script_write(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<ScriptWritePayload>,
) -> ZapJsonResult {
    validate_script_path(&claims, &payload.path)?;
    let resp = zapexec::call(Request::AppstoreScriptWrite {
        path: payload.path.clone(),
        content: payload.content.clone(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_script_write",
        &payload.path,
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "保存成功", "data": resp.data })))
}

#[derive(Debug, Deserialize)]
pub struct ScriptRunPayload {
    pub path: String,
}

pub async fn script_run(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<ScriptRunPayload>,
) -> ZapJsonResult {
    validate_script_path(&claims, &payload.path)?;
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, "script", &payload.path, &claims.sub, &log_path).await?;

    let resp = zapexec::call(Request::AppstoreScriptRun {
        path: payload.path.clone(),
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "appstore_script_run",
        &payload.path,
        "",
    )
    .await;
    info!("AppStore script run started: {}", payload.path);
    Ok(Json(json!({
        "code": 0,
        "message": "脚本已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

#[derive(Debug, Deserialize)]
pub struct ScriptStopPayload {
    pub run_id: String,
}

pub async fn script_stop(
    claims: ValidatedClaims,
    Json(payload): Json<ScriptStopPayload>,
) -> ZapJsonResult {
    // 非管理员只能停止自己发起的任务
    if !jwt::is_admin(&claims) {
        if let Ok(Some(run)) = ast::get_run(&payload.run_id).await {
            if run.username != claims.sub {
                return Err(ZapError::New(-1, "只能停止自己发起的任务".to_string()));
            }
        }
    }
    let resp = zapexec::call(Request::AppstoreScriptStop { run_id: payload.run_id.clone() }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "已发送停止信号" })))
}

// ── 运行记录 / 日志 ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

pub async fn runs(_claims: ValidatedClaims, Query(q): Query<RunsQuery>) -> ZapJsonResult {
    let (rows, total) = ast::list_runs(q.page.unwrap_or(1), q.page_size.unwrap_or(20)).await?;
    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "run_id": r.run_id,
                "action": r.action,
                "pkg": r.pkg,
                "username": r.username,
                "status": r.status,
                "exit_code": r.exit_code,
                "log_path": r.log_path,
                "started_at": r.started_at,
                "finished_at": r.finished_at,
            })
        })
        .collect();
    Ok(Json(json!({ "code": 0, "message": "OK", "data": { "items": items, "total": total } })))
}

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub offset: Option<u64>,
}

pub async fn log(
    _claims: ValidatedClaims,
    Path(run_id): Path<String>,
    Query(q): Query<LogQuery>,
) -> ZapJsonResult {
    let run = ast::get_run(&run_id).await?.ok_or_else(|| ZapError::New(-1, "任务不存在".to_string()))?;
    let (content, exit_code, done) = ast::read_log(&run.log_path, q.offset.unwrap_or(0)).await?;
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": { "content": content, "exit_code": exit_code, "done": done }
    })))
}

// ── WebSocket 实时日志 ──────────────────────────────────────

pub async fn ws_log(
    ws: WebSocketUpgrade,
    Path(run_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let token = match params.get("token") {
        Some(t) => t.clone(),
        None => {
            return axum::response::Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(axum::body::Body::from("Missing token"))
                .unwrap();
        }
    };
    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    if decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secure_key.as_ref()),
        &Validation::default(),
    )
    .is_err()
    {
        return axum::response::Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(axum::body::Body::from("Invalid token"))
            .unwrap();
    }
    ws.on_upgrade(move |socket| handle_ws_log(socket, run_id))
}

async fn handle_ws_log(mut socket: WebSocket, run_id: String) {
    info!("AppStore log WebSocket connected: {run_id}");
    let Some(run) = ast::get_run(&run_id).await.unwrap_or(None) else {
        let _ = socket
            .send(Message::Text(Utf8Bytes::from(
                json!({ "type": "error", "message": "任务不存在" }).to_string(),
            )))
            .await;
        return;
    };
    let log_path = run.log_path;
    let mut offset: u64 = 0;

    loop {
        match ast::read_log(&log_path, offset).await {
            Ok((text, exit_code, done)) => {
                if !text.is_empty() {
                    // 去掉完成标记行，避免重复展示
                    let clean = if done { ast::strip_done_marker(&text) } else { text.clone() };
                    if !clean.is_empty() {
                        if socket
                            .send(Message::Text(Utf8Bytes::from(
                                json!({ "type": "log", "data": clean }).to_string(),
                            )))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    offset += text.len() as u64;
                }
                if done {
                    let status = if exit_code == Some(0) { "success" } else { "failed" };
                    let _ = socket
                        .send(Message::Text(Utf8Bytes::from(
                            json!({ "type": "done", "status": status, "exit_code": exit_code })
                                .to_string(),
                        )))
                        .await;
                    let _ = socket.close().await;
                    return;
                }
            }
            Err(e) => {
                error!("read appstore log {run_id} failed: {e}");
                let _ = socket.close().await;
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
}
