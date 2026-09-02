//! AppStore 运行任务管理：DB 记录 + 日志监控 + 本地目录扫描。

use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::{config, db, zap::ZapError};

/// 日志结束标记：`__ZAP_DONE__ <exit_code>`（zapexec 写入）
pub const DONE_MARKER: &str = "__ZAP_DONE__";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AppstoreRun {
    pub id: i64,
    pub run_id: String,
    pub action: String,
    pub pkg: String,
    pub username: String,
    pub status: String,
    pub exit_code: i64,
    pub log_path: String,
    pub started_at: i64,
    pub finished_at: i64,
}

/// AppStore 根目录（与 zap.db 同级的 appstore/）
pub fn appstore_dir() -> PathBuf {
    let cfg = config::get_config().read().unwrap();
    let db_path = Path::new(&cfg.db.path);
    db_path
        .parent()
        .map(|p| p.join("appstore"))
        .unwrap_or_else(|| PathBuf::from("data/appstore"))
}

/// 已安装软件目录（apps/）
pub fn apps_dir() -> PathBuf {
    let cfg = config::get_config().read().unwrap();
    let db_path = Path::new(&cfg.db.path);
    db_path
        .parent()
        .map(|p| p.join("apps"))
        .unwrap_or_else(|| PathBuf::from("data/apps"))
}

pub fn logs_dir() -> PathBuf {
    appstore_dir().join("logs")
}

pub fn log_path_for(run_id: &str) -> String {
    logs_dir()
        .join(format!("run-{run_id}.log"))
        .to_string_lossy()
        .into_owned()
}

pub fn generate_run_id() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let rand_part: u64 = rand::random();
    format!("{millis:x}{rand_part:x}")
}

/// 登记一条运行记录（status=running）。
pub async fn register_run(
    run_id: &str,
    action: &str,
    pkg: &str,
    username: &str,
    log_path: &str,
) -> Result<(), ZapError> {
    let now = chrono::Utc::now().timestamp();
    let pool = db::get_db_pool().await;
    sqlx::query(
        "INSERT INTO appstore_runs (run_id, action, pkg, username, status, exit_code, log_path, started_at, finished_at) \
         VALUES (?, ?, ?, ?, 'running', -1, ?, ?, 0)",
    )
    .bind(run_id)
    .bind(action)
    .bind(pkg)
    .bind(username)
    .bind(log_path)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn finish_run(run_id: &str, status: &str, exit_code: i64) {
    let now = chrono::Utc::now().timestamp();
    let pool = db::get_db_pool().await;
    let _ = sqlx::query(
        "UPDATE appstore_runs SET status = ?, exit_code = ?, finished_at = ? WHERE run_id = ?",
    )
    .bind(status)
    .bind(exit_code)
    .bind(now)
    .bind(run_id)
    .execute(pool)
    .await;
}

pub async fn get_run(run_id: &str) -> Result<Option<AppstoreRun>, sqlx::Error> {
    let pool = db::get_db_pool().await;
    sqlx::query_as::<_, AppstoreRun>("SELECT * FROM appstore_runs WHERE run_id = ?")
        .bind(run_id)
        .fetch_optional(pool)
        .await
}

pub async fn list_runs(page: i64, page_size: i64) -> Result<(Vec<AppstoreRun>, i64), sqlx::Error> {
    let pool = db::get_db_pool().await;
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);
    let offset = (page - 1) * page_size;
    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM appstore_runs")
        .fetch_one(pool)
        .await?;
    let rows = sqlx::query_as::<_, AppstoreRun>(
        "SELECT * FROM appstore_runs ORDER BY id DESC LIMIT ? OFFSET ?",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok((rows, total))
}

/// 后台监控日志直到出现 `__ZAP_DONE__ <code>`，随后更新运行状态。
pub fn watch_log(run_id: String, log_path: String) {
    tokio::spawn(async move {
        let interval = std::time::Duration::from_millis(500);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(24 * 3600);
        loop {
            match read_done_marker(&log_path).await {
                Some(code) => {
                    let status = if code == 0 { "success" } else { "failed" };
                    finish_run(&run_id, status, code).await;
                    break;
                }
                None => {
                    if std::time::Instant::now() > deadline {
                        finish_run(&run_id, "failed", -2).await;
                        break;
                    }
                    tokio::time::sleep(interval).await;
                }
            }
        }
    });
}

/// 从日志末尾探测完成标记，返回退出码。
async fn read_done_marker(log_path: &str) -> Option<i64> {
    let content = tokio::fs::read_to_string(log_path).await.ok()?;
    let tail = content.rsplit(DONE_MARKER).next()?.trim();
    let code: i64 = tail.split_whitespace().next()?.parse().ok()?;
    Some(code)
}

/// 读取日志 offset 之后的内容，同时返回是否已完成。
pub async fn read_log(
    log_path: &str,
    offset: u64,
) -> Result<(String, Option<i64>, bool), ZapError> {
    let content = match tokio::fs::read_to_string(log_path).await {
        Ok(c) => c,
        // 日志文件尚未生成（后台任务刚启动的竞态窗口），视为空日志，
        // 由调用方（WebSocket / HTTP 轮询）继续等待而非直接失败。
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let bytes = content.as_bytes();
    let start = (offset as usize).min(bytes.len());
    let text = String::from_utf8_lossy(&bytes[start..]).to_string();
    let done = read_done_marker(log_path).await;
    Ok((text, done, done.is_some()))
}

/// 去掉日志尾部完成标记，供最终展示。
pub fn strip_done_marker(content: &str) -> String {
    match content.rfind(DONE_MARKER) {
        Some(idx) => content[..idx].trim_end().to_string(),
        None => content.to_string(),
    }
}

// ── 本地目录扫描（包列表 / 已安装列表）──────────────────────

#[derive(Debug, Default, serde::Deserialize)]
struct AppYaml {
    name: Option<String>,
    version: Option<String>,
    category: Option<String>,
    title: Option<String>,
    description: Option<String>,
    deps: Option<Vec<String>>,
    default_port: Option<u16>,
    #[serde(default)]
    scripts: Option<Value>,
}

/// 扫描全部 Git 源 + 自定义包。同名覆盖顺序（优先级从低到高）：
/// 内置源 < 后添加的源 < custom。
pub async fn scan_packages() -> Vec<Value> {
    let repos_root = appstore_dir().join("repos");
    let custom_dir = appstore_dir().join("custom");
    let repo_list = read_repos_value().await.unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        let mut by_path: std::collections::BTreeMap<String, Value> =
            std::collections::BTreeMap::new();
        // 先扫各 Git 源（按 repos.yaml 顺序，内置源排最前 → 优先级最低）
        if let Some(repos) = repo_list.get("repos").and_then(|r| r.as_array()) {
            for repo in repos {
                let id = repo.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let dir = repos_root.join(id);
                scan_source_dir(&dir, "official", Some(id), &mut by_path);
            }
        }
        // custom 最后扫描，覆盖同名官方包
        scan_source_dir(&custom_dir, "custom", None, &mut by_path);
        by_path.into_values().collect()
    })
    .await
    .unwrap_or_default()
}

fn scan_source_dir(
    dir: &Path,
    source: &str,
    repo_id: Option<&str>,
    by_path: &mut std::collections::BTreeMap<String, Value>,
) {
    for category in ["database", "application", "webserver", "library"] {
        let cat_dir = dir.join(category);
        let Ok(entries) = std::fs::read_dir(&cat_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let pkg_dir = entry.path();
            if !pkg_dir.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let app_yaml = match parse_app_yaml(&pkg_dir.join("app.yaml")) {
                Some(a) => a,
                None => continue,
            };
            let pkg_path = format!("{category}/{name}");
            by_path.insert(
                pkg_path.clone(),
                json!({
                    "pkg_path": pkg_path,
                    "category": app_yaml.category.clone().unwrap_or_else(|| category.to_string()),
                    "name": app_yaml.name.clone().unwrap_or(name),
                    "title": app_yaml.title.clone().unwrap_or_default(),
                    "description": app_yaml.description.clone().unwrap_or_default(),
                    "version": app_yaml.version.clone().unwrap_or_default(),
                    "deps": app_yaml.deps.clone().unwrap_or_default(),
                    "default_port": app_yaml.default_port,
                    "scripts": app_yaml.scripts.clone().unwrap_or(Value::Null),
                    "source": source,
                    "repo_id": repo_id,
                }),
            );
        }
    }
}

/// 扫描已安装包（apps/ 下 meta.yaml）。
pub async fn scan_installed() -> Vec<Value> {
    tokio::task::spawn_blocking(|| {
        let mut items = Vec::new();
        let Ok(entries) = std::fs::read_dir(apps_dir()) else {
            return items;
        };
        for entry in entries.flatten() {
            let app_path = entry.path();
            if !app_path.is_dir() {
                continue;
            }
            let Some(meta) = parse_meta_yaml(&app_path.join("meta.yaml")) else {
                continue;
            };
            items.push(meta);
        }
        items.sort_by(|a, b| {
            a.get("pkg_path")
                .and_then(|x| x.as_str())
                .cmp(&b.get("pkg_path").and_then(|x| x.as_str()))
        });
        items
    })
    .await
    .unwrap_or_default()
}

/// 读取某个已安装包的版本（升级时获取 old_version）。
pub async fn installed_version_of(pkg_path: &str) -> Option<String> {
    let apps = apps_dir();
    let pkg_path = pkg_path.to_string();
    tokio::task::spawn_blocking(move || {
        let p = apps.join(&pkg_path).join("meta.yaml");
        parse_meta_yaml(&p).and_then(|m| {
            m.get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
    })
    .await
    .unwrap_or(None)
}

/// 读取 repos.yaml 并返回 Value；不存在时兜底返回内置源（不落盘，写盘由 zapexec 负责）。
pub async fn read_repos_value() -> Option<Value> {
    let path = appstore_dir().join("repos.yaml");
    tokio::task::spawn_blocking(move || {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_yaml::from_str::<Value>(&content) {
                return Some(v);
            }
        }
        Some(json!({
            "repos": [{
                "id": "zap-appstore",
                "name": "Zap 官方应用商店",
                "url": "https://github.com/zapj/zap-appstore.git",
                "builtin": true,
                "enabled": true,
                "version": "",
                "commit": "",
                "updated_at": 0,
            }]
        }))
    })
    .await
    .unwrap_or(None)
}

/// 读取 Git 源列表，附加每个源目录是否存在的信息，供前端展示。
pub async fn list_repos() -> Vec<Value> {
    let repos_root = appstore_dir().join("repos");
    let value = read_repos_value().await;
    tokio::task::spawn_blocking(move || {
        let mut items = Vec::new();
        if let Some(v) = value {
            if let Some(repos) = v.get("repos").and_then(|r| r.as_array()) {
                for repo in repos {
                    let id = repo.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                    items.push(json!({
                        "id": id,
                        "name": repo.get("name").and_then(|v| v.as_str()).unwrap_or_default(),
                        "url": repo.get("url").and_then(|v| v.as_str()).unwrap_or_default(),
                        "builtin": repo.get("builtin").and_then(|v| v.as_bool()).unwrap_or(false),
                        "enabled": repo.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        "version": repo.get("version").and_then(|v| v.as_str()).unwrap_or_default(),
                        "commit": repo.get("commit").and_then(|v| v.as_str()).unwrap_or_default(),
                        "updated_at": repo.get("updated_at").and_then(|v| v.as_i64()).unwrap_or(0),
                        "exists": repos_root.join(id).is_dir(),
                    }));
                }
            }
        }
        items
    })
    .await
    .unwrap_or_default()
}

fn parse_app_yaml(path: &Path) -> Option<AppYaml> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_yaml::from_str(&content).ok()
}

fn parse_meta_yaml(path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_yaml::from_str(&content).ok()?;
    // meta.yaml 位于 apps/{category}/{name}/meta.yaml，pkg_path 取其父目录的相对路径
    let pkg_path = path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|cat_dir| cat_dir.file_name())
        .map(|cat| {
            let name = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            format!("{}/{}", cat.to_string_lossy(), name)
        })
        .unwrap_or_default();
    Some(json!({
        "pkg_path": pkg_path,
        "name": v.get("name"),
        "version": v.get("version"),
        "category": v.get("category"),
        "source": v.get("source"),
        "installed_at": v.get("installed_at"),
        "upgraded_from": v.get("upgraded_from"),
        "run_id": v.get("run_id"),
    }))
}
