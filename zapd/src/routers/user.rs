use axum::{Json, extract::Extension};
use serde::{Deserialize, Serialize};
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
    /// PHP-FPM 规格引用：''=面板默认 / 'inherit'=继承 owner(reseller) 名下默认 / 模板名
    fpm_spec_ref: String,
    last_login_ip: String,
    last_login_time: i64,
    status: i32,
    roles: String,
    permissions: String,
    owner_id: i64,
    package_id: i64,
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
    /// PHP-FPM 规格引用：''=面板默认 / 'inherit'=继承 owner(reseller) 名下默认 / 模板名
    pub fpm_spec_ref: Option<String>,
    /// 套餐 id（0 / None = 不绑定套餐）
    pub package_id: Option<i64>,
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
    /// PHP-FPM 规格引用：''=面板默认 / 'inherit'=继承 owner(reseller) 名下默认 / 模板名；
    /// 提交引用（含面板默认）时后端会同步清空旧的自定义 fpm_pool
    pub fpm_spec_ref: Option<String>,
    /// 套餐 id（0 = 解除套餐绑定）
    pub package_id: Option<i64>,
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

/// 按用户当前套餐下发磁盘配额（best-effort：失败仅写日志，不阻断用户创建/编辑）。
/// 未绑定套餐、套餐未提供配额字段或用户无 Linux 系统账号（www 统一模式）时跳过。
pub async fn sync_package_quota(user_id: i64) {
    let Some(pkg) = crate::routers::package::package_of_user(user_id).await else {
        return;
    };
    let pool = db::get_db_pool().await;
    let lu: Option<String> = sqlx::query_scalar("SELECT linux_user FROM user WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    let Some(linux_user) = lu.filter(|s| !s.trim().is_empty()) else {
        return;
    };
    let req = zap_proto::types::Request::UserQuotaSet {
        linux_user: linux_user.clone(),
        quota_mb: pkg.disk_quota_mb,
    };
    match crate::zapexec::call(req).await {
        Ok(resp) if resp.code == 0 => info!(
            "套餐「{}」磁盘配额已下发: {} = {} MB",
            pkg.name, linux_user, pkg.disk_quota_mb
        ),
        Ok(resp) => warn!(
            "下发磁盘配额失败({}): {}（配额未生效时可检查文件系统是否启用 quota）",
            linux_user, resp.message
        ),
        Err(e) => warn!("下发磁盘配额失败({}): {}", linux_user, e),
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
                "fpm_spec_ref": user.fpm_spec_ref,
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

// ── 个人中心 → 偏好设置 ──────────────────────────────────────

fn default_true() -> bool {
    true
}

fn default_autossl_mode() -> String {
    "deferrals".to_string()
}

/// 当前用户通知/其它偏好（个人中心 → 偏好设置），存 user.prefs（JSON）。
/// 子项 `*_disable`：对应父通知类别下的子通知被禁用（cPanel 风格偏好覆盖）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NoticePrefs {
    /// 账户接近磁盘配额
    #[serde(default = "default_true")]
    pub notify_disk_quota: bool,
    /// 账户接近带宽限制
    #[serde(default = "default_true")]
    pub notify_bandwidth: bool,
    /// SSL 证书即将过期
    #[serde(default = "default_true")]
    pub notify_ssl_expiry: bool,
    /// 账户密码变化
    #[serde(default = "default_true")]
    pub notify_password_change: bool,
    #[serde(default)]
    pub password_change_disable: bool,
    /// 有人登录我的账户（成功登录通知）
    #[serde(default)]
    pub notify_login: bool,
    #[serde(default)]
    pub login_disable: bool,
    /// AutoSSL 通知模式：deferrals=失败及延后 / failures=仅失败 / disabled=禁用
    #[serde(default = "default_autossl_mode")]
    pub autossl_notify_mode: String,
}

impl Default for NoticePrefs {
    fn default() -> Self {
        Self {
            notify_disk_quota: true,
            notify_bandwidth: true,
            notify_ssl_expiry: true,
            notify_password_change: true,
            password_change_disable: false,
            notify_login: false,
            login_disable: false,
            autossl_notify_mode: "deferrals".to_string(),
        }
    }
}

/// GET /user/prefs：读取当前用户的偏好设置（无记录时返回默认值）。
pub async fn user_prefs_get(claims: Claims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let prefs: Option<String> = sqlx::query_scalar("SELECT prefs FROM user WHERE id = ?")
        .bind(claims.id as i64)
        .fetch_optional(pool)
        .await?;
    let prefs = prefs
        .and_then(|s| serde_json::from_str::<NoticePrefs>(&s).ok())
        .unwrap_or_default();
    Ok(Json(json!({ "code": 0, "message": "ok", "data": prefs })))
}

/// POST /user/prefs：保存当前用户的偏好设置（仅能改自己）。
pub async fn user_prefs_save(claims: Claims, Json(payload): Json<NoticePrefs>) -> ZapJsonResult {
    let mut data = payload;
    // 约束 AutoSSL 通知模式取值，避免非法字符进入
    match data.autossl_notify_mode.as_str() {
        "failures" | "disabled" => {}
        _ => data.autossl_notify_mode = "deferrals".to_string(),
    }
    let store = serde_json::to_string(&data).unwrap_or_default();
    let pool = db::get_db_pool().await;
    sqlx::query("UPDATE user SET prefs = ? WHERE id = ?")
        .bind(&store)
        .bind(claims.id as i64)
        .execute(pool)
        .await?;
    Ok(Json(
        json!({ "code": 0, "message": "偏好设置已保存", "data": data }),
    ))
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
        "SELECT id,username,email,phone,nickname,home_dir,linux_user,fpm_pool,fpm_spec_ref,last_login_ip,last_login_time,status,roles,permissions,owner_id,package_id,created_at,updated_at FROM user",
    );
    if is_reseller && !is_admin {
        querybuilder
            .push(" WHERE owner_id = ")
            .push_bind(claims.id as i64);
    }
    querybuilder.push(" order by id desc");
    let users: Vec<UserInfo> = querybuilder.build_query_as().fetch_all(pool).await?;
    // 套餐名映射（列表展示用；未绑定套餐时 id 为 0，查不到即空串）
    let pkg_rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM packages")
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    let pkg_names: std::collections::HashMap<i64, String> = pkg_rows.into_iter().collect();

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
                "fpm_spec_ref": user.fpm_spec_ref,
                "last_login_ip": user.last_login_ip,
                "last_login_time": user.last_login_time,
                "status": user.status,
                "roles": user.roles.split(',').collect::<Vec<&str>>(),
                "permissions": user.permissions.split(',').collect::<Vec<&str>>(),
                "owner_id": user.owner_id,
                "package_id": user.package_id,
                "package_name": pkg_names
                    .get(&user.package_id)
                    .cloned()
                    .unwrap_or_default(),
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
    // 套餐：校验可见性（admin 全部；reseller 仅全局与自己名下）
    let package = match payload.package_id.filter(|v| *v > 0) {
        Some(pid) => {
            Some(crate::routers::package::load_for_actor(pid, is_admin, claims.id as i64).await?)
        }
        None => None,
    };
    let package_id = package.as_ref().map(|p| p.id).unwrap_or(0);
    // fpm 规格：fpm_pool（旧版自定义 JSON）与 fpm_spec_ref（模板/继承/默认）互斥。
    // 前端新流程只传 fpm_spec_ref；一旦显式指定引用，不再保留自定义 JSON（避免遮蔽模板）。
    let mut fpm_pool = normalize_fpm_spec(payload.fpm_pool)?.unwrap_or_default();
    let mut fpm_spec_ref = payload.fpm_spec_ref.unwrap_or_default().trim().to_string();
    // 未显式选择模板时继承套餐绑定的 FPM 规格模板
    if fpm_spec_ref.is_empty()
        && let Some(p) = &package
        && !p.fpm_spec_ref.trim().is_empty()
    {
        fpm_spec_ref = p.fpm_spec_ref.trim().to_string();
        // 套餐模板可能由 admin 创建，reseller 使用时同样校验可见性
        crate::routers::fpm_spec::validate_spec_ref(&fpm_spec_ref, is_admin, &claims.sub).await?;
    }
    if !fpm_spec_ref.is_empty() {
        fpm_pool.clear();
    }
    // 引用校验：reseller 只能选自己名下或全局通用模板；admin 校验模板存在性
    if fpm_spec_ref.is_empty() || fpm_spec_ref == crate::routers::fpm_spec::INHERIT {
        // '' 与 inherit 恒允许
    } else if is_admin {
        crate::routers::fpm_spec::validate_spec_ref(&fpm_spec_ref, true, "").await?;
    } else {
        crate::routers::fpm_spec::validate_spec_ref(&fpm_spec_ref, false, &claims.sub).await?;
    }

    // 家目录 / Linux 账号：{默认挂载点}/{linux_username(username)} 派生，
    // 站点文档根与站点日志均规划于其下；派生名与已有账号冲突时追加 -n 后缀。
    // 默认挂载点取自运行环境默认设置（conf: user_home_root，默认 /home）；
    // /home 磁盘不足时管理员可切换到新挂载点（如 /home2），此后新建用户即落到新挂载点，
    // 存量用户不受影响（数据迁移请使用「服务器配置 → 数据迁移」）。
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
    let home_root = crate::routers::system_env::conf_get("user_home_root")
        .await
        .filter(|s| s.starts_with('/') && !s.contains("..") && s.len() > 1)
        .unwrap_or_else(|| "/home".to_string());
    let home_dir = format!("{home_root}/{lu}");

    let result = sqlx::query(
        "INSERT INTO user (username, home_dir, linux_user, fpm_pool, fpm_spec_ref, password, email, phone, nickname, roles, permissions, owner_id, package_id, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&payload.username)
    .bind(&home_dir)
    .bind(&lu)
    .bind(&fpm_pool)
    .bind(&fpm_spec_ref)
    .bind(&hashed)
    .bind(&payload.email)
    .bind(phone)
    .bind(&nickname)
    .bind(&roles)
    .bind("")
    .bind(owner_id)
    .bind(package_id)
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
            // 套餐：下发磁盘配额（系统账号就绪后执行，best-effort）
            if package.is_some() {
                sync_package_quota(new_id).await;
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
            || payload.fpm_spec_ref.is_some()
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
            "权限不足，不能修改 PHP-FPM 自定义规格".to_string(),
        ));
    }
    // fpm_spec_ref（模板 / 继承）：reseller 可为自己客户设置，但只能选自己名下或全局通用模板
    if !jwt::is_admin(&claims) && payload.fpm_spec_ref.is_some() {
        let rv = payload.fpm_spec_ref.as_deref().unwrap_or("").trim();
        if !rv.is_empty() && rv != crate::routers::fpm_spec::INHERIT {
            crate::routers::fpm_spec::validate_spec_ref(rv, false, &claims.sub).await?;
        }
    }
    // 套餐：admin 全部可用；reseller 仅全局套餐与自己名下套餐
    if let Some(pid) = payload.package_id
        && pid > 0
    {
        crate::routers::package::load_for_actor(pid, jwt::is_admin(&claims), claims.id as i64)
            .await?;
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
        || payload.fpm_pool.is_some()
        || payload.fpm_spec_ref.is_some()
        || payload.package_id.is_some();

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
        separated
            .push("fpm_pool = ")
            .push_bind_unseparated(norm.clone());
        // 自定义 JSON 与模板引用互斥：提交自定义时清空引用
        if payload.fpm_spec_ref.is_none() {
            separated
                .push("fpm_spec_ref = ")
                .push_bind_unseparated(String::new());
        }
    }
    if let Some(ref rv) = payload.fpm_spec_ref {
        let norm = rv.trim().to_string();
        if jwt::is_admin(&claims) && !norm.is_empty() && norm != crate::routers::fpm_spec::INHERIT {
            crate::routers::fpm_spec::validate_spec_ref(&norm, true, "").await?;
        }
        separated
            .push("fpm_spec_ref = ")
            .push_bind_unseparated(norm);
        // 切到「模板 / 继承 / 面板默认」后清除旧的自定义 JSON，避免遮蔽新选择
        separated
            .push("fpm_pool = ")
            .push_bind_unseparated(String::new());
    }
    if let Some(pid) = payload.package_id {
        separated
            .push("package_id = ")
            .push_bind_unseparated(pid.max(0));
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

    // 套餐变更 → 重新下发磁盘配额（best-effort）
    if payload.package_id.is_some() {
        sync_package_quota(payload.id).await;
    }

    // First-time password change (was still using the default password):
    // tell the frontend to log out and require re-login with the new password.
    let mut resp = json!({ "code": 0, "message": "用户更新成功" });
    if payload.password.is_some() && claims.pwd_is_default {
        resp["must_relogin"] = json!(true);
    }
    // 密码被修改（本人或管理员/经销商改密）→ 向目标用户发站内信（受其通知偏好控制）
    if payload.password.is_some() {
        crate::zap::notify::password_changed(payload.id, &claims.sub).await;
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
