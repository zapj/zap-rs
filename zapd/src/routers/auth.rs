use axum::{ Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{query_as, Pool, Sqlite};
use tracing::info;
use crate::db;
use bcrypt::{self, DEFAULT_COST};
// use crate::zap;

#[derive(Debug,Deserialize,Serialize,Clone)]
pub struct UserLoginData {
    pub username : String,
    pub password : String
}

pub async fn login(Extension(conn) : Extension<Pool<Sqlite>>,Json(playload) : Json<UserLoginData>)  -> Json<Value> {
    info!("json {:?}", playload);
    let record: Result<db::models::UserModel, sqlx::Error> = query_as("select * from user where username = ?")
    .bind(playload.username)
    .fetch_one(&conn).await;
    if let Ok(row) = record {
       if let Ok(v) =  bcrypt::verify(playload.password, &row.password) && v == true {
            return Json(json!({
                "code":0,
                "message":"登陆成功"
            }));    
       }
    }
    return Json(json!({
        "code":-1,
        "message":"用户名或密码错误"
    }));
}