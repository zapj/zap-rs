use axum::{extract::Extension, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite};
use std::net::SocketAddr;
use tracing::info;

use crate::{
    db,
    zap::{
        audit,
        jwt::{self, Claims, ValidatedClaims},
        ZapError, ZapJsonResult,
    },
};

#[derive(sqlx::FromRow, Debug)]
struct UserInfo {
    id: i64,
    username: String,
    email: String,
    phone: Option<String>,
    nickname: String,
    last_login_ip: String,
    last_login_time: i64,
    status: i32,
    roles: String,
    permissions: String,
    owner_id: i64,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub email: String,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    pub roles: Option<String>,
    pub owner_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPayload {
    pub id: i64,
    pub email: Option<String>,
    pub phone: Option<String>,
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

/// Fetch the owner_id of a user. Returns -1 when the user does not exist.
async fn get_user_owner_id(id: i64) -> Result<i64, ZapError> {
    let pool = db::get_db_pool().await;
    let row: Option<(i64,)> = sqlx::query_as("SELECT owner_id FROM user WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(o,)| o).unwrap_or(-1))
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
                "phone": user.phone.clone().unwrap_or_default(),
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

/// List users — admin sees all, reseller sees only own customers
pub async fn user_list(claims: ValidatedClaims) -> ZapJsonResult {
    let is_admin = jwt::is_admin(&claims);
    let is_reseller = jwt::is_reseller(&claims);
    if !is_admin && !is_reseller {
        return Err(ZapError::New(-1, "权限不足".to_string()));
    }

    let pool = db::get_db_pool().await;

    let mut querybuilder: QueryBuilder<'_, Sqlite> = QueryBuilder::new(
        "SELECT id,username,email,phone,nickname,last_login_ip,last_login_time,status,roles,permissions,owner_id,created_at,updated_at FROM user",
    );
    if is_reseller && !is_admin {
        querybuilder.push(" WHERE owner_id = ").push_bind(claims.id as i64);
    }
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
                "phone": user.phone.clone().unwrap_or_default(),
                "nickname": user.nickname,
                "last_login_ip": user.last_login_ip,
                "last_login_time": user.last_login_time,
                "status": user.status,
                "roles": user.roles.split(',').collect::<Vec<&str>>(),
                "permissions": user.permissions.split(',').collect::<Vec<&str>>(),
                "owner_id": user.owner_id,
                "created_at": user.created_at,
                "updated_at": user.updated_at,
            })
        }).collect::<Vec<Value>>(),
        "total": users.len(),
    })))
}

/// Create a new user — admin or reseller (own customer only)
pub async fn user_add(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CreateUserPayload>,
) -> ZapJsonResult {
    let is_admin = jwt::is_admin(&claims);
    let is_reseller = jwt::is_reseller(&claims);
    if !is_admin && !is_reseller {
        return Err(ZapError::New(-1, "权限不足".to_string()));
    }

    let hashed = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| ZapError::Error(format!("密码加密失败: {}", e)))?;

    let now = chrono::Local::now().timestamp();
    // reseller 只能创建普通用户客户；admin 可指定角色
    let roles = if is_admin {
        payload.roles.unwrap_or_else(|| "user".to_string())
    } else {
        "user".to_string()
    };
    // reseller 创建的客户归属自己；admin 可指定归属（默认系统直属）
    let owner_id: i64 = if is_admin {
        payload.owner_id.unwrap_or(0)
    } else {
        claims.id as i64
    };
    let nickname = payload.nickname.unwrap_or_else(|| payload.username.clone());
    // 空手机号存 NULL，避免 UNIQUE 约束下多个空串互相冲突
    let phone = payload
        .phone
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    let pool = db::get_db_pool().await;
    let result = sqlx::query(
        "INSERT INTO user (username, password, email, phone, nickname, roles, permissions, owner_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&payload.username)
    .bind(&hashed)
    .bind(&payload.email)
    .bind(phone)
    .bind(&nickname)
    .bind(&roles)
    .bind("")
    .bind(owner_id)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            audit::log(
                Some(&claims),
                Some(client_addr.ip().to_string().as_str()),
                "user_create",
                &format!("id={}", r.last_insert_rowid()),
                &format!("username={}", payload.username),
            )
            .await;
            info!("User created: {} (id: {})", payload.username, r.last_insert_rowid());
            Ok(Json(json!({
                "code": 0,
                "message": "用户创建成功",
                "data": { "id": r.last_insert_rowid() }
            })))
        }
        Err(e) => {
            if let sqlx::Error::Database(db_err) = &e {
                let msg = db_err.message();
                if msg.contains("user.phone") {
                    return Err(ZapError::New(-1, "手机号已被其他用户使用".to_string()));
                }
                if msg.contains("user.email") {
                    return Err(ZapError::New(-1, "邮箱已存在".to_string()));
                }
                if msg.contains("user.username") {
                    return Err(ZapError::New(-1, "用户名已存在".to_string()));
                }
            }
            if e.to_string().contains("UNIQUE") {
                Err(ZapError::New(-1, "用户名、邮箱或手机号已存在".to_string()))
            } else {
                Err(ZapError::from(e))
            }
        }
    }
}

/// Update an existing user.
/// - admin: any user
/// - reseller: own customers only, and cannot change roles
/// - default-password users: only their own password
pub async fn user_update(
    claims: Claims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<UpdateUserPayload>,
) -> ZapJsonResult {
    // Default-password users: only allow changing their own password
    if claims.pwd_is_default {
        if payload.id != claims.id as i64 {
            return Err(ZapError::New(-1, "请先修改默认密码".to_string()));
        }
        // Only allow password updates for default-password users
        if payload.email.is_some()
            || payload.phone.is_some()
            || payload.nickname.is_some()
            || payload.roles.is_some()
            || payload.status.is_some()
        {
            return Err(ZapError::New(-1, "请先修改默认密码，当前只能修改密码".to_string()));
        }
    }

    // 非管理员不能修改角色（防止提权，admin 除外）
    if !jwt::is_admin(&claims) && payload.roles.is_some() {
        return Err(ZapError::New(-1, "权限不足，不能修改角色".to_string()));
    }

    // 更新他人时的归属/权限校验
    if !claims.pwd_is_default && payload.id != claims.id as i64 {
        if jwt::is_admin(&claims) {
            // admin: full access
        } else if jwt::is_reseller(&claims) {
            // reseller: own customers only
            let owner_id = get_user_owner_id(payload.id).await?;
            if owner_id != claims.id as i64 {
                return Err(ZapError::New(-1, "权限不足，只能管理自己的客户".to_string()));
            }
        } else {
            require_admin(&claims)?;
        }
    }

    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();

    let has_any_field = payload.email.is_some()
        || payload.phone.is_some()
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
    if let Some(ref phone) = payload.phone {
        let p = phone.trim();
        if p.is_empty() {
            // 空手机号清空为 NULL
            separated.push("phone = ").push_bind_unseparated(Option::<String>::None);
        } else {
            separated.push("phone = ").push_bind_unseparated(p);
        }
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

    let result = match qb.build().execute(pool).await {
        Ok(r) => r,
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("user.phone") => {
            return Err(ZapError::New(-1, "手机号已被其他用户使用".to_string()));
        }
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("user.email") => {
            return Err(ZapError::New(-1, "邮箱已被其他用户使用".to_string()));
        }
        Err(e) => return Err(ZapError::from(e)),
    };

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "用户不存在".to_string()));
    }

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "user_update",
        &format!("id={}", payload.id),
        "",
    )
    .await;

    // First-time password change (was still using the default password):
    // tell the frontend to log out and require re-login with the new password.
    let mut resp = json!({ "code": 0, "message": "用户更新成功" });
    if payload.password.is_some() && claims.pwd_is_default {
        resp["must_relogin"] = json!(true);
    }
    Ok(Json(resp))
}

/// Delete a user — admin: any; reseller: own customers only
pub async fn user_delete(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<DeleteUserPayload>,
) -> ZapJsonResult {
    if jwt::is_admin(&claims) {
        // admin: full access (still cannot delete self)
    } else if jwt::is_reseller(&claims) {
        // reseller: own customers only
        let owner_id = get_user_owner_id(payload.id).await?;
        if owner_id != claims.id as i64 {
            return Err(ZapError::New(-1, "权限不足，只能删除自己的客户".to_string()));
        }
    } else {
        require_admin(&claims)?;
    }

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

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "user_delete",
        &format!("id={}", payload.id),
        "",
    )
    .await;

    info!("User deleted: id={}", payload.id);
    Ok(Json(json!({
        "code": 0,
        "message": "用户删除成功"
    })))
}

/// List all reseller users — admin only (used to assign customer ownership)
pub async fn reseller_list(claims: ValidatedClaims) -> ZapJsonResult {
    require_admin(&claims)?;

    let pool = db::get_db_pool().await;
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, username, nickname FROM user WHERE roles LIKE '%reseller%' ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": rows.iter().map(|(id, username, nickname)| {
            json!({ "id": id, "username": username, "nickname": nickname })
        }).collect::<Vec<Value>>(),
    })))
}
