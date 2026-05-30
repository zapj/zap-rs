use std::io::{Read, Write};
use std::net::TcpStream;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Executor, Row};
use ssh2::Session;
use tracing::{error, info, warn};

use crate::config;
use crate::db;
use crate::zap::jwt::{Claims, ValidatedClaims};
use crate::zap::{ZapError, ZapJsonResult};

// ── Database schema ────────────────────────────────────────

pub async fn init_table() {
    if table_exists("ssh_connections").await {
        return;
    }
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
    pub ssh_key_name: String,
    pub remark: String,
    pub status: i32,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_conn(row: &sqlx::sqlite::SqliteRow) -> SshConnection {
    SshConnection {
        id: row.get("id"),
        name: row.get("name"),
        host: row.get("host"),
        port: row.get("port"),
        username: row.get("username"),
        auth_type: row.get("auth_type"),
        password: row.try_get("password").unwrap_or_default(),
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

fn default_port() -> i32 { 22 }
fn default_username() -> String { "root".to_string() }
fn default_auth_type() -> String { "password".to_string() }

// ── CRUD handlers ──────────────────────────────────────────

pub async fn list_connections(_claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let rows = sqlx::query(
        "SELECT id, name, host, port, username, auth_type, password, ssh_key_name,
                remark, status, sort_order, created_at, updated_at
         FROM ssh_connections ORDER BY sort_order, id"
    )
    .fetch_all(pool)
    .await?;

    let connections: Vec<SshConnection> = rows.iter().map(|r| row_to_conn(r)).collect();
    Ok(Json(json!({ "code": 0, "data": connections })))
}

pub async fn get_connection(
    _claims: ValidatedClaims,
    Path(id): Path<i64>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let row = sqlx::query(
        "SELECT id, name, host, port, username, auth_type, password, ssh_key_name,
                remark, status, sort_order, created_at, updated_at
         FROM ssh_connections WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(Json(json!({ "code": 0, "data": row_to_conn(&r) }))),
        None => Err(ZapError::New(-1, "连接不存在".to_string())),
    }
}

pub async fn create_connection(
    _claims: ValidatedClaims,
    Json(payload): Json<CreateConnectionPayload>,
) -> ZapJsonResult {
    if payload.name.trim().is_empty() {
        return Err(ZapError::New(-1, "连接名称不能为空".to_string()));
    }
    if payload.host.trim().is_empty() {
        return Err(ZapError::New(-1, "主机地址不能为空".to_string()));
    }
    if payload.auth_type != "password" && payload.auth_type != "key" {
        return Err(ZapError::New(-1, "认证类型仅支持 password 或 key".to_string()));
    }
    if payload.auth_type == "password" && payload.password.is_empty() {
        return Err(ZapError::New(-1, "密码认证时密码不能为空".to_string()));
    }
    if payload.auth_type == "key" && payload.ssh_key_name.is_empty() {
        return Err(ZapError::New(-1, "密钥认证时必须选择一个 SSH 密钥".to_string()));
    }

    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        "INSERT INTO ssh_connections (name, host, port, username, auth_type, password, ssh_key_name, remark, status, sort_order, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)"
    )
    .bind(payload.name.trim())
    .bind(payload.host.trim())
    .bind(payload.port)
    .bind(payload.username)
    .bind(payload.auth_type)
    .bind(payload.password)
    .bind(payload.ssh_key_name)
    .bind(payload.remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    info!("SSH connection created: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "创建成功" })))
}

pub async fn update_connection(
    _claims: ValidatedClaims,
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
            .bind(&v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.host.as_deref().map(|s| s.trim().to_string()) {
        sqlx::query("UPDATE ssh_connections SET host = ?, updated_at = ? WHERE id = ?")
            .bind(&v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.port {
        sqlx::query("UPDATE ssh_connections SET port = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.username {
        sqlx::query("UPDATE ssh_connections SET username = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.auth_type {
        sqlx::query("UPDATE ssh_connections SET auth_type = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.password {
        sqlx::query("UPDATE ssh_connections SET password = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.ssh_key_name {
        sqlx::query("UPDATE ssh_connections SET ssh_key_name = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.remark {
        sqlx::query("UPDATE ssh_connections SET remark = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.status {
        sqlx::query("UPDATE ssh_connections SET status = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }
    if let Some(v) = payload.sort_order {
        sqlx::query("UPDATE ssh_connections SET sort_order = ?, updated_at = ? WHERE id = ?")
            .bind(v).bind(now).bind(id).execute(pool).await?;
    }

    info!("SSH connection updated: id={}", id);
    Ok(Json(json!({ "code": 0, "message": "更新成功" })))
}

pub async fn delete_connection(
    _claims: ValidatedClaims,
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

    let rows: u32 = params.get("rows").and_then(|v| v.parse().ok()).unwrap_or(24);
    let cols: u32 = params.get("cols").and_then(|v| v.parse().ok()).unwrap_or(80);

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

    if let Err(e) = authenticate(&mut session, &conn_info) {
        error!("SSH authentication failed: {}", e);
        send_error_and_close(socket, &format!("认证失败: {}\r\n", e)).await;
        return;
    }

    let mut channel = match session.channel_session() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to open SSH channel: {}", e);
            send_error_and_close(socket, &format!("打开通道失败: {}\r\n", e)).await;
            return;
        }
    };

    if let Err(e) = channel.request_pty("xterm-256color", None, Some((cols, rows, 0, 0))) {
        error!("Failed to request PTY: {}", e);
        return;
    }

    if let Err(e) = channel.shell() {
        error!("Failed to start shell: {}", e);
        return;
    }

    // Now switch to non-blocking for the interactive I/O loop
    session.set_blocking(false);

    info!("SSH terminal started for connection {}", conn_id);

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Channel: SSH read → WebSocket
    let (ssh_read_tx, mut ssh_read_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
    // Channel: WebSocket → SSH write
    let (ssh_write_tx, mut ssh_write_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);

    // Spawn blocking SSH I/O task (owns the channel, no mutex needed)
    let ssh_handle = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        let mut write_buf: Vec<u8> = Vec::new();
        loop {
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

    // Forward WebSocket writes to SSH
    while let Some(Ok(msg)) = ws_rx.next().await {
        let data: Vec<u8> = match msg {
            Message::Binary(d) => d.to_vec(),
            Message::Text(t) => t.as_bytes().to_vec(),
            Message::Close(_) => break,
            _ => continue,
        };
        if ssh_write_tx.send(data).await.is_err() {
            break;
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
        .send(Message::Text(axum::extract::ws::Utf8Bytes::from(msg.to_string())))
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
            let key_content = std::fs::read_to_string(&key_path.unwrap())
                .map_err(|e| format!("读取密钥文件失败: {}", e))?;
            session
                .userauth_pubkey_memory(&info.username, None, &key_content, None)
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

async fn load_connection_info(id: i64) -> Result<ConnectionInfo, ZapError> {
    let pool = db::get_db_pool().await;
    let row = sqlx::query(
        "SELECT host, port, username, auth_type, password, ssh_key_name
         FROM ssh_connections WHERE id = ? AND status = 1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => Ok(ConnectionInfo {
            host: r.get("host"),
            port: r.get("port"),
            username: r.get("username"),
            auth_type: r.get("auth_type"),
            password: r.try_get("password").unwrap_or_default(),
            ssh_key_name: r.try_get("ssh_key_name").unwrap_or_default(),
        }),
        None => Err(ZapError::New(-1, "连接不存在或已禁用".to_string())),
    }
}

fn get_key_path(key_name: &str) -> Option<std::path::PathBuf> {
    let ssh_dir = std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".ssh"))
        .unwrap_or_else(|_| std::path::PathBuf::from("/root/.ssh"));

    let key_path = ssh_dir.join(key_name);
    if key_path.exists() {
        return Some(key_path);
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
            return Ok(Json(json!({ "code": 0, "success": false, "message": format!("TCP 连接失败: {}", e) })));
        }
    };

    let mut session = Session::new()
        .map_err(|e| ZapError::Error(format!("创建 SSH 会话失败: {}", e)))?;

    session.set_tcp_stream(tcp);
    session.handshake()
        .map_err(|e| ZapError::Error(format!("SSH 握手失败: {}", e)))?;

    match authenticate(&mut session, &conn_info) {
        Ok(()) => Ok(Json(json!({ "code": 0, "success": true, "message": "连接成功" }))),
        Err(e) => Ok(Json(json!({ "code": 0, "success": false, "message": e }))),
    }
}
