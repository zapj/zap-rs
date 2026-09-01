use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::{Extension, Json};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::query_as;
use tracing::warn;

use crate::db;
use crate::zap::{
    self,
    audit,
    jwt::ValidatedClaims,
    totp,
    ZapError, ZapJsonResult,
};

/// Check if the stored password hash is for the default password "123456"
fn is_default_password(stored_hash: &str) -> bool {
    bcrypt::verify("123456", stored_hash).unwrap_or(false)
}

/// Rate limiter state: Maps IP -> (attempt_count, window_start)
static LOGIN_RATE_LIMITER: Lazy<Mutex<HashMap<IpAddr, (u32, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const MAX_LOGIN_ATTEMPTS: u32 = 5;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// 持久化失败锁定策略（针对账号+IP）
const MAX_DB_FAILED_ATTEMPTS: i64 = 5;
const LOCK_DURATION_SECS: i64 = 15 * 60;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserLoginData {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

#[derive(sqlx::FromRow, Debug)]
struct UserRecord {
    id: u64,
    username: String,
    password: String,
    roles: String,
    totp_secret: String,
    totp_enabled: i32,
}

/// Check rate limit for the given IP. Returns Ok(()) if allowed, Err if rate limited.
fn check_rate_limit(ip: IpAddr) -> Result<(), ZapError> {
    let mut map = LOGIN_RATE_LIMITER.lock().unwrap();
    let now = Instant::now();

    let entry = map.entry(ip).or_insert((0, now));

    // Reset window if expired
    if now.duration_since(entry.1) > RATE_LIMIT_WINDOW {
        *entry = (1, now);
        return Ok(());
    }

    entry.0 += 1;
    if entry.0 > MAX_LOGIN_ATTEMPTS {
        warn!("Rate limit exceeded for IP: {}", ip);
        return Err(ZapError::New(
            -1,
            "登录尝试过于频繁，请60秒后再试".to_string(),
        ));
    }

    Ok(())
}

/// 检查持久化锁定（账号+IP 维度，失败 5 次锁定 15 分钟）。
async fn check_db_lock(ip: &str, username: &str) -> Result<(), ZapError> {
    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT locked_until FROM login_attempts
         WHERE username = ? AND ip = ? AND locked_until > ?",
    )
    .bind(username)
    .bind(ip)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    if let Some((until,)) = row {
        let remaining = until - now;
        return Err(ZapError::New(
            -1,
            format!("登录失败次数过多，账号已锁定，请约 {} 分钟后再试", remaining / 60 + 1),
        ));
    }
    Ok(())
}

/// 记录一次登录失败，达到阈值后锁定。
async fn record_failed_login(ip: &str, username: &str) {
    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let lock_until = now + LOCK_DURATION_SECS;
    let _ = sqlx::query(
        r#"INSERT INTO login_attempts (username, ip, failed_count, locked_until, updated_at)
           VALUES (?, ?, 1, 0, ?)
           ON CONFLICT(username, ip) DO UPDATE SET
             failed_count = login_attempts.failed_count + 1,
             locked_until = CASE
               WHEN login_attempts.failed_count + 1 >= ? THEN ?
               ELSE 0
             END,
             updated_at = excluded.updated_at"#,
    )
    .bind(username)
    .bind(ip)
    .bind(now)
    .bind(MAX_DB_FAILED_ATTEMPTS)
    .bind(lock_until)
    .bind(now)
    .execute(pool)
    .await;
}

/// 登录成功后清除失败记录。
async fn clear_login_attempts(ip: &str, username: &str) {
    let pool = db::get_db_pool().await;
    let _ = sqlx::query("DELETE FROM login_attempts WHERE username = ? AND ip = ?")
        .bind(username)
        .bind(ip)
        .execute(pool)
        .await;
}

#[axum::debug_handler]
pub async fn login(
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<UserLoginData>,
) -> ZapJsonResult {
    // 第一道防线：内存滑动窗口限流
    check_rate_limit(client_addr.ip())?;

    let ip = client_addr.ip().to_string();
    let username = payload.username.trim().to_string();
    if username.is_empty() {
        return Err(ZapError::New(-1, "用户名不能为空".to_string()));
    }

    // 第二道防线：持久化失败锁定（账号+IP）
    check_db_lock(&ip, &username).await?;

    let pool = db::get_db_pool().await;
    let record: Result<UserRecord, sqlx::Error> = query_as(
        "SELECT id, username, password, roles, totp_secret, totp_enabled
         FROM user WHERE username = ?",
    )
    .bind(&username)
    .fetch_one(pool)
    .await;

    if let Ok(row) = record {
        if let Ok(true) = bcrypt::verify(payload.password.to_string(), &row.password) {
            // TOTP 两步验证（已启用时校验）
            if row.totp_enabled == 1 {
                let code = payload.totp_code.unwrap_or_default();
                if !totp::verify(&row.totp_secret, &code) {
                    audit::log(None, Some(&ip), "login_2fa_failed", &row.username, "").await;
                    return Err(ZapError::New(-1, "两步验证码错误或已失效".to_string()));
                }
            }

            // Check if using default password
            let is_default = is_default_password(&row.password);

            if let Ok(token) =
                zap::jwt::generate_jwt_token(row.username.clone(), row.id, &row.roles, is_default)
            {
                clear_login_attempts(&ip, &username).await;
                // 更新最后登录信息
                let now = chrono::Local::now().timestamp();
                let _ = sqlx::query(
                    "UPDATE user SET last_login_time = ?, last_login_ip = ? WHERE id = ?",
                )
                .bind(now)
                .bind(&ip)
                .bind(row.id as i64)
                .execute(pool)
                .await;
                audit::log(None, Some(&ip), "login_success", &row.username, "").await;
                return Ok(Json(json!({
                    "code": 0,
                    "access_token": token,
                    "token_type": "Bearer",
                    "message": "登陆成功",
                    "expire_in": crate::config::get_config().read().unwrap().jwt.jwt_expire,
                    "must_change_password": is_default,
                })));
            }
        }
    }
    record_failed_login(&ip, &username).await;
    audit::log(None, Some(&ip), "login_failed", &username, "").await;
    Err(ZapError::New(-1, "用户名或密码错误".to_string()))
}

pub async fn logout() -> ZapJsonResult {
    Ok(Json(json!({
        "code": 0,
        "message": "退出成功"
    })))
}

/// Change password — uses `Claims` (not `ValidatedClaims`) so it works even
/// when the user is still on the default password.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordPayload {
    pub old_password: String,
    pub new_password: String,
}

pub async fn change_password(
    claims: zap::jwt::Claims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<ChangePasswordPayload>,
) -> ZapJsonResult {
    if payload.new_password.len() < 6 {
        return Err(ZapError::New(-1, "新密码长度不能少于6位".to_string()));
    }
    if payload.old_password == payload.new_password {
        return Err(ZapError::New(-1, "新密码不能与旧密码相同".to_string()));
    }

    let pool = db::get_db_pool().await;

    // Verify old password
    let row: Result<(String,), sqlx::Error> =
        sqlx::query_as("SELECT password FROM user WHERE id = ?")
            .bind(claims.id as i64)
            .fetch_one(pool)
            .await;

    match row {
        Ok((stored_hash,)) => {
            if !bcrypt::verify(&payload.old_password, &stored_hash).unwrap_or(false) {
                audit::log(Some(&claims), None, "password_change_failed", &claims.sub, "旧密码错误")
                    .await;
                return Err(ZapError::New(-1, "旧密码错误".to_string()));
            }
        }
        Err(_) => return Err(ZapError::New(-1, "用户不存在".to_string())),
    }

    // Hash new password
    let new_hash = bcrypt::hash(&payload.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| ZapError::Error(format!("密码加密失败: {}", e)))?;

    // Update password
    let now = chrono::Utc::now().timestamp();
    sqlx::query("UPDATE user SET password = ?, updated_at = ? WHERE id = ?")
        .bind(&new_hash)
        .bind(now)
        .bind(claims.id as i64)
        .execute(pool)
        .await?;

    // Issue a new token with pwd_is_default = false
    let token = zap::jwt::generate_jwt_token(
        claims.sub.clone(),
        claims.id,
        &claims.roles,
        false, // no longer default
    )
    .map_err(|_| ZapError::New(-1, "Token 生成失败".to_string()))?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "password_change",
        &claims.sub,
        "",
    )
    .await;

    tracing::info!("Password changed for user {}", claims.sub);
    Ok(Json(json!({
        "code": 0,
        "message": "密码修改成功",
        "access_token": token,
        "token_type": "Bearer",
    })))
}

pub async fn reflash_token(claims: zap::jwt::Claims) -> ZapJsonResult {
    if let Ok(token) =
        zap::jwt::generate_jwt_token(claims.sub, claims.id, &claims.roles, claims.pwd_is_default)
    {
        return Ok(Json(json!({
            "code": 0,
            "access_token": token,
            "token_type": "Bearer",
            "message": "刷新成功",
            "expire_in": crate::config::get_config().read().unwrap().jwt.jwt_expire
        })));
    }
    Err(ZapError::New(-1, "刷新失败".to_string()))
}

// ── TOTP 2FA ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TotpCodePayload {
    pub code: String,
}

/// GET /auth/totp/setup — 生成密钥与 otpauth URL（未启用时）。
pub async fn totp_setup(claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let (enabled, existing): (i32, String) =
        sqlx::query_as("SELECT totp_enabled, totp_secret FROM user WHERE id = ?")
            .bind(claims.id as i64)
            .fetch_one(pool)
            .await?;
    if enabled == 1 {
        return Err(ZapError::New(-1, "两步验证已启用，如需更换请先关闭".to_string()));
    }
    // 复用已有未启用密钥，避免每次打开都更换
    if existing.is_empty() {
        let secret = totp::generate_secret();
        let _ = sqlx::query("UPDATE user SET totp_secret = ? WHERE id = ?")
            .bind(&secret)
            .bind(claims.id as i64)
            .execute(pool)
            .await;
        let url = totp::otpauth_url(&secret, &claims.sub);
        return Ok(Json(json!({ "code": 0, "data": { "secret": secret, "otpauth_url": url } })));
    }
    let url = totp::otpauth_url(&existing, &claims.sub);
    Ok(Json(json!({ "code": 0, "data": { "secret": existing, "otpauth_url": url } })))
}

/// POST /auth/totp/verify — 校验验证码并启用两步验证。
pub async fn totp_verify(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<TotpCodePayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let (enabled, secret): (i32, String) =
        sqlx::query_as("SELECT totp_enabled, totp_secret FROM user WHERE id = ?")
            .bind(claims.id as i64)
            .fetch_one(pool)
            .await?;
    if enabled == 1 {
        return Err(ZapError::New(-1, "两步验证已启用".to_string()));
    }
    if secret.is_empty() {
        return Err(ZapError::New(-1, "请先获取验证密钥".to_string()));
    }
    if !totp::verify(&secret, &payload.code) {
        return Err(ZapError::New(-1, "验证码错误或已失效".to_string()));
    }
    sqlx::query("UPDATE user SET totp_enabled = 1 WHERE id = ?")
        .bind(claims.id as i64)
        .execute(pool)
        .await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "totp_enable",
        &claims.sub,
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "两步验证已启用" })))
}

/// POST /auth/totp/disable — 校验验证码后关闭两步验证。
pub async fn totp_disable(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<TotpCodePayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let (enabled, secret): (i32, String) =
        sqlx::query_as("SELECT totp_enabled, totp_secret FROM user WHERE id = ?")
            .bind(claims.id as i64)
            .fetch_one(pool)
            .await?;
    if enabled == 0 {
        return Err(ZapError::New(-1, "两步验证未启用".to_string()));
    }
    if !totp::verify(&secret, &payload.code) {
        return Err(ZapError::New(-1, "验证码错误或已失效".to_string()));
    }
    sqlx::query("UPDATE user SET totp_enabled = 0, totp_secret = '' WHERE id = ?")
        .bind(claims.id as i64)
        .execute(pool)
        .await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "totp_disable",
        &claims.sub,
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "两步验证已关闭" })))
}

/// GET /auth/totp/status — 查询两步验证开启状态。
pub async fn totp_status(claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let (enabled,): (i32,) = sqlx::query_as("SELECT totp_enabled FROM user WHERE id = ?")
        .bind(claims.id as i64)
        .fetch_one(pool)
        .await?;
    Ok(Json(json!({ "code": 0, "data": { "enabled": enabled == 1 } })))
}
