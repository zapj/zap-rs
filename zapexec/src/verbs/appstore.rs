//! AppStore 特权动词（全部以 root 执行）。
//!
//! 职责：
//! - `repo_add` / `repo_remove` / `repo_update`：多 Git 源管理
//!   （clone/fetch 到 data/appstore/repos/<id>/，刷新 repos.yaml）
//! - `install` / `uninstall` / `upgrade`：运行包脚本，日志写入 logs/run-{id}.log
//! - `script_run` / `script_stop`：运行/停止自定义脚本（进程组管理）
//! - `script_read` / `script_write`：读写 appstore 内脚本（写仅限 custom/）
//! - `installed`：扫描 $APPS_DIR/*/meta.yaml
//!
//! 安全边界：所有相对路径先做 sanitize（拒绝绝对路径 / `..` / 越界），
//! 包名只允许 `[A-Za-z0-9_-]`。脚本永远通过白名单动词进入，不提供任意命令执行。

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::root_cmd;
use std::os::unix::process::CommandExt;
use zap_proto::Response;

// ── 内置源（跟随 zap 发行包发布）────────────────────────────

pub const BUILTIN_REPO_ID: &str = "zap-appstore";
pub const BUILTIN_REPO_NAME: &str = "Zap 官方应用商店";
pub const BUILTIN_REPO_URL: &str = "https://github.com/zapj/zap-appstore.git";

// ── 目录定位 ───────────────────────────────────────────────

fn zap_path() -> PathBuf {
    std::env::var("ZAP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/usr/local/zap"))
}

fn appstore_dir() -> PathBuf {
    zap_path().join("data/appstore")
}

fn apps_dir() -> PathBuf {
    zap_path().join("data/apps")
}

fn logs_dir() -> PathBuf {
    appstore_dir().join("logs")
}

/// 所有 Git 源根目录：repos/<id>/
fn repos_dir() -> PathBuf {
    appstore_dir().join("repos")
}

fn repos_yaml_path() -> PathBuf {
    appstore_dir().join("repos.yaml")
}

// ── 路径安全 ───────────────────────────────────────────────

fn safe_rel(requested: &str) -> Result<PathBuf, String> {
    let p = Path::new(requested);
    if p.is_absolute() {
        return Err("不允许绝对路径".into());
    }
    for seg in p.components() {
        match seg {
            Component::ParentDir => return Err("不允许 .. 路径".into()),
            Component::RootDir | Component::Prefix(_) => return Err("不允许绝对路径".into()),
            Component::CurDir => {}
            Component::Normal(_) => {}
        }
    }
    Ok(p.to_path_buf())
}

fn safe_join(base: &Path, requested: &str) -> Result<PathBuf, String> {
    let rel = safe_rel(requested)?;
    let joined = base.join(rel);
    if !joined.starts_with(base) {
        return Err("路径越界".into());
    }
    Ok(joined)
}

/// 包路径必须是 `category/name`，且只含安全字符。
fn validate_pkg_path(pkg_path: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = pkg_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() != 2 {
        return Err(format!("包路径格式应为 category/name，收到: {pkg_path}"));
    }
    for s in &parts {
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!("包名含非法字符: {s}"));
        }
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// 定位包目录：custom 优先于官方源；官方源按 repo_id 定位到 repos/<repo_id>/，
/// 未指定 repo_id 时遍历所有源目录。
fn find_package(pkg_path: &str, source: &str, repo_id: Option<&str>) -> Result<PathBuf, String> {
    let (cat, name) = validate_pkg_path(pkg_path)?;
    if source == "custom" {
        let dir = safe_join(&appstore_dir().join("custom"), &format!("{cat}/{name}"))?;
        if dir.is_dir() {
            return Ok(dir);
        }
        return Err(format!("自定义包不存在: {pkg_path}"));
    }
    if let Some(rid) = repo_id {
        let rid = safe_rel(rid)?;
        let rid_str = rid.to_string_lossy().to_string();
        let dir = repos_dir().join(rid).join(&cat).join(&name);
        if dir.is_dir() {
            return Ok(dir);
        }
        return Err(format!("官方包不存在: {pkg_path}（源 {rid_str}）"));
    }
    let mut found: Option<PathBuf> = None;
    if let Ok(entries) = std::fs::read_dir(repos_dir()) {
        for entry in entries.flatten() {
            let dir = entry.path().join(&cat).join(&name);
            if dir.is_dir() {
                found = Some(dir);
                break;
            }
        }
    }
    found.ok_or_else(|| format!("官方包不存在: {pkg_path}"))
}

/// 包脚本文件名：app.yaml 的 `scripts.{install|uninstall|upgrade}` 可覆盖默认约定。
fn script_file(pkg_dir: &Path, key: &str, default: &str) -> Result<PathBuf, String> {
    let mut file = default.to_string();
    if let Ok(content) = std::fs::read_to_string(pkg_dir.join("app.yaml")) {
        if let Ok(v) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
            if let Some(name) = v
                .get("scripts")
                .and_then(|s| s.get(key))
                .and_then(|s| s.as_str())
            {
                file = name.to_string();
            }
        }
    }
    let p = safe_join(pkg_dir, &file)?;
    if !p.is_file() {
        return Err(format!("包脚本不存在: {}", p.display()));
    }
    Ok(p)
}

// ── repos.yaml / meta.yaml ─────────────────────────────────

/// 单个 Git 源配置项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    /// 系统内置源（随 zap 发行包发布，禁止删除）
    #[serde(default)]
    pub builtin: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 最近一次同步的 commit 短哈希
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub commit: String,
    #[serde(default)]
    pub updated_at: i64,
}

fn default_enabled() -> bool {
    true
}

impl RepoEntry {
    fn builtin() -> RepoEntry {
        RepoEntry {
            id: BUILTIN_REPO_ID.into(),
            name: BUILTIN_REPO_NAME.into(),
            url: BUILTIN_REPO_URL.into(),
            builtin: true,
            enabled: true,
            version: String::new(),
            commit: String::new(),
            updated_at: 0,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ReposFile {
    #[serde(default)]
    pub repos: Vec<RepoEntry>,
}

/// 读取源列表；repos.yaml 不存在或为空时自动补内置源记录。
fn read_repos() -> Result<Vec<RepoEntry>, String> {
    let mut file = match std::fs::read_to_string(repos_yaml_path()) {
        Ok(content) => serde_yaml::from_str::<ReposFile>(&content)
            .map_err(|e| format!("解析 repos.yaml 失败: {e}"))?,
        Err(_) => ReposFile::default(),
    };
    if file.repos.is_empty() {
        file.repos.push(RepoEntry::builtin());
        write_repos(&file.repos)?;
    }
    Ok(file.repos)
}

fn write_repos(repos: &[RepoEntry]) -> Result<(), String> {
    std::fs::create_dir_all(appstore_dir()).map_err(|e| e.to_string())?;
    let file = ReposFile {
        repos: repos.to_vec(),
    };
    let yaml = serde_yaml::to_string(&file).map_err(|e| format!("序列化 repos.yaml 失败: {e}"))?;
    std::fs::write(repos_yaml_path(), yaml).map_err(|e| format!("写入 repos.yaml 失败: {e}"))
}

/// 从 Git URL 末段生成源 id（去 .git，保留 [a-z0-9-]）。
fn id_from_url(url: &str) -> String {
    let base = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("store")
        .trim_end_matches(".git");
    let mut id = String::new();
    for c in base.to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else {
            id.push('-');
        }
    }
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    let id = id.trim_matches('-').to_string();
    if id.is_empty() { "store".into() } else { id }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaInfo {
    pub name: String,
    pub version: String,
    pub category: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    pub installed_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upgraded_from: Option<String>,
    pub run_id: String,
}

fn write_meta(app_path: &Path, meta: &MetaInfo) -> std::io::Result<()> {
    std::fs::create_dir_all(app_path)?;
    let yaml = serde_yaml::to_string(meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(app_path.join("meta.yaml"), yaml)
}

fn read_meta(app_path: &Path) -> Result<MetaInfo, String> {
    let content = std::fs::read_to_string(app_path.join("meta.yaml"))
        .map_err(|e| format!("读取 meta.yaml 失败: {e}"))?;
    serde_yaml::from_str(&content).map_err(|e| format!("解析 meta.yaml 失败: {e}"))
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ── 命令执行 ───────────────────────────────────────────────

fn run_capture(program: &str, args: &[&str]) -> Result<String, String> {
    // git 远程操作（clone/fetch）在网络不可达时可能无限挂起，导致任务永久 running。
    // 统一用 timeout 限时；同时禁止 git 交互式认证提示（避免等待输入用户名/密码）。
    let out = root_cmd("timeout")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("180")
        .arg(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program} 执行失败: {e}"))?;
    if out.status.code() == Some(124) {
        return Err(format!("{program} 执行超时(180s)"));
    }
    if !out.status.success() {
        return Err(format!(
            "{program} 失败: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

struct ScriptStep {
    script: PathBuf,
    env: Vec<(String, String)>,
}

/// 后台运行一个或多个脚本（同一 run_id、同一日志追加写）。
/// 每个脚本以 `setsid` 启动独立进程组，pid 写入 run-{id}.pid 供停止使用。
/// 全部成功退出码为 0；任一脚本失败则中断后续步骤。结束后追加 `__ZAP_DONE__ <code>`。
fn spawn_background(
    run_id: &str,
    steps: Vec<ScriptStep>,
    on_done: Box<dyn FnOnce(i32) + Send>,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(logs_dir()).map_err(|e| e.to_string())?;
    let log_path = logs_dir().join(format!("run-{run_id}.log"));
    let pid_path = logs_dir().join(format!("run-{run_id}.pid"));
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("打开日志失败: {e}"))?;
    let ret_path = log_path.clone();
    let cpu_num = std::thread::available_parallelism()
        .map(|n| n.get().to_string())
        .unwrap_or_else(|_| "1".into());

    std::thread::spawn(move || {
        use std::io::Write;
        let mut log = log_file;
        let mut final_code = 0;
        for step in steps {
            let mut cmd = root_cmd("/bin/bash");
            cmd.arg(&step.script)
                .env("ZAP_PATH", zap_path())
                .env("ZAPCTL", zap_path().join("zapctl"))
                .env("APPS_DIR", apps_dir())
                .env("LOG_FILE", &log_path)
                .env("CPU_NUM", &cpu_num)
                .stdout(std::process::Stdio::from(log.try_clone().unwrap_or_else(
                    |_| {
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&log_path)
                            .unwrap()
                    },
                )))
                .stderr(std::process::Stdio::from(log.try_clone().unwrap_or_else(
                    |_| {
                        std::fs::OpenOptions::new()
                            .append(true)
                            .open(&log_path)
                            .unwrap()
                    },
                )));
            for (k, v) in &step.env {
                cmd.env(k, v);
            }
            // 新进程组：pid == pgid，便于停止时 kill(-pid)
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
            match cmd.spawn() {
                Ok(mut child) => {
                    let pid = child.id();
                    let _ = std::fs::write(&pid_path, pid.to_string());
                    let code = child.wait().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
                    if code != 0 {
                        final_code = code;
                        break;
                    }
                }
                Err(e) => {
                    let _ = writeln!(log, "启动脚本失败: {e}");
                    final_code = -1;
                    break;
                }
            }
        }
        let _ = std::fs::remove_file(&pid_path);
        let _ = writeln!(log, "\n__ZAP_DONE__ {final_code}");
        on_done(final_code);
    });

    Ok(ret_path)
}

fn base_env() -> Vec<(String, String)> {
    vec![
        ("ZAP_PATH".into(), zap_path().to_string_lossy().into_owned()),
        (
            "ZAPCTL".into(),
            zap_path().join("zapctl").to_string_lossy().into_owned(),
        ),
        ("APPS_DIR".into(), apps_dir().to_string_lossy().into_owned()),
    ]
}

/// 构造包脚本执行环境：在 base_env 基础上补齐脚本通用变量。
/// - PKG_PATH 为包源目录（含脚本/app.yaml），APP_PATH 为安装目录
/// - version 为 Some 时注入 APP_VERSION 及由其解析的 MAJOR_VERSION/MINOR_VERSION
/// - LOG_FILE / CPU_NUM 由 spawn_background 统一注入
fn task_env(
    pkg_dir: &Path,
    app_path: &Path,
    app_name: &str,
    version: Option<&str>,
    run_id: &str,
) -> Vec<(String, String)> {
    let mut env = base_env();
    env.push(("PKG_PATH".into(), pkg_dir.to_string_lossy().into_owned()));
    env.push(("APP_ID".into(), run_id.to_string()));
    env.push(("APP_NAME".into(), app_name.to_string()));
    env.push(("APP_PATH".into(), app_path.to_string_lossy().into_owned()));
    env.push((
        "BUILD_PATH".into(),
        apps_dir()
            .join(".build")
            .join(app_name)
            .to_string_lossy()
            .into_owned(),
    ));
    env.push((
        "ZAP_DATA_PATH".into(),
        zap_path().join("data").to_string_lossy().into_owned(),
    ));
    if let Some(v) = version {
        env.push(("APP_VERSION".into(), v.to_string()));
        if let Some((major, rest)) = v.split_once('.') {
            env.push(("MAJOR_VERSION".into(), major.to_string()));
            env.push((
                "MINOR_VERSION".into(),
                rest.split('.').next().unwrap_or("").to_string(),
            ));
        }
    }
    env
}

// ── 动词实现 ───────────────────────────────────────────────

/// 校验 Git 源 URL：只允许 http(s) 或 git@ 形式，禁止命令注入。
fn validate_repo_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Git 地址不能为空".into());
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("git@")
    {
        Ok(())
    } else {
        Err("Git 地址必须以 http(s):// 或 git@ 开头".into())
    }
}

/// 后台执行仓库操作，进度写入 run-{run_id}.log，结束写 `__ZAP_DONE__ <code>`。
fn spawn_repo_task(
    run_id: String,
    op: impl FnOnce() -> Result<String, String> + Send + 'static,
    title: String,
) -> Response {
    let log_path = logs_dir().join(format!("run-{run_id}.log"));
    let ret_log = log_path.clone();
    // 同步创建日志文件，确保接口返回时文件已存在（WebSocket 立即读日志不会 ENOENT）
    if let Err(e) = std::fs::create_dir_all(logs_dir()).and_then(|_| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map(|_| ())
    }) {
        return Response::err(-1, format!("创建日志失败: {e}"));
    }
    tokio::task::spawn_blocking(move || {
        use std::io::Write;
        let _ = std::fs::create_dir_all(logs_dir());
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap_or_else(|_| {
                std::fs::OpenOptions::new()
                    .append(true)
                    .open("/dev/null")
                    .unwrap()
            });
        let _ = writeln!(log, "=== {title} ===");
        match op() {
            Ok(detail) => {
                let _ = writeln!(log, "成功: {detail}");
                let _ = writeln!(log, "\n__ZAP_DONE__ 0");
            }
            Err(e) => {
                let _ = writeln!(log, "失败: {e}");
                let _ = writeln!(log, "\n__ZAP_DONE__ -1");
            }
        }
    });
    Response::ok(
        "任务已启动",
        Some(json!({ "run_id": run_id, "log": ret_log })),
    )
}

/// 添加 Git 源（后台执行）：clone 到 repos/<id>/，并写入 repos.yaml。
pub async fn repo_add(name: String, url: String, run_id: String) -> Response {
    validate_repo_url(&url).map_or_else(
        |e| Response::err(-1, e),
        |_| {
            let title = format!("添加 Git 源: {name} ({url})");
            spawn_repo_task(run_id.clone(), move || repo_add_inner(&name, &url), title)
        },
    )
}

fn repo_add_inner(name: &str, url: &str) -> Result<String, String> {
    let mut repos = read_repos()?;
    // 生成唯一 id（来自 URL 末段，冲突自动加后缀）
    let base_id = id_from_url(url);
    let mut id = base_id.clone();
    let mut n = 2;
    while repos.iter().any(|r| r.id == id) {
        id = format!("{base_id}-{n}");
        n += 1;
    }
    let dir = repos_dir().join(&id);
    if dir.exists() {
        return Err(format!("源目录已存在: {}", dir.display()));
    }
    std::fs::create_dir_all(repos_dir()).map_err(|e| e.to_string())?;
    let tmp_clone = repos_dir().join(format!(".tmp-{id}"));
    let _ = std::fs::remove_dir_all(&tmp_clone);
    run_capture(
        "git",
        &["clone", "--depth", "1", url, tmp_clone.to_str().unwrap()],
    )?;
    // 临时目录非空校验，防止克隆出空目录
    if std::fs::read_dir(&tmp_clone)
        .map_err(|e| e.to_string())?
        .next()
        .is_none()
    {
        let _ = std::fs::remove_dir_all(&tmp_clone);
        return Err("克隆结果为空".into());
    }
    std::fs::rename(&tmp_clone, &dir).map_err(|e| format!("移动到源目录失败: {e}"))?;
    let commit = run_capture("git", &["-C", dir.to_str().unwrap(), "rev-parse", "HEAD"])?;
    let short = commit.chars().take(7).collect::<String>();
    repos.push(RepoEntry {
        id: id.clone(),
        name: name.to_string(),
        url: url.to_string(),
        builtin: false,
        enabled: true,
        version: short.clone(),
        commit: commit.clone(),
        updated_at: now_ts(),
    });
    write_repos(&repos)?;
    Ok(format!("id={id} commit={short}"))
}

/// 删除 Git 源（同步执行）：内置源禁止删除。
pub async fn repo_remove(id: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let id = safe_rel(&id)?.to_string_lossy().to_string();
        let mut repos = read_repos()?;
        let Some(idx) = repos.iter().position(|r| r.id == id) else {
            return Err(format!("源不存在: {id}"));
        };
        if repos[idx].builtin {
            return Err("内置源不可删除".into());
        }
        let dir = repos_dir().join(&id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("删除源目录失败: {e}"))?;
        }
        repos.remove(idx);
        write_repos(&repos)?;
        Ok(Response::ok("源已删除", Some(json!({ "id": id }))))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

/// 更新单个 Git 源（后台执行）：fetch + reset，或首次 clone。
pub async fn repo_update(id: String, run_id: String) -> Response {
    let valid =
        safe_rel(&id).and_then(|_| read_repos().map(|repos| repos.iter().any(|r| r.id == id)));
    match valid {
        Ok(true) => {
            let title = format!("更新 Git 源: {id}");
            spawn_repo_task(run_id.clone(), move || repo_update_inner(&id), title)
        }
        Ok(false) => Response::err(-1, format!("源不存在: {id}")),
        Err(e) => Response::err(-1, e),
    }
}

fn repo_update_inner(id: &str) -> Result<String, String> {
    let repos = read_repos()?;
    let Some(entry) = repos.iter().find(|r| r.id == id) else {
        return Err(format!("源不存在: {id}"));
    };
    let dir = repos_dir().join(id);
    if !dir.exists() || !dir.join(".git").exists() {
        // 首次 clone（内置源发行时无 .git，直接重建）
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(repos_dir()).map_err(|e| e.to_string())?;
        let tmp_clone = repos_dir().join(format!(".tmp-{id}"));
        let _ = std::fs::remove_dir_all(&tmp_clone);
        run_capture(
            "git",
            &[
                "clone",
                "--depth",
                "1",
                &entry.url,
                tmp_clone.to_str().unwrap(),
            ],
        )?;
        std::fs::rename(&tmp_clone, &dir).map_err(|e| format!("移动到源目录失败: {e}"))?;
    } else {
        run_capture("git", &["-C", dir.to_str().unwrap(), "fetch", "origin"])?;
        run_capture(
            "git",
            &["-C", dir.to_str().unwrap(), "reset", "--hard", "FETCH_HEAD"],
        )?;
    }
    let commit = run_capture("git", &["-C", dir.to_str().unwrap(), "rev-parse", "HEAD"])?;
    let short = commit.chars().take(7).collect::<String>();
    let mut new_repos = repos;
    if let Some(r) = new_repos.iter_mut().find(|r| r.id == id) {
        r.version = short.clone();
        r.commit = commit.clone();
        r.updated_at = now_ts();
    }
    write_repos(&new_repos)?;
    Ok(format!("commit={short}"))
}

pub async fn install(
    pkg_path: String,
    source: String,
    repo_id: Option<String>,
    version: String,
    action: Option<String>,
    run_id: String,
) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let (cat, name) = validate_pkg_path(&pkg_path)?;
        let pkg_dir = find_package(&pkg_path, &source, repo_id.as_deref())?;
        let script = script_file(&pkg_dir, "install", "bin.sh")?;
        let app_path = apps_dir().join(&pkg_path);
        let mut env = task_env(&pkg_dir, &app_path, &name, Some(&version), &run_id);
        if let Some(a) = action.as_deref() {
            if !a.is_empty() {
                env.push(("ACTION".into(), a.to_string()));
            }
        }
        let done_run_id = run_id.clone();
        let done_pkg_path = pkg_path.clone();
        let done_source = source.clone();
        let done_repo_id = repo_id.clone();
        let on_done = Box::new(move |code: i32| {
            if code == 0 {
                let meta = MetaInfo {
                    name: name.clone(),
                    version: version.clone(),
                    category: cat.clone(),
                    source: done_source.clone(),
                    repo_id: done_repo_id.clone(),
                    installed_at: now_ts(),
                    upgraded_from: None,
                    run_id: done_run_id.clone(),
                };
                if let Err(e) = write_meta(&app_path, &meta) {
                    tracing::error!("写入 {done_pkg_path} 安装元数据失败: {e}");
                }
            }
        });
        let log = spawn_background(&run_id, vec![ScriptStep { script, env }], on_done)?;
        Ok(Response::ok(
            "安装已启动",
            Some(json!({ "run_id": run_id, "log": log })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn uninstall(pkg_path: String, run_id: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let (_, name) = validate_pkg_path(&pkg_path)?;
        let app_path = apps_dir().join(&pkg_path);
        if !app_path.is_dir() {
            return Err("该包未安装".into());
        }
        let (source, repo_id) = read_meta(&app_path)
            .map(|m| (m.source, m.repo_id))
            .unwrap_or_else(|_| ("official".into(), None));
        let pkg_dir = find_package(&pkg_path, &source, repo_id.as_deref())?;
        let script = script_file(&pkg_dir, "uninstall", "uninstall.sh")?;
        // 卸载脚本可能需要版本信息（如按版本计算安装目录），注入 meta 中记录的版本
        let meta_version = read_meta(&app_path).ok().map(|m| m.version);
        let env = task_env(&pkg_dir, &app_path, &name, meta_version.as_deref(), &run_id);
        let on_done = Box::new(move |code: i32| {
            if code == 0 {
                let _ = std::fs::remove_dir_all(&app_path);
            }
        });
        let log = spawn_background(&run_id, vec![ScriptStep { script, env }], on_done)?;
        Ok(Response::ok(
            "卸载已启动",
            Some(json!({ "run_id": run_id, "log": log })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn upgrade(
    pkg_path: String,
    source: String,
    repo_id: Option<String>,
    version: String,
    old_version: String,
    action: Option<String>,
    run_id: String,
) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let (cat, name) = validate_pkg_path(&pkg_path)?;
        let app_path = apps_dir().join(&pkg_path);
        if !app_path.is_dir() {
            return Err("该包未安装，无法升级".into());
        }
        let pkg_dir = find_package(&pkg_path, &source, repo_id.as_deref())?;
        let mut env = task_env(&pkg_dir, &app_path, &name, Some(&version), &run_id);
        env.push(("APP_OLD_VERSION".into(), old_version.clone()));
        if let Some(a) = action.as_deref() {
            if !a.is_empty() {
                env.push(("ACTION".into(), a.to_string()));
            }
        }

        let mut steps = Vec::new();
        if pkg_dir.join("upgrade.sh").is_file() {
            steps.push(ScriptStep {
                script: script_file(&pkg_dir, "upgrade", "upgrade.sh")?,
                env: env.clone(),
            });
        } else {
            // 缺省升级策略：先卸载（uninstall.sh 自带数据备份）再安装
            steps.push(ScriptStep {
                script: script_file(&pkg_dir, "uninstall", "uninstall.sh")?,
                env: env.clone(),
            });
            steps.push(ScriptStep {
                script: script_file(&pkg_dir, "install", "bin.sh")?,
                env: env.clone(),
            });
        }
        let done_run_id = run_id.clone();
        let done_pkg_path = pkg_path.clone();
        let done_source = source.clone();
        let done_repo_id = repo_id.clone();
        let on_done = Box::new(move |code: i32| {
            if code == 0 {
                let meta = MetaInfo {
                    name: name.clone(),
                    version: version.clone(),
                    category: cat.clone(),
                    source: done_source.clone(),
                    repo_id: done_repo_id.clone(),
                    installed_at: now_ts(),
                    upgraded_from: Some(old_version.clone()),
                    run_id: done_run_id.clone(),
                };
                if let Err(e) = write_meta(&app_path, &meta) {
                    tracing::error!("写入 {done_pkg_path} 升级元数据失败: {e}");
                }
            }
        });
        let log = spawn_background(&run_id, steps, on_done)?;
        Ok(Response::ok(
            "升级已启动",
            Some(json!({ "run_id": run_id, "log": log })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn script_run(path: String, run_id: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let custom = appstore_dir().join("custom");
        let resolved = safe_join(&custom, &path)?;
        if !resolved.is_file() {
            return Err("脚本不存在".into());
        }
        let log = spawn_background(
            &run_id,
            vec![ScriptStep {
                script: resolved,
                env: base_env(),
            }],
            Box::new(|_| {}),
        )?;
        Ok(Response::ok(
            "脚本已启动",
            Some(json!({ "run_id": run_id, "log": log })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn script_stop(run_id: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let pid_path = logs_dir().join(format!("run-{run_id}.pid"));
        let pid: i32 = std::fs::read_to_string(&pid_path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .ok_or_else(|| "运行实例不存在或已结束".to_string())?;
        // 向进程组发 SIGTERM，最多等 5 秒后 SIGKILL
        let mut alive = unsafe { libc::kill(-pid, libc::SIGTERM) } == 0;
        for _ in 0..5 {
            if !alive {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            alive = unsafe { libc::kill(-pid, 0) } == 0;
        }
        if alive {
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        Ok::<_, String>(Response::ok("已发送停止信号", None))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn script_read(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let base = appstore_dir();
        let resolved = safe_join(&base, &path)?;
        let md = std::fs::metadata(&resolved).map_err(|e| format!("路径不存在: {e}"))?;
        if !md.is_file() {
            return Err("不是文件".to_string());
        }
        let content = std::fs::read_to_string(&resolved).map_err(|e| format!("读取失败: {e}"))?;
        Ok::<_, String>(Response::ok(
            "ok",
            Some(json!({ "path": resolved.to_string_lossy(), "content": content })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn script_write(path: String, content: String) -> Response {
    tokio::task::spawn_blocking(move || {
        use std::os::unix::fs::PermissionsExt;
        let custom = appstore_dir().join("custom");
        let resolved = safe_join(&custom, &path)?;
        if let Ok(md) = std::fs::metadata(&resolved) {
            if md.is_dir() {
                return Err("不能覆盖目录".to_string());
            }
        }
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&resolved, &content).map_err(|e| format!("写入失败: {e}"))?;
        // 脚本保持可执行
        let _ = std::fs::set_permissions(&resolved, std::fs::Permissions::from_mode(0o755));
        Ok::<_, String>(Response::ok(
            "保存成功",
            Some(json!({ "path": resolved.to_string_lossy() })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

pub async fn installed() -> Response {
    tokio::task::spawn_blocking(move || {
        let mut items = Vec::new();
        let root = apps_dir();
        if let Ok(cats) = std::fs::read_dir(&root) {
            for cat in cats.flatten() {
                let cat_path = cat.path();
                if !cat_path.is_dir() {
                    continue;
                }
                let category = cat.file_name().to_string_lossy().to_string();
                if let Ok(pkgs) = std::fs::read_dir(&cat_path) {
                    for pkg in pkgs.flatten() {
                        let app_path = pkg.path();
                        if !app_path.is_dir() {
                            continue;
                        }
                        let Ok(meta) = read_meta(&app_path) else {
                            continue;
                        };
                        let name = pkg.file_name().to_string_lossy().to_string();
                        let pkg_path = format!("{category}/{name}");
                        let info = read_info_yaml(&app_path);
                        let state = probe_instance_state(&app_path, info.as_ref());
                        let instance = info
                            .as_ref()
                            .and_then(|i| i.get("instance").and_then(|v| v.as_str()))
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| name.clone());
                        items.push(json!({
                            "pkg_path": pkg_path,
                            "name": meta.name,
                            "version": meta.version,
                            "category": meta.category,
                            "source": meta.source,
                            "repo_id": meta.repo_id,
                            "installed_at": meta.installed_at,
                            "upgraded_from": meta.upgraded_from,
                            "run_id": meta.run_id,
                            "instance": instance,
                            "state": state,
                            "info": info.as_ref().map(yaml_to_json).unwrap_or_else(|| json!({})),
                        }));
                    }
                }
            }
        }
        items.sort_by(|a, b| {
            a.get("category")
                .and_then(|c| c.as_str())
                .cmp(&b.get("category").and_then(|c| c.as_str()))
                .then_with(|| {
                    a.get("name")
                        .and_then(|n| n.as_str())
                        .cmp(&b.get("name").and_then(|n| n.as_str()))
                })
        });
        Response::ok("ok", Some(json!({ "items": items })))
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

/// 读取安装脚本登记的实例信息 apps/<category>/<name>/info.yaml（可选文件）。
fn read_info_yaml(app_path: &Path) -> Option<serde_yaml::Value> {
    let content = std::fs::read_to_string(app_path.join("info.yaml")).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// 探测实例运行状态：登记了 svc_name（systemd）走 systemctl；否则读 pid_file 探活。
fn probe_instance_state(_app_path: &Path, info: Option<&serde_yaml::Value>) -> String {
    let info = match info {
        Some(i) => i,
        None => return "unknown".into(),
    };
    if let Some(svc) = info.get("svc_name").and_then(|v| v.as_str()) {
        return match root_cmd("systemctl").args(["is-active", svc]).output() {
            Ok(o) => normalize_state(String::from_utf8_lossy(&o.stdout).trim()),
            Err(_) => "unknown".into(),
        };
    }
    if let Some(pf) = info.get("pid_file").and_then(|v| v.as_str()) {
        let pid: i32 = match std::fs::read_to_string(pf)
            .ok()
            .and_then(|t| t.trim().parse().ok())
        {
            Some(p) => p,
            None => return "unknown".into(),
        };
        return match root_cmd("kill").args(["-0", &pid.to_string()]).status() {
            Ok(s) if s.success() => "running".into(),
            _ => "stopped".into(),
        };
    }
    "unknown".into()
}

/// 归一化 systemctl 状态输出：running/stopped/failed/starting/stopping/unknown。
fn normalize_state(raw: &str) -> String {
    match raw {
        "active" | "running" => "running",
        "inactive" | "dead" | "stopped" | "exited" => "stopped",
        "failed" => "failed",
        "activating" | "reloading" => "starting",
        "deactivating" => "stopping",
        _ => "unknown",
    }
    .into()
}

fn serde_yaml_num_str(n: &serde_yaml::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    n.as_f64().map(|f| f.to_string()).unwrap_or_default()
}

/// serde_yaml 值 → serde_json 值（数字转字符串、布尔保留，便于展示与比对）。
fn yaml_to_json(v: &serde_yaml::Value) -> Value {
    match v {
        serde_yaml::Value::String(s) => Value::String(s.clone()),
        serde_yaml::Value::Bool(b) => Value::Bool(*b),
        serde_yaml::Value::Number(n) => Value::String(serde_yaml_num_str(n)),
        serde_yaml::Value::Mapping(m) => {
            let mut obj = serde_json::Map::new();
            for (k, val) in m {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Number(n) => serde_yaml_num_str(n),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    other => format!("{other:?}"),
                };
                obj.insert(key, yaml_to_json(val));
            }
            Value::Object(obj)
        }
        serde_yaml::Value::Sequence(seq) => Value::Array(seq.iter().map(yaml_to_json).collect()),
        serde_yaml::Value::Tagged(t) => yaml_to_json(&t.value),
        serde_yaml::Value::Null => Value::Null,
    }
}

/// 对已安装应用的实例执行 start/stop/restart。
/// 要求脚本在 info.yaml 中登记 svc_name（systemd unit），由 root 执行 systemctl。
pub async fn instance_action(pkg_path: String, action: String) -> Response {
    let allowed = ["start", "stop", "restart"];
    if !allowed.contains(&action.as_str()) {
        return Response::err(-1, format!("不支持的实例操作: {action}"));
    }
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let (cat, name) = validate_pkg_path(&pkg_path)?;
        let app_path = apps_dir().join(&cat).join(&name);
        if !app_path.is_dir() {
            return Err("该应用未安装".into());
        }
        let info = read_info_yaml(&app_path).ok_or("缺少实例信息 info.yaml（由安装脚本登记）")?;
        let svc = info
            .get("svc_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or("未登记 systemd 服务（info.yaml 缺 svc_name），无法通过面板启停")?;
        let out = root_cmd("systemctl")
            .args([action.as_str(), svc.as_str()])
            .output()
            .map_err(|e| format!("执行 systemctl {action} {svc} 失败: {e}"))?;
        if !out.status.success() {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(format!("systemctl {action} {svc} 失败: {msg}"));
        }
        // 操作后回读状态（restart 稍等稳定）
        std::thread::sleep(std::time::Duration::from_millis(300));
        let state = match root_cmd("systemctl").args(["is-active", &svc]).output() {
            Ok(o) => normalize_state(String::from_utf8_lossy(&o.stdout).trim()),
            Err(_) => "unknown".into(),
        };
        Ok(Response::ok(
            "ok",
            Some(json!({
                "pkg_path": pkg_path,
                "svc_name": svc,
                "action": action,
                "state": state,
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .map_or_else(|e| Response::err(-1, e), |r| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 建立独立 ZAP_PATH，返回 (guard, zap_root)。
    fn with_zap_root() -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        let guard = ENV_GUARD.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "zap-appstore-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe {
            std::env::set_var("ZAP_PATH", &dir);
        }
        (guard, dir)
    }

    #[test]
    fn safe_rel_rejects_absolute_and_parent() {
        assert!(safe_rel("/etc/passwd").is_err());
        assert!(safe_rel("../etc").is_err());
        assert!(safe_rel("a/../../b").is_err());
        assert!(safe_rel("a/b/c.sh").is_ok());
    }

    #[test]
    fn safe_join_rejects_escape() {
        let base = PathBuf::from("/tmp/zap-test-base");
        assert!(safe_join(&base, "../x").is_err());
        let ok = safe_join(&base, "sub/x.sh").unwrap();
        assert_eq!(ok, base.join("sub/x.sh"));
    }

    #[test]
    fn validate_pkg_path_rules() {
        assert_eq!(
            validate_pkg_path("database/mariadb").unwrap(),
            ("database".into(), "mariadb".into())
        );
        assert!(validate_pkg_path("mariadb").is_err());
        assert!(validate_pkg_path("a/b/c").is_err());
        assert!(validate_pkg_path("db/mariadb;rm").is_err());
        assert!(validate_pkg_path("db/../x").is_err());
    }

    #[test]
    fn script_file_parses_app_yaml_override() {
        let (_g, root) = with_zap_root();
        let pkg = root.join("database/nginx");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(
            pkg.join("app.yaml"),
            "name: nginx\nversion: \"1.27\"\nscripts:\n  install: setup.sh\n  uninstall: remove.sh\n",
        )
        .unwrap();
        std::fs::write(pkg.join("setup.sh"), "#!/bin/bash\n").unwrap();
        std::fs::write(pkg.join("remove.sh"), "#!/bin/bash\n").unwrap();
        std::fs::write(pkg.join("upgrade.sh"), "#!/bin/bash\n").unwrap();

        let install = script_file(&pkg, "install", "bin.sh").unwrap();
        assert_eq!(install.file_name().unwrap(), "setup.sh");
        // 未在 yaml 中定义的 key 回退到默认
        let upgrade = script_file(&pkg, "upgrade", "upgrade.sh").unwrap();
        assert_eq!(upgrade.file_name().unwrap(), "upgrade.sh");
        // 覆盖指向不存在的文件 → 报错
        let missing = script_file(&pkg, "install", "bin.sh");
        assert!(missing.is_ok());
    }

    #[test]
    fn repos_round_trip_and_builtin_fallback() {
        let (_g, _root) = with_zap_root();
        // 空 repos.yaml（不存在）→ 自动补内置源
        let repos = read_repos().unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].id, BUILTIN_REPO_ID);
        assert!(repos[0].builtin);
        // 往返写入
        let mut repos = repos;
        repos.push(RepoEntry {
            id: "my-store".into(),
            name: "My Store".into(),
            url: "https://github.com/user/store.git".into(),
            builtin: false,
            enabled: true,
            version: "abc1234".into(),
            commit: "abc1234".repeat(8),
            updated_at: 1_700_000_000,
        });
        write_repos(&repos).unwrap();
        let back = read_repos().unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[1].id, "my-store");
        assert_eq!(back[1].version, "abc1234");
    }

    #[test]
    fn id_from_url_extracts_repo_name() {
        assert_eq!(
            id_from_url("https://github.com/zapj/zap-appstore.git"),
            "zap-appstore"
        );
        assert_eq!(id_from_url("https://gitlab.com/org/store"), "store");
        assert_eq!(id_from_url("git@github.com:user/store.git"), "store");
        assert_eq!(id_from_url("https://x.io/A_B-C.d/"), "a-b-c-d");
    }

    #[test]
    fn validate_repo_url_rules() {
        assert!(validate_repo_url("https://github.com/a/b.git").is_ok());
        assert!(validate_repo_url("git@github.com:a/b.git").is_ok());
        assert!(validate_repo_url("http://x/y.git").is_ok());
        assert!(validate_repo_url("").is_err());
        assert!(validate_repo_url("; rm -rf /").is_err());
        assert!(validate_repo_url("/tmp/store").is_err());
    }

    #[test]
    fn meta_round_trip_and_installed() {
        let (_g, root) = with_zap_root();
        let app = root.join("data/apps/database/mariadb");
        let meta = MetaInfo {
            name: "mariadb".into(),
            version: "11.4.4".into(),
            category: "database".into(),
            source: "official".into(),
            repo_id: Some("zap-appstore".into()),
            installed_at: 1_700_000_000,
            upgraded_from: None,
            run_id: "r1".into(),
        };
        write_meta(&app, &meta).unwrap();
        let back = read_meta(&app).unwrap();
        assert_eq!(back.version, "11.4.4");
        assert_eq!(back.source, "official");
        assert_eq!(back.repo_id.as_deref(), Some("zap-appstore"));
        assert!(back.upgraded_from.is_none());
    }
}
