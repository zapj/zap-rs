use std::time::{self, UNIX_EPOCH};

use axum::{
    Json, RequestPartsExt,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use jsonwebtoken::{
    DecodingKey, EncodingKey, Header, Validation, decode, encode,
    errors::{Error, ErrorKind},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use sha2::{Digest, Sha256};

use crate::{config, db};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: u64,       // uid
    pub iat: u64,      // 签发时间
    pub sub: String,   // 签发给
    pub iss: String,   // 发布者
    pub exp: u64,      // 过期时间
    pub roles: String, // 用户角色，逗号分隔
    #[serde(default)]
    pub pwd_is_default: bool, // 是否仍在使用默认密码
}

/// Wrapper around Claims that rejects requests if password hasn't been changed
/// from the default. Use this for all endpoints except login/logout/health/change-password.
pub struct ValidatedClaims(pub Claims);

impl std::ops::Deref for ValidatedClaims {
    type Target = Claims;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub enum AuthError {
    ExpiredSignature,
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
    InvalidToken,
    MustChangePassword,
}

#[derive(Debug, Serialize)]
pub struct AuthBody {
    access_token: String,
    token_type: String,
}

pub fn generate_jwt_token(
    username: String,
    id: u64,
    roles: &str,
    pwd_is_default: bool,
) -> Result<String, Error> {
    let expire = &config::get_config().read().unwrap().jwt.jwt_expire;
    let now_secs = time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = Claims {
        iat: now_secs,
        sub: username,
        iss: "Zap".to_string(),
        id,
        exp: now_secs + *expire,
        roles: roles.to_string(),
        pwd_is_default,
    };
    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secure_key.as_ref()),
    )
}

/// Check if the claims contain the admin role
pub fn is_admin(claims: &Claims) -> bool {
    claims.roles.split(',').any(|r| r.trim() == "admin")
}

/// Check if the claims contain the reseller role
pub fn is_reseller(claims: &Claims) -> bool {
    claims.roles.split(',').any(|r| r.trim() == "reseller")
}

/// Check if the claims contain the demo role（只读演示账号）
pub fn is_demo(claims: &Claims) -> bool {
    claims.roles.split(',').any(|r| r.trim() == "demo")
}

/// 静态 API Token 前缀（`zap_` 开头，用于区分 JWT）
pub const API_TOKEN_PREFIX: &str = "zap_";

/// SHA-256 十六进制摘要（API Token 在 DB 中仅存哈希，不落明文）
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// 解析 Bearer 凭据字符串为 claims（支持 JWT 与静态 API Token，供只读守卫等中间件使用）。
/// 入参应为已剥离 `Bearer ` 前缀的 token，调用方需在 await 前持有 owned String，避免借用跨 await。
pub async fn claims_from_token(raw_token: &str) -> Option<Claims> {
    if raw_token.starts_with(API_TOKEN_PREFIX) {
        return resolve_api_token(raw_token).await;
    }
    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    decode::<Claims>(
        raw_token,
        &DecodingKey::from_secret(secure_key.as_ref()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

// ── Claims extractor (allows default-password users through) ────────────────

impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        extract_claims(parts).await
    }
}

// ── ValidatedClaims extractor (rejects default-password users) ─────────────

impl<S> FromRequestParts<S> for ValidatedClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let claims = extract_claims(parts).await?;

        if claims.pwd_is_default {
            return Err(AuthError::MustChangePassword);
        }

        Ok(ValidatedClaims(claims))
    }
}

async fn extract_claims(parts: &mut Parts) -> Result<Claims, AuthError> {
    let TypedHeader(Authorization(bearer)) = parts
        .extract::<TypedHeader<Authorization<Bearer>>>()
        .await
        .map_err(|_| AuthError::MissingCredentials)?;

    let raw = bearer.token();

    // 静态 API Token（zap_ 前缀）→ 数据库校验
    if raw.starts_with(API_TOKEN_PREFIX) {
        return match resolve_api_token(raw).await {
            Some(claims) => Ok(claims),
            None => Err(AuthError::InvalidToken),
        };
    }

    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    let token_data = decode::<Claims>(
        raw,
        &DecodingKey::from_secret(secure_key.as_ref()),
        &Validation::default(),
    )
    .map_err(|e| {
        if *e.kind() == ErrorKind::ExpiredSignature {
            return AuthError::ExpiredSignature;
        }
        AuthError::InvalidToken
    })?;

    Ok(token_data.claims)
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
            AuthError::ExpiredSignature => (StatusCode::UNAUTHORIZED, "Token 已过期，请重新登录"),
            AuthError::MustChangePassword => {
                (StatusCode::FORBIDDEN, "请先修改默认密码后再进行操作")
            }
        };
        let body = Json(json!({
            "code": -1,
            "message": error_message,
        }));
        (status, body).into_response()
    }
}

// ── 静态 API Token 校验 ─────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ApiTokenLookup {
    user_id: i64,
    username: String,
    roles: String,
    token_status: i64,
    user_status: i64,
    expires_at: i64,
}

/// 校验静态 API Token：按哈希查表，校验 Token/用户状态与有效期，返回等价 Claims。
async fn resolve_api_token(raw: &str) -> Option<Claims> {
    let pool = db::get_db_pool().await;
    let row: Option<ApiTokenLookup> = sqlx::query_as(
        "SELECT t.user_id, u.username, u.roles, t.status AS token_status,
                u.status AS user_status, t.expires_at
         FROM api_token t JOIN user u ON u.id = t.user_id
         WHERE t.token_hash = ?",
    )
    .bind(sha256_hex(raw))
    .fetch_optional(pool)
    .await
    .ok()?;
    let r = row?;

    let now = time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    if r.token_status != 1 || r.user_status != 1 || (r.expires_at > 0 && r.expires_at <= now) {
        return None;
    }

    // 尽力更新最近使用时间（失败不阻断请求）
    let pool = db::get_db_pool().await;
    let _ = sqlx::query("UPDATE api_token SET last_used_at = ? WHERE token_hash = ?")
        .bind(now)
        .bind(sha256_hex(raw))
        .execute(pool)
        .await;

    let exp = if r.expires_at > 0 {
        r.expires_at as u64
    } else {
        // 永不过期的 Token：赋予足够远的 exp（10 年）
        now as u64 + 10 * 365 * 86400
    };
    Some(Claims {
        id: r.user_id as u64,
        iat: now as u64,
        sub: r.username,
        iss: "Zap".to_string(),
        exp,
        roles: r.roles,
        pwd_is_default: false,
    })
}
