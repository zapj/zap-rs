use axum::{Json, extract::Extension};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{QueryBuilder, Sqlite};
use std::net::SocketAddr;
use tracing::{info, warn};

use crate::{
    db,
    zap::{
        ZapError, ZapJsonResult, audit,
        jwt::{self, Claims, ValidatedClaims},
    },
};

#[derive(sqlx::FromRow, Debug)]
struct UserInfo {
    id: i64,
    username: String,
    email: String,
    phone: Option<String>,
    nickname: String,
    home_dir: String,
    linux_user: String,
    fpm_pool: String,
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
    /// 该用户 PHP-FPM pool 规格（JSON 字符串；空 = 使用面板默认规格）
    pub fpm_pool: Option<String>,
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
    /// 该用户 PHP-FPM pool 规格（JSON 字符串；空 = 恢复面板默认）
    pub fpm_pool: Option<String>,
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

/// 归一化用户 fpm pool 规格：
/// - None / 空 → Some("")（不指定，使用面板默认）
/// - 其它 → 必须是 JSON 对象字符串
fn normalize_fpm_spec(raw: Option<String>) -> Result<Option<String>, ZapError> {
    match raw {
        None => Ok(None),
        Some(v) => {
            let v = v.trim().to_string();
            if v.is_empty() {
                return Ok(Some(String::new()));
            }
            match serde_json::from_str::<Value>(&v) {
                Ok(Value::Object(_)) => Ok(Some(v)),
                _ => Err(ZapError::New(
                    -1,
                    "fpm_pool 必须是 JSON 对象（如 {\"max_children\": 12}）".to_string(),
                )),
            }
        }
    }
}

/// 按全局虚拟主机运行模式补齐「面板用户 → 运行实体」（幂等）：
/// - system：确保 user.linux_user 有值，创建 Linux 系统账号（useradd，nologin），
///   家目录按独立用户模式赋权（owner = linux_user）
/// - www：仅按统一 www 模式补家目录骨架
///
/// 站点同步 / 用户同步 / 新增用户均调用；失败返回 Err 描述。
pub async fn ensure_user_runtime(uid: i64) -> Result<(), String> {
    let pool = db::get_db_pool().await;
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT username, home_dir, linux_user FROM user WHERE id = ?")
            .bind(uid)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let Some((username, home_dir, mut linux_user)) = row else {
        return Err(format!("用户 {uid} 不存在"));
    };
    if home_dir.is_empty() {
        return Err(format!("用户 {username} 未配置家目录（home_dir 为空）"));
    }
    if linux_user.is_empty() {
        linux_user = zap_proto::linux_username(&username);
        sqlx::query("UPDATE user SET linux_user = ? WHERE id = ?")
            .bind(&linux_user)
            .bind(uid)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }
    let mode = crate::routers::system_env::vhost_mode().await;
    if mode == "system" {
        let resp = crate::zapexec::call(zap_proto::types::Request::UserSystemInit {
            linux_user: linux_user.clone(),
            home_dir: home_dir.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;
        if resp.code != 0 {
            return Err(format!("创建 Linux 账号失败: {}", resp.message));
        }
    }
    let owner = if mode == "system" {
        Some(linux_user)
    } else {
        None
    };
    let resp = crate::zapexec::call(zap_proto::types::Request::UserHomeInit {
        home_dir: home_dir.clone(),
        owner,
    })
    .await
    .map_err(|e| e.to_string())?;
    if resp.code != 0 {
        return Err(format!("初始化家目录失败: {}", resp.message));
    }
    Ok(())
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
    let result: Result<UserInfo, sqlx::Error> = sqlx::query_as("select * from user where id = ?")
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
                "home_dir": user.home_dir,
                "linux_user": user.linux_user,
                "fpm_pool": user.fpm_pool,
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
        "SELECT id,username,email,phone,nickname,home_dir,linux_user,fpm_pool,last_login_ip,last_login_time,status,roles,permissions,owner_id,created_at,updated_at FROM user",
    );
    if is_reseller && !is_admin {
        querybuilder
            .push(" WHERE owner_id = ")
            .push_bind(claims.id as i64);
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
                "home_dir": user.home_dir,
                "linux_user": user.linux_user,
                "fpm_pool": user.fpm_pool,
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
    // fpm pool 规格（空 = 面板默认）
    let fpm_pool = normalize_fpm_spec(payload.fpm_pool)?;
    let fpm_pool = fpm_pool.unwrap_or_default();

    // 家目录 / Linux 账号：/home/{linux_username(username)} 派生，
    // 站点文档根与站点日志均规划于其下；派生名与已有账号冲突时追加 -n 后缀
    let lu_base = zap_proto::linux_username(&payload.username);
    let pool = db::get_db_pool().await;
    let mut lu = lu_base.clone();
    let mut n: i64 = 2;
    loop {
        let cnt: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM user WHERE linux_user = ? AND linux_user != ''")
                .bind(&lu)
                .fetch_one(pool)
                .await
                .unwrap_or((0,));
        if cnt.0 == 0 {
            break;
        }
        lu = format!("{lu_base}-{n}");
        n += 1;
    }
    let home_dir = format!("/home/{lu}");

    let result = sqlx::query(
        "INSERT INTO user (username, home_dir, linux_user, fpm_pool, password, email, phone, nickname, roles, permissions, owner_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&payload.username)
    .bind(&home_dir)
    .bind(&lu)
    .bind(&fpm_pool)
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
            info!(
                "User created: {} (id: {})",
                payload.username,
                r.last_insert_rowid()
            );
            // 按全局运行模式补齐运行实体（system=Linux 账号 / www=家目录骨架）。
            // 尽力而为：失败仅告警，站点同步时仍会递归补齐
            let new_id = r.last_insert_rowid();
            if let Err(e) = ensure_user_runtime(new_id).await {
                warn!("初始化用户运行实体失败(id={}): {}", new_id, e);
            }
            Ok(Json(json!({
                "code": 0,
                "message": "用户创建成功",
                "data": { "id": new_id, "home_dir": home_dir, "linux_user": lu }
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
            || payload.fpm_pool.is_some()
        {
            return Err(ZapError::New(
                -1,
                "请先修改默认密码，当前只能修改密码".to_string(),
            ));
        }
    }

    // 非管理员不能修改角色（防止提权，admin 除外）
    if !jwt::is_admin(&claims) && payload.roles.is_some() {
        return Err(ZapError::New(-1, "权限不足，不能修改角色".to_string()));
    }
    // PHP-FPM pool 规格（资源配额类）仅管理员可配置
    if !jwt::is_admin(&claims) && payload.fpm_pool.is_some() {
        return Err(ZapError::New(
            -1,
            "权限不足，不能修改 PHP-FPM 规格".to_string(),
        ));
    }

    // 更新他人时的归属/权限校验
    if !claims.pwd_is_default && payload.id != claims.id as i64 {
        if jwt::is_admin(&claims) {
            // admin: full access
        } else if jwt::is_reseller(&claims) {
            // reseller: own customers only
            let owner_id = get_user_owner_id(payload.id).await?;
            if owner_id != claims.id as i64 {
                return Err(ZapError::New(
                    -1,
                    "权限不足，只能管理自己的客户".to_string(),
                ));
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
        || payload.password.is_some()
        || payload.fpm_pool.is_some();

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
            separated
                .push("phone = ")
                .push_bind_unseparated(Option::<String>::None);
        } else {
            separated.push("phone = ").push_bind_unseparated(p);
        }
    }
    if let Some(ref nickname) = payload.nickname {
        separated
            .push("nickname = ")
            .push_bind_unseparated(nickname);
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
    if let Some(ref fpm) = payload.fpm_pool {
        let norm = normalize_fpm_spec(Some(fpm.clone()))?.unwrap_or_default();
        separated.push("fpm_pool = ").push_bind_unseparated(norm);
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
            return Err(ZapError::New(
                -1,
                "权限不足，只能删除自己的客户".to_string(),
            ));
        }
    } else {
        require_admin(&claims)?;
    }

    if payload.id == claims.id as i64 {
        return Err(ZapError::New(-1, "不能删除自己".to_string()));
    }

    let pool = db::get_db_pool().await;
    // 独立系统用户模式下，先记录待清理的 Linux 账号
    let linux_user: Option<String> = sqlx::query_scalar("SELECT linux_user FROM user WHERE id = ?")
        .bind(payload.id)
        .fetch_optional(pool)
        .await?
        .filter(|s: &String| !s.is_empty());
    let was_system = crate::routers::system_env::vhost_mode().await == "system";

    let result = sqlx::query("DELETE FROM user WHERE id = ?")
        .bind(payload.id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "用户不存在".to_string()));
    }

    // 删除用户后清理运行实体（system 模式：清 pool + userdel）
    if was_system && let Some(lu) = linux_user {
        match crate::zapexec::call(zap_proto::types::Request::UserSystemRemove {
            linux_user: lu.clone(),
        })
        .await
        {
            Ok(resp) if resp.code != 0 => {
                warn!("清理 Linux 账号失败(id={}): {}", payload.id, resp.message);
            }
            Err(e) => {
                warn!("清理 Linux 账号失败(id={}): {}", payload.id, e);
            }
            _ => {
                info!("Linux 账号已清理: {} (user id={})", lu, payload.id);
            }
        }
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

/// 批量补齐所有用户运行实体（admin only）：按全局虚拟主机运行模式
/// 为每个用户补家目录骨架（www 模式）或 Linux 账号 + 独立用户家目录（system 模式）。
/// 个别失败不影响整体（结果里给出失败清单）。
pub async fn user_home_sync(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
) -> ZapJsonResult {
    require_admin(&claims)?;

    let mode = crate::routers::system_env::vhost_mode().await;
    let pool = db::get_db_pool().await;
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, username, home_dir, linux_user FROM user WHERE home_dir != '' ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    let mut ok_items: Vec<Value> = Vec::new();
    let mut fail_items: Vec<Value> = Vec::new();
    for (id, username, home_dir, linux_user) in rows {
        match ensure_user_runtime(id).await {
            Ok(()) => {
                ok_items.push(json!({
                    "id": id,
                    "username": username,
                    "home_dir": home_dir,
                    "linux_user": linux_user,
                    "mode": mode,
                }));
            }
            Err(e) => {
                fail_items.push(json!({
                    "id": id,
                    "username": username,
                    "home_dir": home_dir,
                    "linux_user": linux_user,
                    "mode": mode,
                    "error": e,
                }));
            }
        }
    }

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "user_home_sync",
        &format!("ok={} fail={}", ok_items.len(), fail_items.len()),
        &format!("mode={mode}"),
    )
    .await;

    let action = if mode == "system" {
        "运行实体"
    } else {
        "家目录"
    };
    Ok(Json(json!({
        "code": 0,
        "message": format!("{action}同步完成：成功 {}，失败 {}", ok_items.len(), fail_items.len()),
        "data": { "ok": ok_items, "fail": fail_items, "mode": mode }
    })))
}
