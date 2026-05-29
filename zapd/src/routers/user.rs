use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite};
use tracing::info;

use crate::{
    db,
    zap::{
        jwt::{self, Claims, ValidatedClaims},
        ZapError, ZapJsonResult,
    },
};

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

#[derive(Debug, Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub email: String,
    pub nickname: Option<String>,
    pub roles: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPayload {
    pub id: i64,
    pub email: Option<String>,
    pub nickname: Option<String>,
    pub roles: Option<String>,
    pub status: Option<i32>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserPayload {
    pub id: i64,
}

/// Require admin role; return error if not admin
fn require_admin(claims: &jwt::Claims) -> Result<(), ZapError> {
    if jwt::is_admin(claims) {
        Ok(())
    } else {
        Err(ZapError::New(-1, "权限不足，需要管理员权限".to_string()))
    }
}

pub async fn user_info(claims: Claims) -> Json<Value> {
    let uid = claims.id;
    let pool = db::get_db_pool().await;
    let result: Result<UserInfo, sqlx::Error> =
        sqlx::query_as("select * from user where id = ?")
            .bind(uid as i64)
            .fetch_one(pool)
            .await;
    if let Ok(user) = result {
        return Json(json!({
            "code": 0,
            "message": "OK",
            "data": {
                "id": user.id,
                "username": user.username,
                "email": user.email,
                "nickname": user.nickname,
                "last_login_ip": user.last_login_ip,
                "last_login_time": user.last_login_time,
                "roles": user.roles.split(',').collect::<Vec<&str>>(),
                "permissions": user.permissions.split(',').collect::<Vec<&str>>(),
            }
        }));
    }
    Json(json!({
        "code": -1,
        "message": "User not found",
    }))
}

/// List all users — admin only
pub async fn user_list(claims: ValidatedClaims) -> ZapJsonResult {
    require_admin(&claims)?;

    let pool = db::get_db_pool().await;

    let mut querybuilder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT id,username,email,nickname,last_login_ip,last_login_time,status,roles,permissions,created_at,updated_at FROM user",
    );
    querybuilder.push(" order by id desc");
    let users: Vec<UserInfo> = querybuilder.build_query_as().fetch_all(pool).await?;

    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": users.iter().map(|user| {
            json!({
                "id": user.id,
                "username": user.username,
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
    })))
}

/// Create a new user — admin only
pub async fn user_add(
    claims: ValidatedClaims,
    Json(payload): Json<CreateUserPayload>,
) -> ZapJsonResult {
    require_admin(&claims)?;

    let hashed = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| ZapError::Error(format!("密码加密失败: {}", e)))?;

    let now = chrono::Local::now().timestamp();
    let roles = payload.roles.unwrap_or_else(|| "user".to_string());
    let nickname = payload.nickname.unwrap_or_else(|| payload.username.clone());

    let pool = db::get_db_pool().await;
    let result = sqlx::query(
        "INSERT INTO user (username, password, email, nickname, roles, permissions, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&payload.username)
    .bind(&hashed)
    .bind(&payload.email)
    .bind(&nickname)
    .bind(&roles)
    .bind("")
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            info!("User created: {} (id: {})", payload.username, r.last_insert_rowid());
            Ok(Json(json!({
                "code": 0,
                "message": "用户创建成功",
                "data": { "id": r.last_insert_rowid() }
            })))
        }
        Err(e) => {
            if e.to_string().contains("UNIQUE") {
                Err(ZapError::New(-1, "用户名或邮箱已存在".to_string()))
            } else {
                Err(ZapError::from(e))
            }
        }
    }
}

/// Update an existing user — admin only.
/// Default-password users may only update their own password.
pub async fn user_update(
    claims: Claims,
    Json(payload): Json<UpdateUserPayload>,
) -> ZapJsonResult {
    // Default-password users: only allow changing their own password
    if claims.pwd_is_default {
        if payload.id != claims.id as i64 {
            return Err(ZapError::New(-1, "请先修改默认密码".to_string()));
        }
        // Only allow password updates for default-password users
        if payload.email.is_some()
            || payload.nickname.is_some()
            || payload.roles.is_some()
            || payload.status.is_some()
        {
            return Err(ZapError::New(-1, "请先修改默认密码，当前只能修改密码".to_string()));
        }
    } else {
        // Normal users need admin to update others
        if payload.id != claims.id as i64 {
            require_admin(&claims)?;
        }
    }

    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();

    let has_any_field = payload.email.is_some()
        || payload.nickname.is_some()
        || payload.roles.is_some()
        || payload.status.is_some()
        || payload.password.is_some();

    if !has_any_field {
        return Err(ZapError::New(-1, "没有需要更新的字段".to_string()));
    }

    let mut qb: QueryBuilder<'_, Sqlite> = QueryBuilder::new("UPDATE user SET ");
    let mut separated = qb.separated(", ");

    if let Some(ref email) = payload.email {
        separated.push("email = ").push_bind_unseparated(email);
    }
    if let Some(ref nickname) = payload.nickname {
        separated.push("nickname = ").push_bind_unseparated(nickname);
    }
    if let Some(ref roles) = payload.roles {
        separated.push("roles = ").push_bind_unseparated(roles);
    }
    if let Some(status) = payload.status {
        separated.push("status = ").push_bind_unseparated(status);
    }
    if let Some(ref password) = payload.password {
        let hashed = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| ZapError::Error(format!("密码加密失败: {}", e)))?;
        separated.push("password = ").push_bind_unseparated(hashed);
    }
    separated.push("updated_at = ").push_bind_unseparated(now);

    qb.push(" WHERE id = ").push_bind(payload.id);

    let result = qb.build().execute(pool).await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "用户不存在".to_string()));
    }

    Ok(Json(json!({
        "code": 0,
        "message": "用户更新成功"
    })))
}

/// Delete a user — admin only
pub async fn user_delete(
    claims: ValidatedClaims,
    Json(payload): Json<DeleteUserPayload>,
) -> ZapJsonResult {
    require_admin(&claims)?;

    if payload.id == claims.id as i64 {
        return Err(ZapError::New(-1, "不能删除自己".to_string()));
    }

    let pool = db::get_db_pool().await;
    let result = sqlx::query("DELETE FROM user WHERE id = ?")
        .bind(payload.id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "用户不存在".to_string()));
    }

    info!("User deleted: id={}", payload.id);
    Ok(Json(json!({
        "code": 0,
        "message": "用户删除成功"
    })))
}
