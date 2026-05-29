use std::time::{self, UNIX_EPOCH};

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json, RequestPartsExt,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{
    decode, encode,
    errors::{Error, ErrorKind},
    DecodingKey, EncodingKey, Header, Validation,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id: u64,             // uid
    pub iat: u64,            // 签发时间
    pub sub: String,         // 签发给
    pub iss: String,         // 发布者
    pub exp: u64,            // 过期时间
    pub roles: String,       // 用户角色，逗号分隔
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

    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    let token_data = decode::<Claims>(
        bearer.token(),
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
            AuthError::TokenCreation => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error")
            }
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
            AuthError::ExpiredSignature => (StatusCode::UNAUTHORIZED, "Token 已过期，请重新登录"),
            AuthError::MustChangePassword => (
                StatusCode::FORBIDDEN,
                "请先修改默认密码后再进行操作",
            ),
        };
        let body = Json(json!({
            "code": -1,
            "message": error_message,
        }));
        (status, body).into_response()
    }
}
