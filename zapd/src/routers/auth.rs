use axum::{ Extension, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{query_as, Pool, Sqlite};
use crate::{db, zap::{self, ZapError, ZapJsonResult}};
use bcrypt::{self};
// use crate::zap;

#[derive(Debug,Deserialize,Serialize,Clone)]
pub struct UserLoginData {
    pub username : String,
    pub password : String
}

pub async fn login(Extension(conn) : Extension<Pool<Sqlite>>,Json(playload) : Json<UserLoginData>)  -> ZapJsonResult {
    // info!("json {:?}", playload);
    let record: Result<db::models::UserModel, sqlx::Error> = query_as("select * from user where username = ?")
    .bind(playload.username.to_string())
    .fetch_one(&conn).await;
    if let Ok(row) = record {
       if let Ok(v) =  bcrypt::verify(playload.password.to_string(), &row.password) && v == true {
            if let Ok(token) = zap::jwt::generate_jwt_token(row.username,row.id) {
                return Ok(Json(json!({
                    "code":0,
                    "access_token": token,
                    "token_type":"Bearer",
                    "message":"登陆成功"
                })));    
            }
            
       }
    }
    return Err(ZapError::New(-1, "用户名或密码错误".to_string()));
    
}

