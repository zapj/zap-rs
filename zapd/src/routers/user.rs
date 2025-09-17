use axum::{Extension, Json};
use serde_json::{json, Value};
use sqlx::{Pool, Sqlite};
use tracing::info;

use crate::zap::jwt;


#[derive(sqlx::FromRow)]
struct UserInfo {
    id: i64,
    username: String,
    email: String,
    nickname: String,
    last_login_ip: String,
    last_login_time: i64,
}
pub async fn user_info(claims:jwt::Claims,Extension(pool):Extension<Pool<Sqlite>>) -> Json<Value>{
    info!("{:?}",claims);
    let uid = claims.id;
    let result:Result<UserInfo,sqlx::Error> = sqlx::query_as("select * from user where id= ?").bind(uid as i64).fetch_one(&pool).await;
    if let Ok(user) = result {
        return Json(json!({
            "code":0,
            "message":"OK",
            "data": {
                "id": user.id,
                "username":user.username,
                "email": user.email,
                "nickname": user.nickname,
                "last_login_ip": user.last_login_ip,
                "last_login_time": user.last_login_time,
                "roles": ["admin"],
                "permissions":["*"],
            }
        }));
    }
    return Json(json!({
        "code":-1,
        "message":"User not found",
    }));
}
