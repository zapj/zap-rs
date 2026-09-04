//! 服务器运行环境（全局 env 状态表）管理。
//!
//! 状态表 server_env 分为两层：
//! - scope='auto' ：zapexec(root) 自动探测的快照（payload=整份 JSON），记录 detected 时间；
//! - scope='conf' ：管理员手写的全局默认配置（webserver / php_default / database 等），
//!   供后续建站默认 PHP、SSL 签发等流程读取。
//!
//! 端点（均需管理员）：
//! - GET  /system/env             读快照+默认配置；快照超 60s 自动刷新
//! - POST /system/env/refresh     强制重测并落库
//! - POST /system/env/defaults    保存全局默认配置

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::Json;
use axum::extract::Extension;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::db;
use crate::zap::ZapError;
use crate::zap::ZapJsonResult;
use crate::zap::audit;
use crate::zap::jwt::ValidatedClaims;
use crate::zap::jwt::is_admin;
use zap_proto::Request;

/// 快照超过该秒数后在 GET 时自动重测。
const SNAPSHOT_STALE_SECS: i64 = 60;

// ── 内部存取 ────────────────────────────────────────────────

async fn probe_payload() -> Result<Value, ZapError> {
    let resp = crate::zapexec::call(Request::EnvDetect).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    match resp.data {
        Some(v) => Ok(v),
        None => Err(ZapError::New(-1, "环境探测未返回数据".to_string())),
    }
}

/// 保存探测快照（单行 payload，updated_at 即检测时间）。
async fn save_snapshot(payload: &Value) -> i64 {
    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    let text = payload.to_string();
    let _ = sqlx::query(
        "INSERT INTO server_env (scope, k, v, remark, updated_at) VALUES ('auto', 'payload', ?, ?, ?)
         ON CONFLICT(scope, k) DO UPDATE SET v = excluded.v, updated_at = excluded.updated_at",
    )
    .bind(&text)
    .bind("运行环境自动探测快照")
    .bind(now)
    .execute(pool)
    .await;
    now
}

/// 读取快照 (payload, detected_at)。
async fn load_snapshot() -> (Option<Value>, i64) {
    let pool = db::get_db_pool().await;
    let row: Option<(String, i64)> = sqlx::query_as(
        "SELECT v, updated_at FROM server_env WHERE scope = 'auto' AND k = 'payload'",
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);
    match row {
        Some((text, ts)) => match serde_json::from_str::<Value>(&text) {
            Ok(v) => (Some(v), ts),
            Err(_) => (None, ts),
        },
        None => (None, 0),
    }
}

async fn load_conf() -> HashMap<String, String> {
    let pool = db::get_db_pool().await;
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT k, v FROM server_env WHERE scope = 'conf'")
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    rows.into_iter().collect()
}

/// 读取全局 conf 键值（其它模块用：虚拟主机运行模式 / fpm 默认规格）。
pub async fn conf_get(key: &str) -> Option<String> {
    let pool = db::get_db_pool().await;
    let row: Option<String> =
        sqlx::query_scalar("SELECT v FROM server_env WHERE scope = 'conf' AND k = ?")
            .bind(key)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    row.filter(|s| !s.trim().is_empty())
}

/// 面板默认 PHP-FPM pool 规格（JSON 对象；用户未自定义时的兜底）。
pub fn default_fpm_spec() -> serde_json::Map<String, Value> {
    [
        ("pm", "dynamic"),
        ("max_children", "10"),
        ("start_servers", "3"),
        ("min_spare_servers", "2"),
        ("max_spare_servers", "5"),
        ("max_requests", "1000"),
        ("request_terminate_timeout", "300"),
        ("memory_limit", "256M"),
        ("post_max_size", "128M"),
        ("upload_max_filesize", "128M"),
        ("max_execution_time", "300"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), Value::String(v.to_string())))
    .collect()
}

fn default_fpm_spec_json() -> String {
    serde_json::Value::Object(default_fpm_spec()).to_string()
}

/// 虚拟主机运行模式：www（统一 www 用户） / system（每面板用户一个 Linux 系统账号）。
pub async fn vhost_mode() -> String {
    match conf_get("vhost_mode").await.as_deref() {
        Some("system") => "system".to_string(),
        _ => "www".to_string(),
    }
}

fn conf_json(conf: &HashMap<String, String>) -> Value {
    json!({
        "webserver": conf.get("webserver").cloned().unwrap_or_default(),
        "php_default": conf.get("php_default").cloned().unwrap_or_default(),
        "database": conf.get("database").cloned().unwrap_or_default(),
        "vhost_mode": conf.get("vhost_mode").cloned().unwrap_or_else(|| "www".into()),
        "fpm_pool_defaults": conf.get("fpm_pool_defaults").cloned().unwrap_or_else(default_fpm_spec_json),
    })
}

/// 拼装 GET / refresh 的返回体。
async fn build_env_data(
    payload: Option<Value>,
    detected_at: i64,
    refreshed: bool,
    error: Option<String>,
) -> Value {
    let conf = load_conf().await;
    json!({
        "payload": payload.unwrap_or(Value::Null),
        "conf": conf_json(&conf),
        "detected_at": detected_at,
        "refreshed": refreshed,
        "error": error,
    })
}

// ── handlers ────────────────────────────────────────────────

/// GET /system/env：读快照；超时则自动重测。
pub async fn env_get(claims: ValidatedClaims) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可查看运行环境".to_string()));
    }
    let now = chrono::Local::now().timestamp();
    let (payload, detected_at) = load_snapshot().await;
    let stale = payload.is_none() || now - detected_at > SNAPSHOT_STALE_SECS;

    if stale {
        match probe_payload().await {
            Ok(v) => {
                let t = save_snapshot(&v).await;
                Ok(Json(
                    json!({ "code": 0, "data": build_env_data(Some(v), t, true, None).await }),
                ))
            }
            Err(e) => {
                let msg = e.to_string();
                Ok(Json(
                    json!({ "code": 0, "data": build_env_data(payload, detected_at, false, Some(msg)).await }),
                ))
            }
        }
    } else {
        Ok(Json(
            json!({ "code": 0, "data": build_env_data(payload, detected_at, false, None).await }),
        ))
    }
}

/// POST /system/env/refresh：强制探测并刷新快照。
pub async fn env_refresh(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可刷新运行环境".to_string()));
    }
    let payload = probe_payload().await?;
    let t = save_snapshot(&payload).await;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "env_refresh",
        "system",
        "手动刷新服务器运行环境快照",
    )
    .await;

    Ok(Json(
        json!({ "code": 0, "message": "运行环境已刷新", "data": build_env_data(Some(payload), t, true, None).await }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct EnvDefaultsPayload {
    /// 默认 Web 服务器 flavor：空=跟随探测（auto）
    pub webserver: Option<String>,
    /// 默认 PHP 版本（如 8.3 / php83），建站新增站点时的预选值
    pub php_default: Option<String>,
    /// 默认数据库实例（如 mysql / mariadb）
    pub database: Option<String>,
    /// 虚拟主机运行模式：www（统一 www 用户） / system（每用户 Linux 系统账号）
    pub vhost_mode: Option<String>,
    /// PHP-FPM 默认 pool 规格（JSON 字符串）
    pub fpm_pool_defaults: Option<String>,
}

/// POST /system/env/defaults：保存全局默认配置（admin only）。
pub async fn env_defaults_save(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
    Json(payload): Json<EnvDefaultsPayload>,
) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可设置默认运行环境".to_string()));
    }

    let mut upserts: Vec<(String, String)> = Vec::new();
    for (key, val) in [
        ("webserver", payload.webserver),
        ("php_default", payload.php_default),
        ("database", payload.database),
    ] {
        if let Some(v) = val {
            let v = v.trim().to_string();
            if v.len() > 64 {
                return Err(ZapError::New(-1, format!("默认{key}长度超限")));
            }
            upserts.push((key.to_string(), v));
        }
    }
    if let Some(v) = payload.vhost_mode {
        let v = v.trim().to_string();
        if v.is_empty() || !["www", "system"].contains(&v.as_str()) {
            return Err(ZapError::New(
                -1,
                "vhost_mode 仅支持 www / system".to_string(),
            ));
        }
        upserts.push(("vhost_mode".to_string(), v));
    }
    if let Some(v) = payload.fpm_pool_defaults {
        let v = v.trim().to_string();
        if !v.is_empty() && serde_json::from_str::<Value>(&v).map_or(true, |x| !x.is_object()) {
            return Err(ZapError::New(
                -1,
                "fpm_pool_defaults 必须是 JSON 对象".to_string(),
            ));
        }
        upserts.push(("fpm_pool_defaults".to_string(), v));
    }

    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    for (k, v) in &upserts {
        let _ = sqlx::query(
            "INSERT INTO server_env (scope, k, v, remark, updated_at) VALUES ('conf', ?, ?, '面板默认配置', ?)
             ON CONFLICT(scope, k) DO UPDATE SET v = excluded.v, updated_at = excluded.updated_at",
        )
        .bind(k)
        .bind(v)
        .bind(now)
        .execute(pool)
        .await;
    }

    let conf = load_conf().await;
    let detail = upserts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ");
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "env_defaults_save",
        "system",
        &detail,
    )
    .await;

    Ok(Json(
        json!({ "code": 0, "message": "默认配置已保存", "data": conf_json(&conf) }),
    ))
}
