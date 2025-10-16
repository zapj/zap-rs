use axum::Json;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite};

use crate::{db, zap::{jwt, ZapJsonResult}};


#[derive(sqlx::FromRow, Debug)]
struct UserInfo {
    id: i64,
    username: String,
    email: String,
    nickname: String,
    last_login_ip: String,
    last_login_time: i64,
    status: i32,
    roles: String,
    permissions: String,
    created_at: i64,
    updated_at: i64,
}

pub async fn user_info(claims:jwt::Claims) -> Json<Value>{
    // info!("{:?}",claims);
    let uid = claims.id;
    let pool = db::get_db_pool().await;
    let result:Result<UserInfo,sqlx::Error> = sqlx::query_as("select * from user where id= ?")
    .bind(uid as i64)
    .fetch_one(pool).await;
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
                "roles": user.roles.split(',').collect::<Vec<&str>>(),
                "permissions":user.permissions.split(',').collect::<Vec<&str>>(),
            }
        }));
    }
    return Json(json!({
        "code":-1,
        "message":"User not found",
    }));
}



pub async fn user_list(claims:jwt::Claims) ->  ZapJsonResult {
    let _uid: u64 = claims.id;
    let pool = db::get_db_pool().await;
    
    let mut querybuilder   :QueryBuilder<'_, Sqlite>  = QueryBuilder::new("SELECT id,username,email,nickname,last_login_ip,last_login_time,status,roles,permissions,created_at,updated_at FROM user");
    querybuilder.push(" order by id desc");
    // let query = querybuilder.build_query_as::<UserInfo>();
    let users: Vec<UserInfo> = querybuilder.build_query_as().fetch_all(pool).await?;
    
    return Ok(Json(json!({
        "code":0,
        "message":"OK",
        "data": users.iter().map(|user|{
            json!({
                "id": user.id,
                "username":user.username,
                "email": user.email,
                "nickname": user.nickname,
                "last_login_ip": user.last_login_ip,
                "last_login_time": user.last_login_time,
                "status": user.status,
                "roles": user.roles.split(',').collect::<Vec<&str>>(),
                "permissions": user.permissions.split(',').collect::<Vec<&str>>(),
                "created_at": user.created_at,
                "updated_at": user.updated_at,
            })
        }).collect::<Vec<Value>>(),
        "total": users.len(),
    })));
    
    // return Err(ZapError::New(-1, "User not found".to_string()));
    
}
