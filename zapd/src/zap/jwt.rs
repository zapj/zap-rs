use std::time;

use jsonwebtoken::{encode, errors::Error, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::{config, routers::auth::UserLoginData};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    company: String,
    exp: usize,
}


pub fn generate_jwt_token(playload:&UserLoginData) -> Result<String, Error> {
    let claims = Claims {
        sub: playload.username.to_string(),
        company:"Zap".to_string(),
        exp: time::Duration::from_secs(time::SystemTime::now().elapsed().unwrap().as_secs()).as_secs() as usize
    };
    let secure_key = &config::get_config().read().unwrap().jwt.jwt_secure;
    return encode(&Header::default(), &claims, &EncodingKey::from_secret(secure_key.as_ref()));
    
    // Send the authorized token
    // Ok(Json(AuthBody::new(token)))
}