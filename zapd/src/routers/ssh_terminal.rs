use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};

use axum::{
    Json,
    extract::{
        Extension, Path, Query,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, Row};
use ssh2::{OpenFlags, OpenType, Session};
use tracing::{error, info, warn};

use zap_proto::Request;

use crate::config;
use crate::db;
use crate::zap::audit;
use crate::zap::crypto;
use crate::zap::jwt::{Claims, ValidatedClaims};
use crate::zap::{ZapError, ZapJsonResult};

// ── Database schema ────────────────────────────────────────

pub async fn init_table() {
    if !table_exists("ssh_connections").await {
        let sql = r#"
        CREATE TABLE ssh_connections (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(128) NOT NULL,
            host VARCHAR(256) NOT NULL,
            port INTEGER DEFAULT 22,
            username VARCHAR(128) NOT NULL DEFAULT 'root',
            auth_type VARCHAR(32) NOT NULL DEFAULT 'password',
            password VARCHAR(512) DEFAULT '',
            ssh_key_name VARCHAR(128) DEFAULT '',
            remark TEXT DEFAULT '',
            status INTEGER DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at INTEGER,
            updated_at INTEGER
        )
        "#;
        let pool = db::get_db_pool().await;
        pool.execute(sql).await.unwrap();
        info!("ssh_connections table created");
    }
    // 默认本地连接：面板本机 127.0.0.1/root，密码留空。
    // 新库与老库都幂等补插（已存在 root@127.0.0.1 或 root@localhost 则跳过），
    // 用户后续自行编辑填写密码或改为密钥。
    ensure_default_loopback_connection().await;
}

/// 幂等补插默认本地连接（127.0.0.1 / root / 22，密码为空）。
/// 空密码连接双击时由前端弹窗输入密码、仅本次会话使用不落库。
async fn ensure_default_loopback_connection() {
    let pool = db::get_db_pool().await;
    let existing = sqlx::query(
        "SELECT id FROM ssh_connections
         WHERE username = 'root' AND (host = '127.0.0.1' OR host = 'localhost')
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    if existing.is_some() {
        return;
    }
    let now = chrono::Utc::now().timestamp();
    let remark = "本机默认连接：未设置密码，双击连接时输入密码（仅本次会话使用，不会保存）";
    sqlx::query(
        "INSERT INTO ssh_connections (name, host, port, username, auth_type, password, ssh_key_name, remark, status, sort_order, created_at, updated_at)
         VALUES ('localhost', '127.0.0.1', 22, 'root', 'password', '', '', ?, 1, 0, ?, ?)",
    )
    .bind(remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .ok();
    info!("Default loopback SSH connection seeded (root@127.0.0.1, no password)");
}

async fn table_exists(table_name: &str) -> bool {
    let pool = db::get_db_pool().await;
    let result: Result<(String,), sqlx::Error> =
        sqlx::query_as("select name from sqlite_master where name = ?")
            .bind(table_name)
            .fetch_one(pool)
            .await;
    result.is_ok()
}

// ── Models ─────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct SshConnection {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub auth_type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub password: String,
    /// 是否已保存密码（空密码 = 未设置，连接时由前端弹窗临时输入，不落库）
    pub has_password: bool,
    pub ssh_key_name: String,
    pub remark: String,
    pub status: i32,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_conn(row: &sqlx::sqlite::SqliteRow) -> SshConnection {
    let stored_password: String = row.try_get("password").unwrap_or_default();
    SshConnection {
        id: row.get("id"),
        name: row.get("name"),
        host: row.get("host"),
        port: row.get("port"),
        username: row.get("username"),
        auth_type: row.get("auth_type"),
        password: String::new(), // 一律不回传密文
        has_password: !stored_password.is_empty(),
        ssh_key_name: row.try_get("ssh_key_name").unwrap_or_default(),
        remark: row.try_get("remark").unwrap_or_default(),
        status: row.try_get("status").unwrap_or(1),
        sort_order: row.try_get("sort_order").unwrap_or(0),
        created_at: row.try_get("created_at").unwrap_or(0),
        updated_at: row.try_get("updated_at").unwrap_or(0),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateConnectionPayload {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: i32,
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub ssh_key_name: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionPayload {
    pub name: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub auth_type: Option<String>,
    pub password: Option<String>,
    pub ssh_key_name: Option<String>,
    pub remark: Option<String>,
    pub status: Option<i32>,
    pub sort_order: Option<i32>,
}

fn default_port() -> i32 {
    22
}
fn default_username() -> String {
    "root".to_string()
}
fn default_auth_type() -> String {
    "password".to_string()
}

// ── CRUD handlers ──────────────────────────────────────────

pub async fn list_connections(_claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let rows = sqlx::query(
        "SELECT id, name, host, port, username, auth_type, password, ssh_key_name,
                remark, status, sort_order, created_at, updated_at
         FROM ssh_connections ORDER BY sort_order, id",
    )
    .fetch_all(pool)
    .await?;

    let connections: Vec<SshConnection> = rows.iter().map(row_to_conn).collect();
    Ok(Json(json!({ "code": 0, "data": connections })))
}

pub async fn get_connection(_claims: ValidatedClaims, Path(id): Path<i64>) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let row = sqlx::query(
        "SELECT id, name, host, port, username, auth_type, password, ssh_key_name,
                remark, status, sort_order, created_at, updated_at
         FROM ssh_connections WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let mut conn = row_to_conn(&r);
            conn.password = String::new(); // 脱敏
            Ok(Json(json!({ "code": 0, "data": conn })))
        }
        None => Err(ZapError::New(-1, "连接不存在".to_string())),
    }
}

pub async fn create_connection(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CreateConnectionPayload>,
) -> ZapJsonResult {
    if payload.name.trim().is_empty() {
        return Err(ZapError::New(-1, "连接名称不能为空".to_string()));
    }
    if payload.host.trim().is_empty() {
        return Err(ZapError::New(-1, "主机地址不能为空".to_string()));
    }
    if payload.auth_type != "password" && payload.auth_type != "key" {
        return Err(ZapError::New(
            -1,
            "认证类型仅支持 password 或 key".to_string(),
        ));
    }
    // 密码认证允许密码为空：表示「未设置密码」，连接时由前端弹窗临时输入、不落库
    if payload.auth_type == "key" && payload.ssh_key_name.is_empty() {
        return Err(ZapError::New(
            -1,
            "密钥认证时必须选择一个 SSH 密钥".to_string(),
        ));
    }

    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();

    // 密码加密后入库，杜绝明文存储
    let encrypted_password = crypto::encrypt_password(&payload.password);

    sqlx::query(
        "INSERT INTO ssh_connections (name, host, port, username, auth_type, password, ssh_key_name, remark, status, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)"
    )
    .bind(payload.name.trim())
    .bind(payload.host.trim())
    .bind(payload.port)
    .bind(&payload.username)
    .bind(&payload.auth_type)
    .bind(encrypted_password)
    .bind(&payload.ssh_key_name)
    .bind(&payload.remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssh_connection_create",
        &format!("{}@{}:{}", payload.username, payload.host, payload.port),
        &payload.name,
    )
    .await;

    info!("SSH connection created: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "创建成功" })))
}

pub async fn update_connection(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateConnectionPayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;

    let row = sqlx::query("SELECT id FROM ssh_connections WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    if row.is_none() {
        return Err(ZapError::New(-1, "连接不存在".to_string()));
    }

    let now = chrono::Utc::now().timestamp();

    if let Some(v) = payload.name.as_deref().map(|s| s.trim().to_string()) {
        sqlx::query("UPDATE ssh_connections SET name = ?, updated_at = ? WHERE id = ?")
            .bind(&v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.host.as_deref().map(|s| s.trim().to_string()) {
        sqlx::query("UPDATE ssh_connections SET host = ?, updated_at = ? WHERE id = ?")
            .bind(&v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.port {
        sqlx::query("UPDATE ssh_connections SET port = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.username {
        sqlx::query("UPDATE ssh_connections SET username = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.auth_type {
        sqlx::query("UPDATE ssh_connections SET auth_type = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.password {
        // 密码加密后入库
        let encrypted = crypto::encrypt_password(&v);
        sqlx::query("UPDATE ssh_connections SET password = ?, updated_at = ? WHERE id = ?")
            .bind(encrypted)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.ssh_key_name {
        sqlx::query("UPDATE ssh_connections SET ssh_key_name = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.remark {
        sqlx::query("UPDATE ssh_connections SET remark = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.status {
        sqlx::query("UPDATE ssh_connections SET status = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }
    if let Some(v) = payload.sort_order {
        sqlx::query("UPDATE ssh_connections SET sort_order = ?, updated_at = ? WHERE id = ?")
            .bind(v)
            .bind(now)
            .bind(id)
            .execute(pool)
            .await?;
    }

    info!("SSH connection updated: id={}", id);
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssh_connection_update",
        &format!("id={}", id),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "更新成功" })))
}

pub async fn delete_connection(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Path(id): Path<i64>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;

    let result = sqlx::query("DELETE FROM ssh_connections WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "连接不存在".to_string()));
    }

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssh_connection_delete",
        &format!("id={}", id),
        "",
    )
    .await;

    info!("SSH connection deleted: id={}", id);
    Ok(Json(json!({ "code": 0, "message": "删除成功" })))
}

// ── WebSocket SSH terminal ─────────────────────────────────

pub async fn ws_terminal(
    ws: WebSocketUpgrade,
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Validate token from query parameter (browser WebSocket doesn't support custom headers)
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
    let claims = match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secure_key.as_ref()),
        &Validation::default(),
    ) {
        Ok(d) => d.claims,
        Err(_) => {
            return axum::response::Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(axum::body::Body::from("Invalid token"))
                .unwrap();
        }
    };
    // 演示账号仅支持浏览，禁止通过终端执行命令
    if crate::zap::jwt::is_demo(&claims) {
        return axum::response::Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(axum::body::Body::from("演示账号仅支持浏览，不能使用终端"))
            .unwrap();
    }

    let rows: u32 = params
        .get("rows")
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    let cols: u32 = params
        .get("cols")
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);

    ws.on_upgrade(move |socket| handle_terminal(socket, id, rows, cols))
}

async fn handle_terminal(socket: WebSocket, conn_id: i64, rows: u32, cols: u32) {
    info!("Terminal WebSocket connected for connection {}", conn_id);

    let conn_info = match load_connection_info(conn_id).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to load connection {}: {}", conn_id, e);
            return;
        }
    };

    let addr = format!("{}:{}", conn_info.host, conn_info.port);
    let tcp = match TcpStream::connect(&addr) {
        Ok(tcp) => tcp,
        Err(e) => {
            error!("Failed to connect to {}: {}", addr, e);
            send_error_and_close(socket, &format!("连接失败: {}\r\n", e)).await;
            return;
        }
    };

    let mut session = match Session::new() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create SSH session: {}", e);
            send_error_and_close(socket, &format!("创建 SSH 会话失败: {}\r\n", e)).await;
            return;
        }
    };

    session.set_tcp_stream(tcp);
    // Handshake must run in blocking mode
    if let Err(e) = session.handshake() {
        error!("SSH handshake failed: {}", e);
        send_error_and_close(socket, &format!("SSH 握手失败: {}\r\n", e)).await;
        return;
    }

    // 密码认证但未保存密码（如默认的 localhost 连接）：先等待前端通过 WebSocket
    // 下发本次会话的临时密码 {"type":"auth","password":"..."}，认证后即丢弃、不落库
    let mut temporary_password: Option<String> = None;
    let need_ask_password = conn_info.auth_type == "password" && conn_info.password.is_empty();
    let (mut ws_tx, mut ws_rx) = if need_ask_password {
        let (mut tx, mut rx) = socket.split();
        let _ = tx
            .send(Message::Text(axum::extract::ws::Utf8Bytes::from(
                "\x1b[33m需要 SSH 密码，请在弹窗中输入（仅本次会话使用，不会保存）\x1b[0m\r\n",
            )))
            .await;
        let pwd = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                match rx.next().await {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(auth) = serde_json::from_str::<AuthMsg>(t.as_ref())
                            && auth.kind == "auth"
                            && !auth.password.is_empty()
                        {
                            break auth.password;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break String::new(),
                    _ => continue,
                }
            }
        })
        .await
        .unwrap_or_default();
        if pwd.is_empty() {
            let _ = tx
                .send(Message::Text(axum::extract::ws::Utf8Bytes::from(
                    "密码输入超时或已取消，连接已关闭\r\n",
                )))
                .await;
            let _ = tx.close().await;
            return;
        }
        temporary_password = Some(pwd);
        (tx, rx)
    } else {
        socket.split()
    };

    // 认证：临时密码优先（空密码连接由前端下发），否则使用库中保存的凭据
    let mut auth_info = conn_info;
    if let Some(pwd) = temporary_password {
        auth_info.password = pwd;
    }
    if let Err(e) = authenticate(&mut session, &auth_info) {
        error!("SSH authentication failed: {}", e);
        let _ = ws_tx
            .send(Message::Text(axum::extract::ws::Utf8Bytes::from(format!(
                "认证失败: {}\r\n",
                e
            ))))
            .await;
        let _ = ws_tx.close().await;
        return;
    }

    let mut channel = match session.channel_session() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to open SSH channel: {}", e);
            let _ = ws_tx
                .send(Message::Text(axum::extract::ws::Utf8Bytes::from(format!(
                    "打开通道失败: {}\r\n",
                    e
                ))))
                .await;
            let _ = ws_tx.close().await;
            return;
        }
    };

    if let Err(e) = channel.request_pty("xterm-256color", None, Some((cols, rows, 0, 0))) {
        error!("Failed to request PTY: {}", e);
        let _ = ws_tx
            .send(Message::Text(axum::extract::ws::Utf8Bytes::from(format!(
                "请求 PTY 失败: {}\r\n",
                e
            ))))
            .await;
        let _ = ws_tx.close().await;
        return;
    }

    if let Err(e) = channel.shell() {
        error!("Failed to start shell: {}", e);
        let _ = ws_tx
            .send(Message::Text(axum::extract::ws::Utf8Bytes::from(format!(
                "启动 shell 失败: {}\r\n",
                e
            ))))
            .await;
        let _ = ws_tx.close().await;
        return;
    }

    // Now switch to non-blocking for the interactive I/O loop
    session.set_blocking(false);

    info!("SSH terminal started for connection {}", conn_id);

    // Channel: SSH read → WebSocket
    let (ssh_read_tx, mut ssh_read_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    // Channel: WebSocket → SSH write
    let (ssh_write_tx, mut ssh_write_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    // Channel: WebSocket → PTY resize (cols, rows)
    let (resize_tx, mut resize_rx) = tokio::sync::mpsc::channel::<(u32, u32)>(16);

    // Spawn blocking SSH I/O task (owns the channel, no mutex needed)
    let ssh_handle = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        let mut write_buf: Vec<u8> = Vec::new();
        loop {
            // Apply any pending PTY resize requests (window-change)
            while let Ok((cols, rows)) = resize_rx.try_recv() {
                if let Err(e) = channel.request_pty_size(cols, rows, None, None) {
                    warn!("PTY resize to {}x{} failed: {}", cols, rows, e);
                }
            }
            // Try to read from SSH
            match channel.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if ssh_read_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    warn!("SSH read error: {}", e);
                    break;
                }
            }

            // Drain all pending writes
            while let Ok(data) = ssh_write_rx.try_recv() {
                write_buf.extend_from_slice(&data);
            }
            if !write_buf.is_empty() {
                match channel.write_all(&write_buf) {
                    Ok(()) => {
                        write_buf.clear();
                        let _ = channel.flush();
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        warn!("SSH write error: {}", e);
                        break;
                    }
                }
            }

            // Avoid busy-waiting
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // Cleanup
        let _ = channel.close();
        let _ = channel.wait_close();
    });

    // Forward SSH reads to WebSocket
    let ws_forward = tokio::spawn(async move {
        while let Some(data) = ssh_read_rx.recv().await {
            if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    // Forward WebSocket writes to SSH，并识别 resize 控制消息
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Binary(d) => {
                if ssh_write_tx.send(d.to_vec()).await.is_err() {
                    break;
                }
            }
            Message::Text(t) => {
                let txt = t.as_ref();
                // resize 控制消息：前端 fit 后自动同步窗口尺寸
                if let Ok(resize) = serde_json::from_str::<ResizeMsg>(txt)
                    && resize.kind == "resize"
                    && resize.cols > 0
                    && resize.rows > 0
                {
                    if resize_tx.send((resize.cols, resize.rows)).await.is_err() {
                        break;
                    }
                    continue;
                }
                // 其余文本一律作为终端输入
                if ssh_write_tx.send(txt.as_bytes().to_vec()).await.is_err() {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => continue,
        }
    }

    // Cleanup
    ws_forward.abort();
    drop(ssh_write_tx); // signal SSH task to stop
    let _ = ssh_handle.await;
    info!("Terminal WebSocket closed for connection {}", conn_id);
}

async fn send_error_and_close(socket: WebSocket, msg: &str) {
    let (mut sender, _) = socket.split();
    let _ = sender
        .send(Message::Text(axum::extract::ws::Utf8Bytes::from(
            msg.to_string(),
        )))
        .await;
    let _ = sender.close().await;
}

// ── Authentication ─────────────────────────────────────────

fn authenticate(session: &mut Session, info: &ConnectionInfo) -> Result<(), String> {
    match info.auth_type.as_str() {
        "password" => {
            session
                .userauth_password(&info.username, &info.password)
                .map_err(|e| format!("密码认证失败: {}", e))?;
        }
        "key" => {
            let key_path = get_key_path(&info.ssh_key_name);
            if key_path.is_none() {
                return Err(format!("SSH 密钥 '{}' 不存在", info.ssh_key_name));
            }
            let key_content = std::fs::read_to_string(key_path.unwrap())
                .map_err(|e| format!("读取密钥文件失败: {}", e))?;
            // 显式传入公钥，避免 libssh2 从 OpenSSH 私钥格式推导公钥的兼容性问题
            let pub_content = get_pub_key_content(&info.ssh_key_name)
                .ok_or_else(|| format!("公钥 '{}' 不存在", info.ssh_key_name))?;
            session
                .userauth_pubkey_memory(&info.username, Some(&pub_content), &key_content, None)
                .map_err(|e| format!("密钥认证失败: {}", e))?;
        }
        _ => return Err(format!("不支持的认证类型: {}", info.auth_type)),
    }
    if !session.authenticated() {
        return Err("认证失败".to_string());
    }
    Ok(())
}

struct ConnectionInfo {
    host: String,
    port: i32,
    username: String,
    auth_type: String,
    password: String,
    ssh_key_name: String,
}

/// WebSocket 终端 resize 控制消息（前端 fit 后发送），形如
/// `{"type":"resize","cols":120,"rows":40}`。
/// 收到后调用 `Channel::request_pty_size` 向远端发送 window-change，用于动态调整 pty 窗口。
#[derive(Debug, Deserialize)]
struct ResizeMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    cols: u32,
    #[serde(default)]
    rows: u32,
}

/// WebSocket 密码下发消息（未保存密码的连接），形如
/// `{"type":"auth","password":"..."}`，由前端弹窗输入后发送，
/// 仅用于本次会话认证，不会写入数据库。
#[derive(Debug, Deserialize)]
struct AuthMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    password: String,
}

async fn load_connection_info(id: i64) -> Result<ConnectionInfo, ZapError> {
    let pool = db::get_db_pool().await;
    let row = sqlx::query(
        "SELECT host, port, username, auth_type, password, ssh_key_name
         FROM ssh_connections WHERE id = ? AND status = 1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let stored: String = r.try_get("password").unwrap_or_default();
            Ok(ConnectionInfo {
                host: r.get("host"),
                port: r.get("port"),
                username: r.get("username"),
                auth_type: r.get("auth_type"),
                // 解密后用于 SSH 认证；旧明文数据同样兼容
                password: crypto::decrypt_password(&stored),
                ssh_key_name: r.try_get("ssh_key_name").unwrap_or_default(),
            })
        }
        None => Err(ZapError::New(-1, "连接不存在或已禁用".to_string())),
    }
}

fn get_key_path(key_name: &str) -> Option<std::path::PathBuf> {
    // 密钥由 zapexec 写入 /etc/zap/ssh（root:zapadm 0640），zapd 以 zapadm 身份直接读取
    let ssh_dir = std::path::PathBuf::from(zap_proto::SSH_KEY_DIR);

    let key_path = ssh_dir.join(key_name);
    if key_path.exists() {
        return Some(key_path);
    }
    None
}

/// 读取公钥内容（优先读 .pub 文件，缺失时用 ssh-keygen 从私钥推导）
fn get_pub_key_content(key_name: &str) -> Option<String> {
    let ssh_dir = std::path::PathBuf::from(zap_proto::SSH_KEY_DIR);
    let pub_path = ssh_dir.join(format!("{key_name}.pub"));
    if pub_path.exists()
        && let Ok(content) = std::fs::read_to_string(&pub_path)
    {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    // 兜底：从私钥推导公钥
    let key_path = ssh_dir.join(key_name);
    if key_path.exists()
        && let Ok(out) = std::process::Command::new("ssh-keygen")
            .args(["-y", "-f"])
            .arg(&key_path)
            .output()
        && out.status.success()
    {
        let trimmed = String::from_utf8_lossy(&out.stdout);
        let trimmed = trimmed.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

// ── Test connection ────────────────────────────────────────

#[derive(Deserialize)]
pub struct TestConnectionQuery {
    pub id: i64,
}

pub async fn test_connection(
    _claims: ValidatedClaims,
    Query(params): Query<TestConnectionQuery>,
) -> ZapJsonResult {
    let conn_info = load_connection_info(params.id).await?;
    let addr = format!("{}:{}", conn_info.host, conn_info.port);

    let tcp = match TcpStream::connect(&addr) {
        Ok(tcp) => tcp,
        Err(e) => {
            return Ok(Json(
                json!({ "code": 0, "success": false, "message": format!("TCP 连接失败: {}", e) }),
            ));
        }
    };

    let mut session =
        Session::new().map_err(|e| ZapError::Error(format!("创建 SSH 会话失败: {}", e)))?;

    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| ZapError::Error(format!("SSH 握手失败: {}", e)))?;

    match authenticate(&mut session, &conn_info) {
        Ok(()) => Ok(Json(
            json!({ "code": 0, "success": true, "message": "连接成功" }),
        )),
        Err(e) => Ok(Json(json!({ "code": 0, "success": false, "message": e }))),
    }
}

// ── Push public key to host ────────────────────────────────

#[derive(Deserialize)]
pub struct PushKeyPayload {
    pub password: Option<String>,
}

/// 表单直推请求（添加/编辑对话框中「推送公钥」用，连接无需先入库）
#[derive(Deserialize)]
pub struct PushKeyRequest {
    pub host: String,
    pub port: i32,
    pub username: String,
    pub ssh_key_name: String,
    pub password: Option<String>,
}

/// 判断目标主机是否为本地回环（localhost / 127.0.0.1 / ::1）
fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().trim_matches(['[', ']']).to_lowercase();
    h == "localhost" || h == "127.0.0.1" || h == "::1"
}

/// 把连接绑定的公钥推送到主机 ~/.ssh/authorized_keys
///
/// - 本地回环（localhost/127.0.0.1）：直接写入本机系统用户 authorized_keys，
///   需要 root 特权（经 zapexec），仅 admin 角色可操作，无需密码。
/// - 远程主机：使用远程密码做一次性认证（SFTP 写入，密码不保存）。
pub async fn push_key_to_host(
    claims: ValidatedClaims,
    Path(id): Path<i64>,
    Json(payload): Json<PushKeyPayload>,
) -> ZapJsonResult {
    let conn_info = load_connection_info(id).await?;
    if conn_info.auth_type != "key" {
        return Err(ZapError::New(
            -1,
            "仅密钥认证的连接支持推送公钥".to_string(),
        ));
    }
    if conn_info.ssh_key_name.is_empty() {
        return Err(ZapError::New(-1, "连接未绑定 SSH 密钥".to_string()));
    }
    push_key_core(
        &claims,
        &conn_info.host,
        conn_info.port,
        &conn_info.username,
        &conn_info.ssh_key_name,
        payload.password.as_deref(),
        format!(
            "{}@{}:{}",
            conn_info.username, conn_info.host, conn_info.port
        ),
    )
    .await
}

/// 表单直推（连接尚未入库也可用，供添加/编辑对话框中的「推送公钥」按钮调用）
pub async fn push_key_direct(
    claims: ValidatedClaims,
    Json(payload): Json<PushKeyRequest>,
) -> ZapJsonResult {
    let host = payload.host.trim().trim_matches(['[', ']']).to_string();
    if host.is_empty() {
        return Err(ZapError::New(-1, "主机地址不能为空".to_string()));
    }
    if payload.username.trim().is_empty() {
        return Err(ZapError::New(-1, "用户名不能为空".to_string()));
    }
    if payload.ssh_key_name.is_empty() {
        return Err(ZapError::New(-1, "请选择要推送的 SSH 密钥".to_string()));
    }
    push_key_core(
        &claims,
        &host,
        payload.port,
        &payload.username,
        &payload.ssh_key_name,
        payload.password.as_deref(),
        format!("{}@{}:{}", payload.username, host, payload.port),
    )
    .await
}

/// 推送核心实现：
/// - 本地回环（localhost/127.0.0.1）走 zapexec 写本机系统用户 authorized_keys（仅 admin，无需密码）
/// - 远程主机用密码做一次性认证，经 SFTP 追加公钥
async fn push_key_core(
    claims: &ValidatedClaims,
    host: &str,
    port: i32,
    username: &str,
    ssh_key_name: &str,
    password: Option<&str>,
    target: String,
) -> ZapJsonResult {
    // 本地回环主机：root 特权写本机 authorized_keys，仅 admin
    if is_loopback_host(host) {
        if !crate::zap::jwt::is_admin(claims) {
            return Err(ZapError::New(
                403,
                "仅 admin 角色可以写入本机 SSH 授权".to_string(),
            ));
        }
        let resp = crate::zapexec::call(Request::SshKeyInstallLocal {
            username: username.to_string(),
            key_name: ssh_key_name.to_string(),
        })
        .await?;
        if resp.code != 0 {
            return Err(ZapError::New(resp.code, resp.message));
        }
        audit::log(
            Some(claims),
            None,
            "push_key_local",
            &target,
            "将公钥写入本机用户 authorized_keys",
        )
        .await;
        return Ok(Json(json!({ "code": 0, "message": resp.message })));
    }

    let pub_content = get_pub_key_content(ssh_key_name)
        .ok_or_else(|| ZapError::New(-1, format!("公钥 '{}' 不存在", ssh_key_name)))?;

    let addr = format!("{}:{}", host, port);
    let tcp =
        TcpStream::connect(&addr).map_err(|e| ZapError::Error(format!("TCP 连接失败: {}", e)))?;
    let mut session =
        Session::new().map_err(|e| ZapError::Error(format!("创建 SSH 会话失败: {}", e)))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|e| ZapError::Error(format!("SSH 握手失败: {}", e)))?;
    let password = password.ok_or_else(|| ZapError::New(-1, "远程主机密码不能为空".to_string()))?;
    session
        .userauth_password(username, password)
        .map_err(|e| ZapError::Error(format!("远程主机密码认证失败: {}", e)))?;
    if !session.authenticated() {
        return Err(ZapError::New(-1, "远程主机密码认证失败".to_string()));
    }

    // 通过 SFTP 写入 ~/.ssh/authorized_keys
    let sftp = session
        .sftp()
        .map_err(|e| ZapError::Error(format!("SFTP 初始化失败: {}", e)))?;
    let home = sftp
        .realpath(std::path::Path::new("."))
        .map_err(|e| ZapError::Error(format!("获取用户主目录失败: {}", e)))?;
    let ssh_dir = home.join(".ssh");
    if sftp.stat(&ssh_dir).is_err() {
        sftp.mkdir(&ssh_dir, 0o700)
            .map_err(|e| ZapError::Error(format!("创建远程 ~/.ssh 失败: {}", e)))?;
    }
    let auth_path = ssh_dir.join("authorized_keys");

    // 已存在且包含该公钥则跳过
    if sftp.stat(&auth_path).is_ok()
        && let Ok(mut f) = sftp.open(&auth_path)
    {
        let mut content = String::new();
        if f.read_to_string(&mut content).is_ok()
            && content.lines().any(|l| l.trim() == pub_content)
        {
            return Ok(Json(
                json!({ "code": 0, "message": "公钥已存在于远程主机，无需重复推送" }),
            ));
        }
    }

    // 追加公钥（文件不存在则创建，权限 0600）
    let mut f = sftp
        .open_mode(
            &auth_path,
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::APPEND,
            0o600,
            OpenType::File,
        )
        .map_err(|e| ZapError::Error(format!("打开远程 authorized_keys 失败: {}", e)))?;
    f.write_all(pub_content.as_bytes())
        .map_err(|e| ZapError::Error(format!("写入远程 authorized_keys 失败: {}", e)))?;
    f.write_all(b"\n").ok();
    drop(f);

    audit::log(
        Some(claims),
        None,
        "push_key",
        &target,
        "推送公钥到远程主机 authorized_keys",
    )
    .await;

    Ok(Json(
        json!({ "code": 0, "message": "公钥已推送到远程主机 ~/.ssh/authorized_keys" }),
    ))
}
