//! 数据库管理（MySQL / MariaDB）。
//!
//! 通过本机 `mysql` 客户端 + `zapadm` 凭据（由 zap-crypto 从
//! /etc/zap/credentials 解密读取）执行管理操作，**不使用 root 账号**：
//!
//! - GET  /api/database/status          服务状态与版本
//! - GET  /api/database/list            库列表（含大小 / 表数 / 字符集）
//! - POST /api/database/create          创建库
//! - POST /api/database/drop            删除库
//! - GET  /api/database/users           用户列表（及其授权）
//! - POST /api/database/user/create     创建用户并授权到指定库
//! - POST /api/database/user/drop       删除用户
//! - GET  /api/database/remote          远程访问授权列表
//! - POST /api/database/remote/grant    授权某主机远程访问某库
//! - POST /api/database/remote/revoke   撤销远程授权
//!
//! 多租户：非管理员只能看到并操作以「用户名_」为前缀的库，
//! 创建库时会自动补上该前缀；管理员不受限制。

use std::path::PathBuf;
use std::process::Command;

use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::zap::ZapError;
use crate::zap::ZapJsonResult;
use crate::zap::jwt::{ValidatedClaims, is_admin};

/// 凭据坐标（与 `zapctl cred show mysql zapadm` 一致）
const CRED_SERVICE: &str = "mysql";
const CRED_USER: &str = "zapadm";

/// mysql 客户端候选路径（按优先级）
const MYSQL_BINS: &[&str] = &[
    "/usr/local/mysql/bin/mysql",
    "/usr/local/apps/mysql-8.4/bin/mysql",
    "/usr/bin/mysql",
];

/// 本机 socket 候选（本地连接优先走 socket，无需开 TCP）
const SOCKETS: &[&str] = &[
    "/tmp/mysql.sock",
    "/var/run/mysqld/mysqld.sock",
    "/run/mysqld/mysqld.sock",
];

/// 系统库：列表与统计中一律排除
const SYSTEM_SCHEMAS: &[&str] = &["information_schema", "mysql", "performance_schema", "sys"];

// ── 连接与执行 ──────────────────────────────────────────────

fn mysql_bin() -> Result<PathBuf, ZapError> {
    MYSQL_BINS
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .ok_or_else(|| {
            ZapError::New(
                -1,
                "未找到 mysql 客户端，请先安装 MySQL / MariaDB".to_string(),
            )
        })
}

fn socket_path() -> Option<&'static str> {
    SOCKETS
        .iter()
        .copied()
        .find(|s| std::path::Path::new(s).exists())
}

/// 从 /etc/zap/credentials 解密读取 zapadm 密码（库内解密，不起子进程）。
fn zapadm_password() -> Result<String, ZapError> {
    zap_crypto::read_cred(CRED_SERVICE, CRED_USER)
        .map_err(|e| ZapError::New(-1, format!("读取数据库凭据失败：{e}")))
}

/// 执行 SQL，返回「无表头 + TAB 分隔」的输出。
///
/// 密码通过 `MYSQL_PWD` 环境变量传递，避免出现在进程命令行里。
fn run_sql(sql: &str) -> Result<String, ZapError> {
    let bin = mysql_bin()?;
    let pwd = zapadm_password()?;

    let mut cmd = Command::new(bin);
    cmd.env("MYSQL_PWD", &pwd);
    if let Some(sock) = socket_path() {
        cmd.arg("--socket").arg(sock);
    }
    cmd.arg("-u")
        .arg(CRED_USER)
        .arg("-N")
        .arg("-B")
        .arg("-e")
        .arg(sql);

    let out = cmd
        .output()
        .map_err(|e| ZapError::New(-1, format!("执行数据库命令失败：{e}")))?;

    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(ZapError::New(-1, format!("数据库操作失败：{err}")))
    }
}

/// 执行若干条语句（用分号分隔，内部自行转义标识符）。
fn run_sqls(sqls: &[String]) -> Result<(), ZapError> {
    let joined = sqls.join("; ");
    run_sql(&joined).map(|_| ())
}

// ── 校验与转义 ──────────────────────────────────────────────

/// 校验标识符（库名 / 用户名）：字母、数字、下划线、连字符，长度 ≤ 64。
fn check_ident(name: &str, label: &str) -> Result<String, ZapError> {
    if name.is_empty() || name.len() > 64 {
        return Err(ZapError::New(-1, format!("{label}长度必须在 1-64 之间")));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(ZapError::New(
            -1,
            format!("{label}只能包含字母、数字、下划线和连字符"),
        ));
    }
    Ok(name.to_string())
}

/// 转义 SQL 字符串字面量（用于密码等）。
fn escape_literal(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

/// 主机部分（用于 user@host）：允许 %、IP、localhost 与域名。
fn check_host(host: &str) -> Result<String, ZapError> {
    let h = host.trim();
    if h.is_empty() || h.len() > 255 {
        return Err(ZapError::New(-1, "主机地址不合法".to_string()));
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '%' | '_' | ':'))
    {
        return Err(ZapError::New(-1, "主机地址含有非法字符".to_string()));
    }
    Ok(h.to_string())
}

/// 非管理员可见的库名前缀；管理员返回 None 表示不限制。
fn schema_prefix(claims: &ValidatedClaims) -> Option<String> {
    if is_admin(claims) {
        return None;
    }
    let user: String = claims
        .sub
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    Some(format!("{user}_"))
}

/// 校验库归属：非管理员只能操作自己前缀下的库。
fn ensure_owned(claims: &ValidatedClaims, schema: &str) -> Result<String, ZapError> {
    let name = check_ident(schema, "数据库名")?;
    if let Some(p) = schema_prefix(claims)
        && !name.starts_with(&p)
    {
        return Err(ZapError::New(
            -1,
            format!("无权操作数据库 `{name}`：只能管理以 `{p}` 开头的库"),
        ));
    }
    Ok(name)
}

// ── 请求体 ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SchemaReq {
    pub name: String,
}

#[derive(Deserialize)]
pub struct CreateDbReq {
    pub name: String,
    #[serde(default = "default_charset")]
    pub charset: String,
}

fn default_charset() -> String {
    "utf8mb4".to_string()
}

#[derive(Deserialize)]
pub struct UserCreateReq {
    pub user: String,
    pub password: String,
    /// 允许连接的主机：localhost / % / 具体 IP
    #[serde(default = "default_host")]
    pub host: String,
    /// 授权到的库；为空表示不授权库（仅创建用户）
    #[serde(default)]
    pub schema: Option<String>,
}

fn default_host() -> String {
    "localhost".to_string()
}

#[derive(Deserialize)]
pub struct UserDropReq {
    pub user: String,
    #[serde(default = "default_host")]
    pub host: String,
}

#[derive(Deserialize)]
pub struct RemoteGrantReq {
    pub user: String,
    pub schema: String,
    /// 允许远程连接的主机（IP 或 %）
    pub host: String,
    /// 是否同时设置/更新密码（可选）
    #[serde(default)]
    pub password: Option<String>,
}

/// 成功返回：与其它模块保持一致的 `{ code: 0, data: ... }` 包装。
fn ok(data: Value) -> ZapJsonResult {
    Ok(Json(json!({ "code": 0, "data": data })))
}

// ── handlers ────────────────────────────────────────────────

/// GET /api/database/status：服务状态与版本。
pub async fn status(_claims: ValidatedClaims) -> ZapJsonResult {
    let version = run_sql("SELECT VERSION()")?
        .trim()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    ok(json!({
        "ok": true,
        "version": version,
        "user": CRED_USER,
        "socket": socket_path(),
    }))
}

/// GET /api/database/list：库列表（含大小、表数、字符集）。
pub async fn list(claims: ValidatedClaims) -> ZapJsonResult {
    let sql = "SELECT s.SCHEMA_NAME, \
               COALESCE(s.DEFAULT_CHARACTER_SET_NAME,''), \
               COALESCE(s.DEFAULT_COLLATION_NAME,''), \
               COALESCE(SUM(t.DATA_LENGTH + t.INDEX_LENGTH), 0), \
               COUNT(t.TABLE_NAME) \
               FROM information_schema.SCHEMATA s \
               LEFT JOIN information_schema.TABLES t ON t.TABLE_SCHEMA = s.SCHEMA_NAME \
               GROUP BY s.SCHEMA_NAME, s.DEFAULT_CHARACTER_SET_NAME, s.DEFAULT_COLLATION_NAME \
               ORDER BY s.SCHEMA_NAME";

    let out = run_sql(sql)?;
    let prefix = schema_prefix(&claims);

    let mut items: Vec<Value> = Vec::new();
    for line in out.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let name = cols[0].to_string();
        if SYSTEM_SCHEMAS.contains(&name.as_str()) {
            continue;
        }
        if let Some(p) = &prefix
            && !name.starts_with(p.as_str())
        {
            continue;
        }
        let size: u64 = cols[3].parse().unwrap_or(0);
        let tables: u64 = cols[4].parse().unwrap_or(0);
        items.push(json!({
            "name": name,
            "charset": cols[1],
            "collation": cols[2],
            "size": size,
            "tables": tables,
        }));
    }

    ok(json!({ "ok": true, "list": items, "prefix": prefix }))
}

/// POST /api/database/create：创建数据库。
pub async fn create(claims: ValidatedClaims, Json(req): Json<CreateDbReq>) -> ZapJsonResult {
    let raw = req.name.trim();
    let name = match schema_prefix(&claims) {
        Some(p) if !raw.starts_with(&p) => check_ident(&format!("{p}{raw}"), "数据库名")?,
        _ => ensure_owned(&claims, raw)?,
    };
    let charset = if req.charset.trim().is_empty() {
        "utf8mb4".to_string()
    } else {
        check_ident(req.charset.trim(), "字符集")?
    };

    run_sqls(&[format!("CREATE DATABASE `{name}` CHARACTER SET {charset}")])?;

    ok(json!({ "ok": true, "name": name }))
}

/// POST /api/database/drop：删除数据库。
pub async fn drop_db(claims: ValidatedClaims, Json(req): Json<SchemaReq>) -> ZapJsonResult {
    let name = ensure_owned(&claims, req.name.trim())?;
    run_sqls(&[format!("DROP DATABASE `{name}`")])?;
    ok(json!({ "ok": true, "name": name }))
}

/// GET /api/database/users：数据库用户列表（含授权）。
pub async fn users(claims: ValidatedClaims) -> ZapJsonResult {
    let out = run_sql("SELECT user, host FROM mysql.user ORDER BY user, host")?;
    let skip = [
        "root",
        "mysql.session",
        "mysql.sys",
        "mysql.infoschema",
        "debian-sys-maint",
        "zapadm",
    ];
    let prefix = schema_prefix(&claims);

    let mut items: Vec<Value> = Vec::new();
    for line in out.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let user = cols[0].to_string();
        let host = cols[1].to_string();
        if skip.contains(&user.as_str()) {
            continue;
        }
        if let Some(p) = &prefix
            && !user.starts_with(p.as_str())
        {
            continue;
        }
        let grants = run_sql(&format!("SHOW GRANTS FOR '{user}'@'{host}'")).unwrap_or_default();
        items.push(json!({ "user": user, "host": host, "grants": grants }));
    }

    ok(json!({ "ok": true, "list": items }))
}

/// POST /api/database/user/create：创建用户（可选授权到某个库）。
pub async fn user_create(claims: ValidatedClaims, Json(req): Json<UserCreateReq>) -> ZapJsonResult {
    let prefix = schema_prefix(&claims);
    let user = match &prefix {
        Some(p) if !req.user.starts_with(p.as_str()) => {
            check_ident(&format!("{p}{}", req.user.trim()), "用户名")?
        }
        _ => check_ident(req.user.trim(), "用户名")?,
    };
    let host = check_host(&req.host)?;
    if req.password.len() < 8 {
        return Err(ZapError::New(-1, "密码长度不能少于 8 位".to_string()));
    }
    let pwd = escape_literal(&req.password);

    let mut sqls = vec![format!(
        "CREATE USER '{user}'@'{host}' IDENTIFIED BY '{pwd}'"
    )];
    if let Some(schema) = req.schema.as_deref() {
        let db = ensure_owned(&claims, schema.trim())?;
        sqls.push(format!(
            "GRANT ALL PRIVILEGES ON `{db}`.* TO '{user}'@'{host}'"
        ));
    }
    sqls.push("FLUSH PRIVILEGES".to_string());
    run_sqls(&sqls)?;

    ok(json!({ "ok": true, "user": user, "host": host }))
}

/// POST /api/database/user/drop：删除用户。
pub async fn user_drop(claims: ValidatedClaims, Json(req): Json<UserDropReq>) -> ZapJsonResult {
    let user = check_ident(req.user.trim(), "用户名")?;
    let host = check_host(&req.host)?;
    if let Some(p) = schema_prefix(&claims)
        && !user.starts_with(&p)
    {
        return Err(ZapError::New(-1, format!("无权删除用户 `{user}`")));
    }
    run_sqls(&[
        format!("DROP USER '{user}'@'{host}'"),
        "FLUSH PRIVILEGES".to_string(),
    ])?;
    ok(json!({ "ok": true, "user": user }))
}

/// GET /api/database/remote：远程访问授权列表（host 不是本机来源的账号）。
pub async fn remote_list(claims: ValidatedClaims) -> ZapJsonResult {
    let out = run_sql("SELECT user, host FROM mysql.user ORDER BY user, host")?;
    let prefix = schema_prefix(&claims);

    let mut items: Vec<Value> = Vec::new();
    for line in out.lines() {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let user = cols[0].to_string();
        let host = cols[1].to_string();
        // 远程授权：host 不是 localhost / 127.0.0.1 / ::1
        if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
            continue;
        }
        if let Some(p) = &prefix
            && !user.starts_with(p.as_str())
        {
            continue;
        }
        let grants = run_sql(&format!("SHOW GRANTS FOR '{user}'@'{host}'")).unwrap_or_default();
        items.push(json!({ "user": user, "host": host, "grants": grants }));
    }

    ok(json!({ "ok": true, "list": items }))
}

/// POST /api/database/remote/grant：授权某主机远程访问某库。
pub async fn remote_grant(
    claims: ValidatedClaims,
    Json(req): Json<RemoteGrantReq>,
) -> ZapJsonResult {
    let user = check_ident(req.user.trim(), "用户名")?;
    let db = ensure_owned(&claims, req.schema.trim())?;
    let host = check_host(&req.host)?;
    if let Some(p) = schema_prefix(&claims)
        && !user.starts_with(&p)
    {
        return Err(ZapError::New(-1, format!("无权配置用户 `{user}`")));
    }

    let ident = format!("'{user}'@'{host}'");
    let exists = run_sql(&format!(
        "SELECT 1 FROM mysql.user WHERE user = '{user}' AND host = '{host}'"
    ))
    .map(|s| !s.trim().is_empty())
    .unwrap_or(false);

    let mut sqls = Vec::new();
    if !exists {
        match req.password.as_deref().map(str::trim) {
            Some(p) if !p.is_empty() => {
                if p.len() < 8 {
                    return Err(ZapError::New(-1, "密码长度不能少于 8 位".to_string()));
                }
                sqls.push(format!(
                    "CREATE USER {ident} IDENTIFIED BY '{}'",
                    escape_literal(p)
                ));
            }
            _ => {
                return Err(ZapError::New(
                    -1,
                    "该用户在该主机下不存在，设置密码后才能创建".to_string(),
                ));
            }
        }
    }
    sqls.push(format!("GRANT ALL PRIVILEGES ON `{db}`.* TO {ident}"));
    sqls.push("FLUSH PRIVILEGES".to_string());
    run_sqls(&sqls)?;

    ok(json!({ "ok": true, "user": user, "host": host, "schema": db }))
}

/// POST /api/database/remote/revoke：撤销远程授权（删除该 host 下的账号）。
pub async fn remote_revoke(claims: ValidatedClaims, Json(req): Json<UserDropReq>) -> ZapJsonResult {
    let user = check_ident(req.user.trim(), "用户名")?;
    let host = check_host(&req.host)?;
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
        return Err(ZapError::New(
            -1,
            "只能撤销远程主机（非 localhost）的授权".to_string(),
        ));
    }
    if let Some(p) = schema_prefix(&claims)
        && !user.starts_with(&p)
    {
        return Err(ZapError::New(-1, format!("无权撤销用户 `{user}`")));
    }
    run_sqls(&[
        format!("DROP USER '{user}'@'{host}'"),
        "FLUSH PRIVILEGES".to_string(),
    ])?;
    ok(json!({ "ok": true, "user": user, "host": host }))
}
