use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug,Deserialize,Serialize)]
pub struct UserLoginData {
    pub username : String,
    pub password : String
}

pub async fn login(Json(playload) : Json<UserLoginData>) {
    info!("json {:?}", playload);
}