use std::time::{self, UNIX_EPOCH};

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json, RequestPartsExt,
};
use axum_extra::{headers::{authorization::Bearer, Authorization}, TypedHeader};
use jsonwebtoken::{decode, encode, errors::{Error, ErrorKind}, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

use crate::config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub id : u64, // uid
    pub iat: u64, // 签发时间
    pub sub: String, //签发给
    pub iss: String, //发布者
    pub exp: u64, // 过期时间
}

#[derive(Debug)]
pub enum AuthError {
    ExpiredSignature,
    WrongCredentials,
    MissingCredentials,
    TokenCreation,
    InvalidToken,
}


#[derive(Debug, Serialize)]
pub struct AuthBody {
    access_token: String,
    token_type: String,
}




pub fn generate_jwt_token(username : String,id : u64) -> Result<String, Error> {
    let expire = &config::get_config().read().unwrap().jwt.jwt_expire;
    let now_secs = time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let claims = Claims {
        iat: now_secs,
        sub: username,
        iss:"Zap".to_string(),
        id: id,
        exp: now_secs + *expire,
    };
    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    return encode(&Header::default(), &claims, &EncodingKey::from_secret(secure_key.as_ref()));
}


impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract the token from the authorization header
        
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| AuthError::MissingCredentials)?;

        
        // Decode the user data
        let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
        info!("secure_key {} ",secure_key);
        let token_data = decode::<Claims>(bearer.token(), &DecodingKey::from_secret(secure_key.as_ref()), &Validation::default())
            .map_err(|e| {
                let a = e.kind();
                if  *a == ErrorKind::ExpiredSignature {
                    return AuthError::ExpiredSignature;
                }
                AuthError::InvalidToken
            })?;

        Ok(token_data.claims)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TokenCreation => (StatusCode::INTERNAL_SERVER_ERROR, "Token creation error"),
            AuthError::InvalidToken => (StatusCode::BAD_REQUEST, "Invalid token"),
            AuthError::ExpiredSignature => (StatusCode::BAD_REQUEST, "Expired Signature")
        };
        let body = Json(json!({
            "code": -1,
            "message": error_message,
        }));
        (status, body).into_response()
    }
}