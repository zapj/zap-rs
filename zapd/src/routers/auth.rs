use axum::{ Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{query,Pool, Sqlite};
use tracing::info;

// use crate::zap;

#[derive(Debug,Deserialize,Serialize,Clone)]
pub struct UserLoginData {
    pub username : String,
    pub password : String
}

pub async fn login(Extension(conn) : Extension<Pool<Sqlite>>,Json(playload) : Json<UserLoginData>)  -> Json<Value> {
    info!("json {:?}", playload);
    let rs = query("select * from user where username = ?")
    .bind(playload.username)
    .fetch_optional(&conn).await;
    if rs.is_err() {
        return Json(json!({
            "code":1,
            "message":"ok"
        }));    
    }
    Json(json!({
        "code":1,
        "message":"ok"
    }))
}