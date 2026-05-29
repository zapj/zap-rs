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
use crate::zap::{self, ZapError, ZapJsonResult};

/// Check if the stored password hash is for the default password "123456"
fn is_default_password(stored_hash: &str) -> bool {
    bcrypt::verify("123456", stored_hash).unwrap_or(false)
}

/// Rate limiter state: Maps IP -> (attempt_count, window_start)
static LOGIN_RATE_LIMITER: Lazy<Mutex<HashMap<IpAddr, (u32, Instant)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const MAX_LOGIN_ATTEMPTS: u32 = 5;
const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserLoginData {
    pub username: String,
    pub password: String,
}

#[derive(sqlx::FromRow, Debug)]
struct UserRecord {
    id: u64,
    username: String,
    password: String,
    roles: String,
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

#[axum::debug_handler]
pub async fn login(
    Extension(client_addr): Extension<SocketAddr>,
    Json(playload): Json<UserLoginData>,
) -> ZapJsonResult {
    // Rate limit check
    check_rate_limit(client_addr.ip())?;

    let pool = db::get_db_pool().await;
    let record: Result<UserRecord, sqlx::Error> =
        query_as("select id, username, password, roles from user where username = ?")
            .bind(playload.username.to_string())
            .fetch_one(pool)
            .await;

    if let Ok(row) = record {
        if let Ok(true) = bcrypt::verify(playload.password.to_string(), &row.password) {
            // Check if using default password
            let is_default = is_default_password(&row.password);

            if let Ok(token) =
                zap::jwt::generate_jwt_token(row.username, row.id, &row.roles, is_default)
            {
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
    Err(ZapError::New(-1, "用户名或密码错误".to_string()))
}

pub async fn logout() -> ZapJsonResult {
    Ok(Json(json!({
        "code": 0,
        "message": "退出成功"
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
