use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::query_as;
use crate::{db, zap::{self, ZapError, ZapJsonResult}};
use bcrypt::{self};

#[derive(Debug,Deserialize,Serialize,Clone)]
pub struct UserLoginData {
    pub username : String,
    pub password : String
}

pub async fn login(Json(playload) : Json<UserLoginData>)  -> ZapJsonResult {
    // info!("json {:?}", playload);
    let pool = db::get_db_pool().await;
    let record: Result<db::models::UserModel, sqlx::Error> = query_as("select * from user where username = ?")
    .bind(playload.username.to_string())
    .fetch_one(pool).await;
    if let Ok(row) = record {
       if let Ok(v) =  bcrypt::verify(playload.password.to_string(), &row.password) && v == true {
            if let Ok(token) = zap::jwt::generate_jwt_token(row.username,row.id) {
                return Ok(Json(json!({
                    "code":0,
                    "access_token": token,
                    "token_type":"Bearer",
                    "message":"登陆成功",
                    "expire_in": crate::config::get_config().read().unwrap().jwt.jwt_expire
                })));    
            }
            
       }
    }
    return Err(ZapError::New(-1, "用户名或密码错误".to_string()));
    
}

pub async fn logout() -> ZapJsonResult {
    Ok(Json(json!({
        "code":0,
        "message":"退出成功"
    })))
}


pub async fn reflash_token(claims:zap::jwt::Claims) -> ZapJsonResult {
    if let Ok(token) = zap::jwt::generate_jwt_token(claims.sub,claims.id) {
        return Ok(Json(json!({
            "code":0,
            "access_token": token,
            "token_type":"Bearer",
            "message":"刷新成功",
            "expire_in": crate::config::get_config().read().unwrap().jwt.jwt_expire
        })));    
    }
    return Err(ZapError::New(-1, "刷新失败".to_string()));
}
