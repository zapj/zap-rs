// 站点管理（admin 管理全部 / reseller 管理所属客户的站点 / 普通用户管理自己的站点）
// 一个站点可绑定多个域名与多个 IP（site_domain / site_ip 子表）
use axum::{Json, extract::Extension, extract::Query};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
};
use tracing::{info, warn};

use crate::{
    db,
    zap::{
        ZapError, ZapJsonResult, audit,
        jwt::{self, ValidatedClaims},
    },
};
use zap_proto::Request;

// ── SQL 行结构 ──────────────────────────────────────────────

// sqlx 行映射元组别名（避免 clippy::type_complexity）
type SiteRow = (
    i64,
    i64,
    String,
    i32,
    String,
    i64,
    i64,
    Option<String>,
    String,
);
type SiteRowExt = (
    i64,
    i64,
    String,
    i32,
    String,
    i64,
    i64,
    Option<String>,
    String,
    Vec<String>,
    Vec<String>,
);
type SyncOneRow = (
    String,
    i32,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
);

#[derive(sqlx::FromRow, Debug)]
struct OwnerCandidate {
    id: i64,
    username: String,
    nickname: String,
}

// ── 入参 ────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct SiteListQuery {
    pub search: Option<String>,
    pub status: Option<i32>,
    pub user_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SiteAddPayload {
    #[serde(default)]
    pub user_id: Option<i64>,
    /// 站点名称（非必填，留空默认取第一个域名）
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub remark: Option<String>,
    /// PHP 实例标识（appstore 已安装 PHP 应用的 instance，如 php74）；空表示未绑定
    #[serde(default)]
    pub php_instance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SiteUpdatePayload {
    pub id: i64,
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    /// None 表示域名保持不变；Some(任意数组，可为空) 表示整体覆盖
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    #[serde(default)]
    pub ips: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<i32>,
    #[serde(default)]
    pub remark: Option<String>,
    /// None 表示 PHP 实例保持不变；Some(空串) 表示清除 PHP 实例
    #[serde(default)]
    pub php_instance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SiteDeletePayload {
    pub ids: Vec<i64>,
}

// ── 工具函数 ────────────────────────────────────────────────

fn has_role(roles: &str, role: &str) -> bool {
    roles.split(',').any(|r| r.trim() == role)
}

/// 站点管理角色门禁：admin / reseller / 普通用户（demo 等不可访问）
fn require_manageable(claims: &jwt::Claims) -> Result<(), ZapError> {
    if jwt::is_admin(claims) || jwt::is_reseller(claims) || has_role(&claims.roles, "user") {
        Ok(())
    } else {
        Err(ZapError::New(
            -1,
            "权限不足，仅支持 admin / reseller / 普通用户访问站点管理".to_string(),
        ))
    }
}

/// 校验归属用户是否可被当前操作者指定：
/// 归属对象可以是 admin / reseller / user 任一角色（即可以归属自己或其它运营账号），
/// 但必须落在当前操作者的管理范围：
/// admin → 任意上述账号；reseller → 自己 + 自己的客户；普通用户 → 只能是自己
async fn resolve_target_user(claims: &jwt::Claims, target: i64) -> Result<(), ZapError> {
    let pool = db::get_db_pool().await;
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT roles, owner_id FROM user WHERE id = ?")
            .bind(target)
            .fetch_optional(pool)
            .await?;
    let Some((roles, owner_id)) = row else {
        return Err(ZapError::New(-1, "指定的归属用户不存在".to_string()));
    };
    let manageable =
        has_role(&roles, "admin") || has_role(&roles, "reseller") || has_role(&roles, "user");
    if !manageable {
        return Err(ZapError::New(
            -1,
            "该账号（如只读演示账号）不能作为站点归属".to_string(),
        ));
    }
    if jwt::is_admin(claims) {
        return Ok(());
    }
    if jwt::is_reseller(claims) {
        if target == claims.id as i64 || owner_id == claims.id as i64 {
            Ok(())
        } else {
            Err(ZapError::New(
                -1,
                "只能将站点归属自己或所属客户".to_string(),
            ))
        }
    } else if target == claims.id as i64 {
        Ok(())
    } else {
        Err(ZapError::New(-1, "普通用户只能管理自己的站点".to_string()))
    }
}

/// 校验站点是否处于当前操作者的管理范围
async fn site_in_scope(claims: &jwt::Claims, site_id: i64) -> Result<(), ZapError> {
    let pool = db::get_db_pool().await;
    let row: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM site WHERE id = ?")
        .bind(site_id)
        .fetch_optional(pool)
        .await?;
    let Some((uid,)) = row else {
        return Err(ZapError::New(-1, "站点不存在".to_string()));
    };
    if jwt::is_admin(claims) {
        return Ok(());
    }
    if jwt::is_reseller(claims) {
        // reseller：自己的站点也可直接管理
        if uid == claims.id as i64 {
            return Ok(());
        }
        let owner: Option<(i64,)> = sqlx::query_as("SELECT owner_id FROM user WHERE id = ?")
            .bind(uid)
            .fetch_optional(pool)
            .await?;
        match owner {
            Some((o,)) if o == claims.id as i64 => Ok(()),
            _ => Err(ZapError::New(
                -1,
                "只能管理自己或所属客户的站点".to_string(),
            )),
        }
    } else if uid == claims.id as i64 {
        Ok(())
    } else {
        Err(ZapError::New(-1, "只能管理自己的站点".to_string()))
    }
}

/// 规范化域名数组：trim + 小写 + 去空 + 去重（保持顺序）
fn norm_domains(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for d in raw {
        let t = d.trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// 规范化 IP 数组：trim + 去空 + 去重（保持顺序）
fn norm_ips(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in raw {
        let t = s.trim().to_string();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

fn valid_domain(d: &str) -> bool {
    !d.is_empty()
        && d.chars().count() <= 253
        && d.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// 名称非必填：留空默认取第一个域名
fn fallback_name(name: &str, domains: &[String]) -> String {
    if name.trim().is_empty() {
        domains.first().cloned().unwrap_or_default()
    } else {
        name.trim().to_string()
    }
}

/// PHP 实例标识校验：允许字母/数字/./_/-/@，最长 120；空串表示未绑定
fn valid_php_instance(s: &str) -> bool {
    s.chars().count() <= 120
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'@' | b'/'))
}

fn validate_site_fields(
    name: &str,
    domains: &[String],
    ips: &[String],
    remark: &str,
) -> Result<(), ZapError> {
    if name.is_empty() {
        return Err(ZapError::New(
            -1,
            "站点名称或域名不能为空（名称留空时默认取第一个域名）".to_string(),
        ));
    }
    if name.chars().count() > 120 {
        return Err(ZapError::New(
            -1,
            "站点名称过长（最多 120 个字符）".to_string(),
        ));
    }
    if domains.len() > 50 {
        return Err(ZapError::New(-1, "单个站点最多绑定 50 个域名".to_string()));
    }
    for d in domains {
        if !valid_domain(d) {
            return Err(ZapError::New(
                -1,
                format!("域名 {} 格式不正确（仅支持字母/数字/./-/ _）", d),
            ));
        }
    }
    if ips.len() > 50 {
        return Err(ZapError::New(-1, "单个站点最多绑定 50 个 IP".to_string()));
    }
    for ip in ips {
        if ip.parse::<IpAddr>().is_err() {
            return Err(ZapError::New(
                -1,
                format!("绑定的 IP {} 地址格式不正确", ip),
            ));
        }
    }
    if remark.chars().count() > 500 {
        return Err(ZapError::New(-1, "备注过长（最多 500 个字符）".to_string()));
    }
    Ok(())
}

/// 计算站点的文档根与日志目录：统一规划在归属用户家目录下
/// - web_root = {home}/www/{sanitize(name)}-{site_id}
/// - log_root = {home}/logs/{sanitize(name)}-{site_id}
///
/// 归属用户无 home_dir 时返回空串（执行端回退 {ZAP_PATH}/data/www/...，兼容老站点）
async fn site_dirs_for(
    user_id: i64,
    name: &str,
    site_id: i64,
) -> Result<(String, String), ZapError> {
    let pool = db::get_db_pool().await;
    let row: Option<(String,)> = sqlx::query_as("SELECT home_dir FROM user WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let Some((home,)) = row else {
        return Ok((String::new(), String::new()));
    };
    let home = home.trim();
    if home.is_empty() {
        return Ok((String::new(), String::new()));
    }
    let seg = zap_proto::sanitize_site_name(name);
    Ok((
        format!("{home}/www/{seg}-{site_id}"),
        format!("{home}/logs/{seg}-{site_id}"),
    ))
}

/// 检查域名是否与其它站点重复（exclude_site 用于更新时排除自身）
async fn ensure_domains_unique(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    domains: &[String],
    exclude_site: i64,
) -> Result<(), ZapError> {
    for d in domains {
        let exists: Option<(i64,)> =
            sqlx::query_as("SELECT site_id FROM site_domain WHERE domain = ? AND site_id != ?")
                .bind(d)
                .bind(exclude_site)
                .fetch_optional(&mut **tx)
                .await?;
        if let Some((sid,)) = exists {
            return Err(ZapError::New(
                -1,
                format!("域名 {} 已被其它站点（id={}）绑定", d, sid),
            ));
        }
    }
    Ok(())
}

// ── 处理器 ──────────────────────────────────────────────────

/// 站点列表（按角色裁剪范围）+ 汇总统计
pub async fn site_list(claims: ValidatedClaims, Query(q): Query<SiteListQuery>) -> ZapJsonResult {
    require_manageable(&claims)?;
    let pool = db::get_db_pool().await;
    let base_sql = "SELECT s.id, s.user_id, s.name, s.status, s.remark, s.created_at, s.updated_at, \
                    u.username AS owner_username, s.php_instance \
                    FROM site s LEFT JOIN user u ON u.id = s.user_id";
    let rows: Vec<SiteRow> = if jwt::is_admin(&claims) {
        sqlx::query_as(&format!("{} ORDER BY s.id DESC", base_sql))
            .fetch_all(pool)
            .await?
    } else if jwt::is_reseller(&claims) {
        // reseller：自己的站点 + 名下客户的站点
        sqlx::query_as(&format!(
            "{} WHERE s.user_id = ? OR s.user_id IN (SELECT id FROM user WHERE owner_id = ?) \
                 ORDER BY s.id DESC",
            base_sql
        ))
        .bind(claims.id as i64)
        .bind(claims.id as i64)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(&format!(
            "{} WHERE s.user_id = ? ORDER BY s.id DESC",
            base_sql
        ))
        .bind(claims.id as i64)
        .fetch_all(pool)
        .await?
    };

    // 批量加载子表域名 / IP
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let mut domain_map: HashMap<i64, Vec<String>> = HashMap::new();
    let mut ip_map: HashMap<i64, Vec<String>> = HashMap::new();
    let mut vh_map: HashMap<i64, String> = HashMap::new();
    let mut dir_map: HashMap<i64, (String, String)> = HashMap::new();
    // 归属用户的 Linux 系统账号（system 模式下 PHP pool 按此账号隔离）
    let mut lu_map: HashMap<i64, String> = HashMap::new();
    if !ids.is_empty() {
        let ph = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let dsql = format!(
            "SELECT site_id, domain FROM site_domain WHERE site_id IN ({}) ORDER BY id",
            ph
        );
        let mut dq = sqlx::query_as::<_, (i64, String)>(&dsql);
        for id in &ids {
            dq = dq.bind(id);
        }
        for (sid, d) in dq.fetch_all(pool).await? {
            domain_map.entry(sid).or_default().push(d);
        }
        let isql = format!(
            "SELECT site_id, ip FROM site_ip WHERE site_id IN ({}) ORDER BY id",
            ph
        );
        let mut iq = sqlx::query_as::<_, (i64, String)>(&isql);
        for id in &ids {
            iq = iq.bind(id);
        }
        for (sid, ip) in iq.fetch_all(pool).await? {
            ip_map.entry(sid).or_default().push(ip);
        }
        // vhost 同步状态（独立 map，不进入主行 tuple）
        let vsql = format!(
            "SELECT id, vhost_state FROM site WHERE id IN ({}) ORDER BY id",
            ph
        );
        let mut vq = sqlx::query_as::<_, (i64, String)>(&vsql);
        for id in &ids {
            vq = vq.bind(id);
        }
        for (sid, state) in vq.fetch_all(pool).await? {
            vh_map.insert(sid, state);
        }
        // 站点文档根 / 日志目录（独立 map，不进入主行 tuple）
        let dirsql = format!(
            "SELECT id, web_root, log_root FROM site WHERE id IN ({}) ORDER BY id",
            ph
        );
        let mut dirq = sqlx::query_as::<_, (i64, String, String)>(&dirsql);
        for id in &ids {
            dirq = dirq.bind(id);
        }
        for (sid, w, l) in dirq.fetch_all(pool).await? {
            dir_map.insert(sid, (w, l));
        }
        // 归属用户的 Linux 系统账号（system 模式下 PHP pool 按此账号隔离）
        let mut owner_ids: Vec<i64> = rows.iter().map(|r| r.1).collect();
        owner_ids.sort_unstable();
        owner_ids.dedup();
        if !owner_ids.is_empty() {
            let ph2 = owner_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let lusql = format!("SELECT id, linux_user FROM user WHERE id IN ({})", ph2);
            let mut lq = sqlx::query_as::<_, (i64, String)>(&lusql);
            for id in &owner_ids {
                lq = lq.bind(id);
            }
            for (uid, lu) in lq.fetch_all(pool).await? {
                lu_map.insert(uid, lu);
            }
        }
    }

    let mut recs: Vec<SiteRowExt> = rows
        .into_iter()
        .map(|r| {
            let domains = domain_map.remove(&r.0).unwrap_or_default();
            let ips = ip_map.remove(&r.0).unwrap_or_default();
            (r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, domains, ips)
        })
        .collect();

    // 前端筛选（内存过滤，数据量小）
    if let Some(uid) = q.user_id {
        recs.retain(|r| r.1 == uid);
    }
    if let Some(status) = q.status
        && (status == 0 || status == 1)
    {
        recs.retain(|r| r.3 == status);
    }
    if let Some(search) = q.search {
        let s = search.trim().to_lowercase();
        if !s.is_empty() {
            recs.retain(|r| {
                r.2.to_lowercase().contains(&s)
                    || r.9.iter().any(|d| d.contains(&s))
                    || r.10.iter().any(|ip| ip.to_lowercase().contains(&s))
            });
        }
    }

    let (mut running, mut stopped) = (0usize, 0usize);
    for r in &recs {
        if r.3 == 1 {
            running += 1;
        } else {
            stopped += 1;
        }
    }
    let vmode = crate::routers::system_env::vhost_mode().await;
    let list: Vec<Value> = recs
        .iter()
        .map(|r| {
            json!({
                "id": r.0,
                "user_id": r.1,
                "owner_username": r.7.as_deref().unwrap_or(""),
                "linux_user": lu_map.get(&r.1).cloned().unwrap_or_default(),
                "name": r.2,
                "php_instance": r.8,
                "domains": r.9,
                "ips": r.10,
                "status": r.3,
                "vhost_state": vh_map.get(&r.0).cloned().unwrap_or_else(|| "pending".into()),
                "web_root": dir_map.get(&r.0).map(|d| d.0.clone()).unwrap_or_default(),
                "log_root": dir_map.get(&r.0).map(|d| d.1.clone()).unwrap_or_default(),
                "remark": r.4,
                "created_at": r.5,
                "updated_at": r.6,
            })
        })
        .collect();

    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": {
            "total": recs.len(),
            "running": running,
            "stopped": stopped,
            "vhost_mode": vmode,
            "rows": list,
        }
    })))
}

/// 可选归属用户列表：
/// admin → 全部 admin/reseller/user 账号（含自己）；reseller → 自己 + 自己的客户
pub async fn site_users(claims: ValidatedClaims) -> ZapJsonResult {
    require_manageable(&claims)?;
    if !jwt::is_admin(&claims) && !jwt::is_reseller(&claims) {
        // 普通用户归属固定为自己（归属 = 当前登录用户），无需下拉数据
        return Ok(Json(json!({
            "code": 0,
            "message": "OK",
            "data": [],
        })));
    }
    let pool = db::get_db_pool().await;
    let users: Vec<OwnerCandidate> = if jwt::is_reseller(&claims) {
        // 自己优先展示，再补充名下客户
        let mut v: Vec<OwnerCandidate> =
            sqlx::query_as("SELECT id, username, nickname FROM user WHERE id = ? AND status = 1")
                .bind(claims.id as i64)
                .fetch_all(pool)
                .await?;
        let customers: Vec<OwnerCandidate> = sqlx::query_as(
            "SELECT id, username, nickname FROM user \
             WHERE status = 1 AND owner_id = ? AND (',' || roles || ',') LIKE '%,user,%' \
             ORDER BY id DESC",
        )
        .bind(claims.id as i64)
        .fetch_all(pool)
        .await?;
        for c in customers {
            if !v.iter().any(|u| u.id == c.id) {
                v.push(c);
            }
        }
        v
    } else {
        sqlx::query_as(
            "SELECT id, username, nickname FROM user \
             WHERE status = 1 AND ((',' || roles || ',') LIKE '%,admin,%' \
                OR (',' || roles || ',') LIKE '%,reseller,%' \
                OR (',' || roles || ',') LIKE '%,user,%') \
             ORDER BY id DESC",
        )
        .fetch_all(pool)
        .await?
    };
    let list: Vec<Value> = users
        .iter()
        .map(|u| {
            json!({
                "id": u.id,
                "username": u.username,
                "nickname": u.nickname,
            })
        })
        .collect();
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": list,
    })))
}

/// 新增站点（普通用户归属自动为当前登录用户；admin/reseller 需显式指定客户）
pub async fn site_add(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<SiteAddPayload>,
) -> ZapJsonResult {
    require_manageable(&claims)?;
    let is_admin = jwt::is_admin(&claims);
    let is_reseller = jwt::is_reseller(&claims);
    let owner = if is_admin || is_reseller {
        payload
            .user_id
            .ok_or_else(|| ZapError::New(-1, "请选择站点的归属用户".to_string()))?
    } else {
        // 普通用户：归属即当前登录用户
        claims.id as i64
    };
    resolve_target_user(&claims, owner).await?;

    let domains = norm_domains(&payload.domains);
    let ips = norm_ips(&payload.ips);
    let name = fallback_name(payload.name.as_deref().unwrap_or(""), &domains);
    let status = payload.status.unwrap_or(1).clamp(0, 1);
    let remark = payload.remark.unwrap_or_default().trim().to_string();
    let php_instance = payload.php_instance.unwrap_or_default().trim().to_string();
    if !valid_php_instance(&php_instance) {
        return Err(ZapError::New(
            -1,
            "PHP 实例标识不合法（最长 120 字符，仅允许字母/数字/./_/-/@）".to_string(),
        ));
    }
    validate_site_fields(&name, &domains, &ips, &remark)?;

    let pool = db::get_db_pool().await;
    // 套餐限制：最大站点数（0 = 不限），达到上限时硬拦截
    if let Some(pkg) = crate::routers::package::package_of_user(owner).await
        && pkg.max_sites > 0
    {
        let used: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM site WHERE user_id = ?")
            .bind(owner)
            .fetch_one(pool)
            .await?;
        if used.0 >= pkg.max_sites {
            return Err(ZapError::New(
                -1,
                format!(
                    "已达套餐「{}」的站点上限 {} 个（当前 {} 个），无法继续创建",
                    pkg.name, pkg.max_sites, used.0
                ),
            ));
        }
    }
    let now = chrono::Local::now().timestamp();

    let mut tx = pool.begin().await?;
    ensure_domains_unique(&mut tx, &domains, 0).await?;
    let r = sqlx::query(
        "INSERT INTO site (user_id, name, php_instance, status, remark, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(owner)
    .bind(&name)
    .bind(&php_instance)
    .bind(status)
    .bind(&remark)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let id = r.last_insert_rowid();
    for d in &domains {
        sqlx::query("INSERT INTO site_domain (site_id, domain) VALUES (?, ?)")
            .bind(id)
            .bind(d)
            .execute(&mut *tx)
            .await?;
    }
    for ip in &ips {
        sqlx::query("INSERT INTO site_ip (site_id, ip) VALUES (?, ?)")
            .bind(id)
            .bind(ip)
            .execute(&mut *tx)
            .await?;
    }
    // 站点文档根 / 日志目录：规划到归属用户家目录下（vhost 同步时由 zapexec 递归创建）
    let (web_root, log_root) = site_dirs_for(owner, &name, id).await?;
    if !web_root.is_empty() {
        sqlx::query("UPDATE site SET web_root = ?, log_root = ? WHERE id = ?")
            .bind(&web_root)
            .bind(&log_root)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "site_create",
        &format!("id={}", id),
        &format!(
            "user_id={} name={} domains={} ips={} php_instance={}",
            owner,
            name,
            domains.join(","),
            ips.join(","),
            php_instance
        ),
    )
    .await;
    info!(
        "site create: id={} user_id={} domains={:?}",
        id, owner, domains
    );

    Ok(Json(json!({
        "code": 0,
        "message": "站点添加成功",
        "data": { "id": id }
    })))
}

/// 更新站点（名称 / 多域名 / 多 IP / 状态 / 备注 / 归属用户转移）
pub async fn site_update(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<SiteUpdatePayload>,
) -> ZapJsonResult {
    require_manageable(&claims)?;
    site_in_scope(&claims, payload.id).await?;

    let pool = db::get_db_pool().await;
    let row: Option<(i64, String, i32, String, String, String)> = sqlx::query_as(
        "SELECT user_id, name, status, remark, php_instance, web_root FROM site WHERE id = ?",
    )
    .bind(payload.id)
    .fetch_optional(pool)
    .await?;
    let Some((uid, old_name, mut status, mut remark, mut php_instance, old_web_root)) = row else {
        return Err(ZapError::New(-1, "站点不存在".to_string()));
    };

    // 归属转移
    let new_owner = if let Some(nid) = payload.user_id {
        if !jwt::is_admin(&claims) && !jwt::is_reseller(&claims) {
            return Err(ZapError::New(-1, "普通用户无权转移站点归属".to_string()));
        }
        nid
    } else {
        uid
    };
    if new_owner != uid {
        resolve_target_user(&claims, new_owner).await?;
    }

    // 域名 / IP：不传则保留原值，传入则整体覆盖
    let domains = if let Some(ds) = &payload.domains {
        norm_domains(ds)
    } else {
        sqlx::query_as::<_, (String,)>(
            "SELECT domain FROM site_domain WHERE site_id = ? ORDER BY id",
        )
        .bind(payload.id)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| r.0)
        .collect()
    };
    let ips = if let Some(is) = &payload.ips {
        norm_ips(is)
    } else {
        sqlx::query_as::<_, (String,)>("SELECT ip FROM site_ip WHERE site_id = ? ORDER BY id")
            .bind(payload.id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| r.0)
            .collect()
    };
    let name = match &payload.name {
        Some(n) => {
            let n = fallback_name(n, &domains);
            // 若编辑后仅剩域名而旧名称为空字符串（理论上不会发生），做一次兜底
            if n.is_empty() { old_name.clone() } else { n }
        }
        None => old_name.clone(),
    };
    if let Some(s) = payload.status {
        status = s.clamp(0, 1);
    }
    if let Some(rk) = &payload.remark {
        remark = rk.trim().to_string();
    }
    if let Some(p) = &payload.php_instance {
        let p = p.trim().to_string();
        if !valid_php_instance(&p) {
            return Err(ZapError::New(
                -1,
                "PHP 实例标识不合法（最长 120 字符，仅允许字母/数字/./_/-/@）".to_string(),
            ));
        }
        php_instance = p;
    }
    validate_site_fields(&name, &domains, &ips, &remark)?;

    let now = chrono::Local::now().timestamp();
    let mut tx = pool.begin().await?;
    let r = sqlx::query(
        "UPDATE site SET user_id = ?, name = ?, php_instance = ?, status = ?, remark = ?, updated_at = ? \
         WHERE id = ?",
    )
    .bind(new_owner)
    .bind(&name)
    .bind(&php_instance)
    .bind(status)
    .bind(&remark)
    .bind(now)
    .bind(payload.id)
    .execute(&mut *tx)
    .await?;
    if r.rows_affected() == 0 {
        tx.rollback().await?;
        return Err(ZapError::New(-1, "站点不存在".to_string()));
    }
    // 域名整体覆盖：先校验唯一性，再重建
    if payload.domains.is_some() {
        ensure_domains_unique(&mut tx, &domains, payload.id).await?;
        sqlx::query("DELETE FROM site_domain WHERE site_id = ?")
            .bind(payload.id)
            .execute(&mut *tx)
            .await?;
        for d in &domains {
            sqlx::query("INSERT INTO site_domain (site_id, domain) VALUES (?, ?)")
                .bind(payload.id)
                .bind(d)
                .execute(&mut *tx)
                .await?;
        }
    }
    if payload.ips.is_some() {
        sqlx::query("DELETE FROM site_ip WHERE site_id = ?")
            .bind(payload.id)
            .execute(&mut *tx)
            .await?;
        for ip in &ips {
            sqlx::query("INSERT INTO site_ip (site_id, ip) VALUES (?, ?)")
                .bind(payload.id)
                .bind(ip)
                .execute(&mut *tx)
                .await?;
        }
    }
    // 新式站点（DB 已记录 web_root）跟随归属转移 / 改名刷新目录规划；
    // 老站点（web_root 为空）保持默认 data/www 不迁移
    if !old_web_root.is_empty() && (new_owner != uid || name != old_name) {
        let (web_root, log_root) = site_dirs_for(new_owner, &name, payload.id).await?;
        sqlx::query("UPDATE site SET web_root = ?, log_root = ? WHERE id = ?")
            .bind(&web_root)
            .bind(&log_root)
            .bind(payload.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "site_update",
        &format!("id={}", payload.id),
        &format!(
            "user_id={} name={} domains={} status={}",
            new_owner,
            name,
            domains.join(","),
            status
        ),
    )
    .await;
    info!("site update: id={} domains={:?}", payload.id, domains);

    Ok(Json(json!({
        "code": 0,
        "message": "更新成功"
    })))
}

/// 删除站点（可批量，连带删除其域名 / IP 绑定）
pub async fn site_delete(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<SiteDeletePayload>,
) -> ZapJsonResult {
    require_manageable(&claims)?;
    if payload.ids.is_empty() {
        return Err(ZapError::New(-1, "请选择要删除的站点".to_string()));
    }
    for id in &payload.ids {
        site_in_scope(&claims, *id).await?;
    }

    // 先清理 Nginx vhost（尽力而为，失败不阻塞删除）
    for id in &payload.ids {
        if let Ok(resp) = crate::zapexec::call(Request::SiteVhostRemove {
            site_id: *id,
            name: String::new(),
        })
        .await
            && resp.code != 0
        {
            tracing::warn!("remove vhost for site {} failed: {}", id, resp.message);
        }
    }

    let placeholders = payload
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let pool = db::get_db_pool().await;
    let mut tx = pool.begin().await?;

    let ssql = format!("DELETE FROM site WHERE id IN ({})", placeholders);
    let mut q = sqlx::query(&ssql);
    for id in &payload.ids {
        q = q.bind(id);
    }
    let r = q.execute(&mut *tx).await?;
    let deleted = r.rows_affected();

    let dsql = format!(
        "DELETE FROM site_domain WHERE site_id IN ({})",
        placeholders
    );
    let mut dq = sqlx::query(&dsql);
    for id in &payload.ids {
        dq = dq.bind(id);
    }
    dq.execute(&mut *tx).await?;

    let isql = format!("DELETE FROM site_ip WHERE site_id IN ({})", placeholders);
    let mut iq = sqlx::query(&isql);
    for id in &payload.ids {
        iq = iq.bind(id);
    }
    iq.execute(&mut *tx).await?;

    tx.commit().await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "site_delete",
        &format!("ids={:?}", payload.ids),
        &format!("deleted={}", deleted),
    )
    .await;
    info!("site delete: ids={:?} deleted={}", payload.ids, deleted);

    Ok(Json(json!({
        "code": 0,
        "message": format!("已删除 {} 个站点", deleted)
    })))
}

#[derive(Debug, Deserialize)]
pub struct SiteSyncPayload {
    pub id: i64,
}

/// 将站点档案同步为 Nginx vhost：按域名/状态/PHP 实例渲染 conf → nginx -t → reload
pub async fn site_sync(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<SiteSyncPayload>,
) -> ZapJsonResult {
    require_manageable(&claims)?;
    site_in_scope(&claims, payload.id).await?;
    let (msg, data, name, status) = match sync_one_site(payload.id).await {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    let _ = audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "site_sync",
        &format!("id={}", payload.id),
        &format!("name={} status={} {}", name, status, msg),
    )
    .await;
    info!("site sync: id={} status={}", payload.id, status);
    Ok(Json(json!({ "code": 0, "message": msg, "data": data })))
}

/// 单个站点全量同步核心（幂等，被 /site/sync、/site/sync_all 与数据迁移复用）
pub(crate) async fn sync_one_site(
    id: i64,
) -> Result<(String, Option<serde_json::Value>, String, i32), ZapError> {
    let pool = db::get_db_pool().await;

    // 站点 + 归属用户（LEFT JOIN：站点可能无主 / 用户已删）
    let row: Option<SyncOneRow> = sqlx::query_as(
        "SELECT s.name, s.status, s.php_instance, s.web_root, s.log_root,
                u.id, u.home_dir, u.linux_user, u.fpm_pool, u.fpm_spec_ref, u.owner_id
         FROM site s LEFT JOIN user u ON u.id = s.user_id
         WHERE s.id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((
        name,
        status,
        php_instance,
        web_root,
        log_root,
        uid,
        uhome,
        _ulu,
        ufpm,
        uref,
        uowner,
    )) = row
    else {
        return Err(ZapError::New(-1, "站点不存在".to_string()));
    };
    let mode = crate::routers::system_env::vhost_mode().await;

    // 域名 → server_name
    let mut domains = Vec::new();
    let dsql = "SELECT domain FROM site_domain WHERE site_id = ? ORDER BY id";
    let dq = sqlx::query_as::<_, (String,)>(dsql);
    for (d,) in dq.bind(id).fetch_all(pool).await? {
        let d = d.trim().to_string();
        if !d.is_empty() {
            domains.push(d);
        }
    }

    // 运行实体准备（幂等）：
    // - system 模式：确保归属用户有 Linux 账号 + 独立用户家目录，文件属主 = linux_user
    // - www 模式：确保归属用户家目录骨架（www:www），文件属主 = www
    let mut owner_user: Option<String> = None;
    if mode == "system" {
        if let Some(uid) = uid {
            crate::routers::user::ensure_user_runtime(uid)
                .await
                .map_err(|e| ZapError::New(-1, e))?;
            let lu: Option<String> = sqlx::query_scalar("SELECT linux_user FROM user WHERE id = ?")
                .bind(uid)
                .fetch_one(pool)
                .await?;
            if let Some(lu) = lu.filter(|s| !s.is_empty()) {
                owner_user = Some(lu);
            }
        }
    } else if let Some(uid) = uid
        && let Err(e) = crate::routers::user::ensure_user_runtime(uid).await
    {
        warn!("初始化用户运行实体失败(id={}): {}", uid, e);
    }

    // PHP 通道：
    // - system 模式：先为归属用户同步专属 pool，通道 = /var/run/php-fpm-{linux_user}-{ver}.sock
    // - www 模式：全局实例 socket（info.yaml 解析 / 命名推导）
    let php_socket = if php_instance.is_empty() {
        None
    } else if mode == "system" {
        match &owner_user {
            Some(lu) => {
                // 解析用户最终 pool 规格：存量自定义 fpm_pool → 模板/inherit(继承 reseller) → 全局默认
                let spec = crate::routers::fpm_spec::resolve_user_spec(
                    ufpm.as_deref(),
                    uref.as_deref().unwrap_or(""),
                    uowner,
                )
                .await;
                let resp = crate::zapexec::call(Request::PhpPoolSync {
                    php_instance: php_instance.clone(),
                    linux_user: lu.clone(),
                    home_dir: uhome.unwrap_or_default(),
                    spec,
                })
                .await?;
                if resp.code != 0 {
                    return Err(ZapError::New(
                        resp.code,
                        format!("PHP-FPM pool 同步失败：{}", resp.message),
                    ));
                }
                Some(format!(
                    "/var/run/php-fpm-{lu}-{}.sock",
                    php_version_suffix(&php_instance)
                ))
            }
            None => Some(resolve_php_socket(&php_instance).await?),
        }
    } else {
        Some(resolve_php_socket(&php_instance).await?)
    };

    let web_root_opt = (!web_root.trim().is_empty()).then_some(web_root);
    let log_root_opt = (!log_root.trim().is_empty()).then_some(log_root);
    let resp = crate::zapexec::call(Request::SiteVhostSync {
        site_id: id,
        name: name.clone(),
        domains,
        enabled: status == 1,
        php_socket,
        web_root: web_root_opt,
        log_root: log_root_opt,
        owner_user,
    })
    .await?;

    let state = if resp.code == 0 { "synced" } else { "error" };
    let _ = sqlx::query("UPDATE site SET vhost_state = ? WHERE id = ?")
        .bind(state)
        .bind(id)
        .execute(pool)
        .await;

    if resp.code != 0 {
        return Err(ZapError::New(
            resp.code,
            format!("vhost 同步失败：{}", resp.message),
        ));
    }

    info!("site sync core: id={} status={}", id, status);
    Ok((resp.message, resp.data, name, status))
}

/// 全部站点按当前模式重同步：vhost 模式开关切换后的「再同步」入口
pub async fn site_sync_all(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
) -> ZapJsonResult {
    require_manageable(&claims)?;
    let pool = db::get_db_pool().await;
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM site ORDER BY id")
        .fetch_all(pool)
        .await?;
    if ids.is_empty() {
        return Ok(Json(json!({ "code": 0, "message": "没有需要同步的站点" })));
    }
    let mut ok = 0usize;
    let mut fails: Vec<String> = Vec::new();
    for sid in ids {
        match sync_one_site(sid).await {
            Ok(_) => ok += 1,
            Err(e) => fails.push(format!("站点 #{}：{}", sid, e)),
        }
    }
    let fail = fails.len();
    let summary = if fail == 0 {
        format!("已按当前模式重同步 {} 个站点", ok)
    } else {
        let detail = fails.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        format!("成功 {} 个，失败 {} 个（{}…）", ok, fail, detail)
    };
    let _ = audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "site_sync_all",
        "all",
        &summary,
    )
    .await;
    info!("site sync all: {}", summary);
    if fail > 0 {
        return Err(ZapError::New(-1, format!("部分站点同步失败：{}", summary)));
    }
    Ok(Json(json!({ "code": 0, "message": summary })))
}

/// PHP 实例 → 版本后缀：php8.3 → 8.3，php74 → 74
fn php_version_suffix(php_instance: &str) -> String {
    php_instance.trim_start_matches("php").to_string()
}

/// 解析 PHP 实例的 FPM 通道：
/// 1) info.yaml 登记的 php_socket / fpm_socket / expose(unix:/tcp:)；
/// 2) 否则按官方包命名约定推导（php8.3 → /var/run/php-fpm-8.3.sock，php74 → /var/run/php-fpm-74.sock）
async fn resolve_php_socket(php_instance: &str) -> Result<String, ZapError> {
    let resp = crate::zapexec::call(Request::AppstoreInstalled).await?;
    if resp.code == 0
        && let Some(data) = &resp.data
        && let Some(items) = data.get("items").and_then(|v| v.as_array())
    {
        for it in items {
            if it.get("instance").and_then(|v| v.as_str()) != Some(php_instance) {
                continue;
            }
            let info = it.get("info").unwrap_or(&serde_json::Value::Null);
            for key in ["php_socket", "fpm_socket"] {
                if let Some(v) = info.get(key).and_then(|v| v.as_str()) {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Ok(v.to_string());
                    }
                }
            }
            if let Some(v) = info.get("expose").and_then(|v| v.as_str()) {
                for seg in v.split(['\n', ',']) {
                    let seg = seg.trim();
                    if let Some(rest) = seg.strip_prefix("unix:") {
                        let rest = rest.trim();
                        if !rest.is_empty() {
                            return Ok(format!("unix:{rest}"));
                        }
                    }
                    if let Some(rest) = seg.strip_prefix("tcp:") {
                        let rest = rest.trim();
                        if !rest.is_empty() {
                            return Ok(rest.to_string());
                        }
                    }
                }
            }
        }
    }
    // 官方包命名推导
    let ver = php_instance
        .strip_prefix("php")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            ZapError::New(
                -1,
                format!(
                    "无法确定 PHP 实例 {php_instance} 的 FPM socket：\
                     实例未登记 php_socket 且命名不是 php<版本> 形式"
                ),
            )
        })?;
    Ok(format!("/var/run/php-fpm-{ver}.sock"))
}
