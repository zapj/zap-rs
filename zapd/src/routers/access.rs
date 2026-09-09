//! 路由 → 角色 权限矩阵中间件（fail-closed）。
//!
//! 背景：角色校验原先散落在各 handler 里手写 `require_admin(&claims)`，
//! `system_config` / `system_job` / `system_info` 等端点漏做校验，导致任意已登录
//! 的普通用户都能改主机名/时区、启停或安装 sshd、kill 任意进程、改 DNS resolver、
//! 启停全局定时任务。
//!
//! 这里把「路径前缀 → 所需角色」收敛成一张表，由中间件统一执行：
//!
//! - **默认拒绝（fail-closed）**：未显式登记的路径一律要求 admin。
//!   新增接口忘记登记时默认不可被普通用户访问，而不是默认放行。
//! - 表中只登记「放宽」的条目（Public / User / Reseller）。
//! - 资源归属（owner）维度仍由 handler 自行收敛：中间件只回答"你有没有资格敲这扇门"，
//!   "你能看到哪几条数据"依旧是 handler 的职责（如站点、证书、文件、SSH 连接）。
//!
//! ## 两层校验
//!
//! 1. **角色下限**（本表 `Required`）：接口的硬门槛，代码内固定，不可配置。
//! 2. **动作级权限点**（本表第三列 ns + `role_permissions` 表）：可运营配置。
//!    请求按方法展开成 `{ns}:view`（GET/HEAD）或 `{ns}:edit`（其余），
//!    用户所属任一角色在 `role_permissions` 中持有该 key 才放行；admin 恒直通。
//!
//! 前端 `v-permission` / 菜单树只是体验层，**不是安全边界**，安全边界在这里。
//!
//! 匹配方式：先剥掉 URL 前缀与 `/api`，再取**最长前缀**命中项。

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock, RwLock},
};

use axum::{
    Json,
    extract::Request,
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tracing::warn;

use crate::zap::jwt::{self, Claims};

/// 访问某接口所需的最低角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Required {
    /// 免鉴权：健康检查、登录
    Public,
    /// 任意已登录用户（含 demo）；数据范围由 handler 按 owner 收敛
    User,
    /// admin 或 reseller
    Reseller,
    /// 仅 admin
    Admin,
}

impl Required {
    fn label(self) -> &'static str {
        match self {
            Required::Public => "public",
            Required::User => "user",
            Required::Reseller => "reseller",
            Required::Admin => "admin",
        }
    }
}

/// 权限矩阵：`(路径前缀, 所需角色下限, 动作级权限命名空间)`。
///
/// - 第三列为 `None` 表示只受角色下限约束（个人接口，无运营配置项）；
/// - 为 `Some(ns)` 时，请求还需通过 `{ns}:view` / `{ns}:edit` 权限点校验。
///
/// admin-only 的条目即使与默认值相同也显式列出：它们同时是权限点的登记处，
/// 漏登记等于该接口绕过权限点校验（仅剩角色下限）。
const RULES: &[(&str, Required, Option<&str>)] = &[
    // ── 免鉴权 ───────────────────────────────────────────────
    ("/health", Required::Public, None),
    ("/auth/login", Required::Public, None),
    // ── 登录态即可访问的个人接口 ─────────────────────────────
    ("/auth", Required::User, None),
    ("/user", Required::User, None),
    // 菜单树：所有角色渲染侧边栏都要读
    ("/system/menus/tree", Required::User, Some("system.menu")),
    // 文件管理：handler 内已把非管理员限制在自己的 home 与私有 tmp
    ("/system/files", Required::User, Some("system.file")),
    // 仪表盘：普通用户首页（cpanel 视图）会读取 /system/info
    ("/system/info", Required::User, Some("system.monitor")),
    ("/system/status", Required::Admin, Some("system.monitor")),
    ("/system/overview", Required::Admin, Some("system.monitor")),
    // ── reseller 可管理名下客户 ──────────────────────────────
    ("/system/user", Required::Reseller, Some("system.user")),
    (
        "/system/user/resellers",
        Required::Admin,
        Some("system.user"),
    ),
    (
        "/system/user/home_sync",
        Required::Admin,
        Some("system.user"),
    ),
    (
        "/system/package",
        Required::Reseller,
        Some("system.package"),
    ),
    (
        "/system/fpm-specs/list",
        Required::Reseller,
        Some("system.fpm_spec"),
    ),
    ("/site/users", Required::Reseller, Some("site")),
    ("/site/sync_all", Required::Reseller, Some("site")),
    // ── 业务对象：handler 按 owner / reseller 归属收敛可见范围 ─
    ("/site", Required::User, Some("site")),
    ("/ssl", Required::User, Some("ssl")),
    ("/terminal", Required::User, Some("terminal")),
    // ── 应用商店：源码与脚本管理仅管理员，其余按 handler 内规则 ─
    (
        "/appstore/repos/add",
        Required::Admin,
        Some("appstore.repo"),
    ),
    (
        "/appstore/repos/remove",
        Required::Admin,
        Some("appstore.repo"),
    ),
    (
        "/appstore/repos/update",
        Required::Admin,
        Some("appstore.repo"),
    ),
    ("/appstore/script", Required::Admin, Some("appstore.script")),
    (
        "/appstore/scripts/tree",
        Required::Admin,
        Some("appstore.script"),
    ),
    ("/appstore", Required::User, Some("appstore")),
    // ── 其余 admin-only 接口：显式登记权限点 ─────────────────
    ("/system/config", Required::Admin, Some("system.config")),
    ("/system/nginx", Required::Admin, Some("service.nginx")),
    (
        "/system/service-conf",
        Required::Admin,
        Some("service.conf"),
    ),
    ("/system/ip", Required::Admin, Some("system.ip")),
    ("/system/role", Required::Admin, Some("system.role")),
    ("/system/menus", Required::Admin, Some("system.menu")),
    ("/system/audit", Required::Admin, Some("system.audit")),
    ("/system/update", Required::Admin, Some("system.update")),
    ("/system/migrate", Required::Admin, Some("system.migrate")),
    ("/system/env", Required::Admin, Some("system.env")),
    ("/system/cron", Required::Admin, Some("system.cron")),
    ("/system/job", Required::Admin, Some("system.job")),
    ("/dev", Required::Admin, Some("dev")),
];

/// 权限点命名空间的中文名（用于角色权限配置页与权限目录接口）。
const NS_LABELS: &[(&str, &str)] = &[
    ("system.menu", "菜单管理"),
    ("system.file", "文件管理"),
    ("system.monitor", "服务器状态"),
    ("system.user", "用户管理"),
    ("system.package", "套餐管理"),
    ("system.fpm_spec", "PHP-FPM 规格"),
    ("system.role", "角色权限"),
    ("system.audit", "审计日志"),
    ("system.update", "系统更新"),
    ("system.migrate", "数据迁移"),
    ("system.env", "运行环境"),
    ("system.cron", "计划任务"),
    ("system.job", "全局任务"),
    ("service.nginx", "Nginx 服务"),
    ("service.conf", "服务配置"),
    ("system.ip", "IP 池"),
    ("system.config", "服务器配置"),
    ("site", "站点管理"),
    ("ssl", "SSL 证书"),
    ("terminal", "终端与密钥"),
    ("appstore", "应用商店"),
    ("appstore.repo", "应用源管理"),
    ("appstore.script", "自定义脚本"),
    ("dev", "开发者接口"),
];

/// 剥离 URL 前缀（`server.url_prefix`）与 `/api`，得到与 `RULES` 对齐的路径。
///
/// 中间件挂在 `api_routers()` 上，`Router::nest` 通常已剥掉外层前缀；
/// 这里幂等再剥一次，保证前缀启用/未启用、以及中间件挂载层级变化时行为一致。
fn normalize(path: &str) -> String {
    let mut p = path;
    if let Some(rest) = strip_prefix(p, &crate::config::url_prefix_path()) {
        p = rest;
    }
    if let Some(rest) = strip_prefix(p, "/api") {
        p = rest;
    }
    if p.is_empty() {
        "/".to_string()
    } else {
        p.to_string()
    }
}

/// 按路径段剥离前缀：`/api/health` - `/api` = `/health`；`/api` - `/api` = `/`。
fn strip_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return None;
    }
    match path.strip_prefix(prefix) {
        Some("") => Some("/"),
        Some(rest) if rest.starts_with('/') => Some(rest),
        _ => None,
    }
}

/// 前缀是否命中（按路径段边界，避免 `/appstore/script` 命中 `/appstore/scripts/tree`）。
fn prefix_hit(path: &str, prefix: &str) -> bool {
    match path.strip_prefix(prefix) {
        Some("") => true,
        Some(rest) => rest.starts_with('/'),
        None => false,
    }
}

/// 查询路径的命中项：最长前缀命中；未命中 → `(Admin, None)`（默认拒绝）。
fn lookup(path: &str) -> (Required, Option<&'static str>) {
    let mut best: Option<(usize, Required, Option<&'static str>)> = None;
    for (prefix, req, ns) in RULES {
        if !prefix_hit(path, prefix) {
            continue;
        }
        if best.is_none_or(|(len, _, _)| prefix.len() > len) {
            best = Some((prefix.len(), *req, *ns));
        }
    }
    best.map(|(_, r, ns)| (r, ns))
        .unwrap_or((Required::Admin, None))
}

/// 查询路径所需角色（兼容旧调用与测试）。
fn required_for(path: &str) -> Required {
    lookup(path).0
}

/// 请求实际需要的权限点：`{ns}:view`（GET/HEAD）或 `{ns}:edit`（其余方法）。
///
/// 未登记权限点的接口返回 `None`，此时只有角色下限生效（如 `/user/*`、`/auth/*`）。
pub fn perm_key_for(path: &str, method: &Method) -> Option<String> {
    let ns = lookup(path).1?;
    let action = if method == Method::GET || method == Method::HEAD {
        "view"
    } else {
        "edit"
    };
    Some(format!("{ns}:{action}"))
}

/// 权限目录项：一个命名空间 = 一个可勾选的模块，含 view / edit 两个动作。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermGroup {
    pub ns: &'static str,
    pub label: &'static str,
    /// 该模块的动作列表：`[view, edit]`
    pub actions: Vec<PermAction>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PermAction {
    pub key: String,
    pub label: &'static str,
}

/// 权限目录（角色权限配置页的数据源）：去重自 `RULES`，顺序与矩阵一致。
pub fn permission_catalog() -> Vec<PermGroup> {
    let mut out: Vec<PermGroup> = Vec::new();
    for (_, _, ns_opt) in RULES {
        let Some(ns) = ns_opt else { continue };
        let ns: &'static str = ns;
        if out.iter().any(|g| g.ns == ns) {
            continue;
        }
        let label = NS_LABELS
            .iter()
            .find(|(k, _)| *k == ns)
            .map(|(_, l)| *l)
            .unwrap_or(ns);
        out.push(PermGroup {
            ns,
            label,
            actions: vec![
                PermAction {
                    key: format!("{ns}:view"),
                    label: "查看",
                },
                PermAction {
                    key: format!("{ns}:edit"),
                    label: "操作",
                },
            ],
        });
    }
    out
}

/// 全部合法权限点（用于校验写入，拒绝脏数据）。
pub fn all_perm_keys() -> HashSet<String> {
    permission_catalog()
        .into_iter()
        .flat_map(|g| g.actions.into_iter().map(|a| a.key))
        .collect()
}

/// 命名空间的最低角色下限（同 ns 多条规则取最宽松的一条，代表"该模块对谁开放"）。
fn ns_min_required(ns: &str) -> Required {
    let mut min = Required::Admin;
    for (_, req, rns) in RULES {
        if *rns == Some(ns) && rank(*req) < rank(min) {
            min = *req;
        }
    }
    min
}

fn rank(r: Required) -> u8 {
    match r {
        Required::Public => 0,
        Required::User => 1,
        Required::Reseller => 2,
        Required::Admin => 3,
    }
}

/// 内置角色的默认权限点：由权限矩阵推导，保证「升级前后行为一致」。
///
/// - admin：全部（且运行时恒直通，避免配置失误把自己锁死）
/// - reseller：开放给 User 与 Reseller 的模块
/// - 其它内置角色（user / demo）：仅开放给 User 的模块
pub fn default_permissions_for(role_key: &str) -> Vec<String> {
    let is_admin = role_key == "admin";
    let is_reseller = role_key == "reseller";
    permission_catalog()
        .into_iter()
        .filter(|g| {
            is_admin
                || ns_min_required(g.ns) == Required::User
                || (is_reseller && ns_min_required(g.ns) == Required::Reseller)
        })
        .flat_map(|g| g.actions.into_iter().map(|a| a.key))
        .collect()
}

// ── 角色权限缓存 ───────────────────────────────────────────

type PermMap = HashMap<String, HashSet<String>>;
static PERM_CACHE: OnceLock<RwLock<Option<Arc<PermMap>>>> = OnceLock::new();

fn cache_slot() -> &'static RwLock<Option<Arc<PermMap>>> {
    PERM_CACHE.get_or_init(|| RwLock::new(None))
}

/// 角色权限变更后调用，立即失效缓存（撤销权限不必等过期）。
pub fn invalidate_perm_cache() {
    if let Ok(mut guard) = cache_slot().write() {
        *guard = None;
    }
}

async fn load_perm_map() -> PermMap {
    // 数据库不可用时返回空表（非 admin 一律拒绝），而不是 panic 掉整个请求。
    let Some(pool) = crate::db::get_db_pool_opt().await else {
        return PermMap::new();
    };
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT r.role_key, p.perm_key FROM role_permissions p JOIN roles r ON r.id = p.role_id",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut map: PermMap = HashMap::new();
    for (role, key) in rows {
        map.entry(role).or_default().insert(key);
    }
    map
}

/// `role_key → 权限点集合`（进程内缓存，写操作通过 `invalidate_perm_cache` 失效）。
pub async fn perm_map() -> Arc<PermMap> {
    if let Ok(guard) = cache_slot().read()
        && let Some(cached) = guard.as_ref()
    {
        return cached.clone();
    }
    let map = Arc::new(load_perm_map().await);
    if let Ok(mut guard) = cache_slot().write() {
        *guard = Some(map.clone());
    }
    map
}

/// 一组角色拥有的全部权限点（供 `/user/info` 回传前端做体验层控制）。
///
/// 注意：这里的结果**只用于前端展示与按钮禁用**，真正的拦截在 `guard` 里。
pub async fn permissions_of_roles(roles: &str) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    let keys: Vec<&str> = roles
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect();

    // admin 与运行时直通保持一致：视为持有全部权限点
    if keys.contains(&"admin") {
        set.extend(all_perm_keys());
    } else {
        let map = perm_map().await;
        for r in &keys {
            if let Some(s) = map.get(*r) {
                set.extend(s.iter().cloned());
            }
        }
    }

    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
}

/// 用户（其任一角色）是否持有该权限点。
pub fn role_has_perm(map: &PermMap, claims: &Claims, key: &str) -> bool {
    claims
        .roles
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .any(|r| map.get(r).is_some_and(|set| set.contains(key)))
}

fn satisfies(claims: &Claims, required: Required) -> bool {
    match required {
        Required::Public | Required::User => true,
        Required::Reseller => jwt::is_admin(claims) || jwt::is_reseller(claims),
        Required::Admin => jwt::is_admin(claims),
    }
}

fn deny(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "code": -1, "message": message }))).into_response()
}

/// 取请求凭据：优先 `Authorization: Bearer`，回退查询串 `?token=`。
///
/// 浏览器 WebSocket 无法自定义请求头，终端与实时日志端点只能把 token 放在
/// query 里（见 `ssh_terminal::ws_terminal`、`appstore::ws_log`），
/// 因此两种取值方式必须等价，否则 WebSocket 会被门禁挡在 401。
fn token_from_request(req: &Request) -> Option<String> {
    if let Some(t) = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
    {
        return Some(t.to_string());
    }
    // token 为 JWT（base64url）或 `zap_` 开头的十六进制，均不含需转义字符
    req.uri().query()?.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == "token").then(|| v.to_string())
    })
}

/// 统一角色门禁中间件（挂在 `api_routers()` 上，覆盖所有 `/api/*` 接口）。
#[allow(clippy::result_large_err)] // axum 中间件约定 Result<Response, Response>
pub async fn guard(req: Request, next: Next) -> Result<Response, Response> {
    let path = normalize(req.uri().path());
    let method = req.method().clone();
    let required = required_for(&path);
    if required == Required::Public {
        return Ok(next.run(req).await);
    }

    // 先取 owned token 再异步解析，避免借用 req 跨 await
    let bearer = token_from_request(&req);

    let claims = match bearer.as_deref() {
        Some(token) => jwt::claims_from_token(token).await,
        None => None,
    };

    let Some(claims) = claims else {
        return Err(deny(
            StatusCode::UNAUTHORIZED,
            "未登录或登录已过期，请重新登录",
        ));
    };

    if !satisfies(&claims, required) {
        warn!(
            "access denied: user={} path={} required={}",
            claims.sub,
            path,
            required.label()
        );
        return Err(deny(
            StatusCode::FORBIDDEN,
            "权限不足，该操作需要更高角色权限",
        ));
    }

    // 第二层：动作级权限点（`role_permissions`）。admin 恒直通，避免配置失误锁死内置管理员。
    if let Some(key) = perm_key_for(&path, &method)
        && !jwt::is_admin(&claims)
        && !role_has_perm(perm_map().await.as_ref(), &claims, &key)
    {
        warn!(
            "permission denied: user={} path={} required={}",
            claims.sub, path, key
        );
        return Err(deny(
            StatusCode::FORBIDDEN,
            &format!("权限不足，需要权限点：{key}"),
        ));
    }

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        middleware,
        routing::{get, post},
    };
    use tower::ServiceExt; // oneshot

    fn app() -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/system/config/time", get(|| async { "ok" }))
            .route("/system/user/list", get(|| async { "ok" }))
            .route("/appstore/ws/{run_id}", get(|| async { "ok" }))
            .route("/site/list", get(|| async { "ok" }))
            .route("/site/add", post(|| async { "ok" }))
            .layer(middleware::from_fn(guard))
    }

    async fn status(uri: &str, token: Option<&str>) -> StatusCode {
        send(Method::GET, uri, token).await
    }

    async fn send(method: Method, uri: &str, token: Option<&str>) -> StatusCode {
        let mut req = axum::http::Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        app()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
            .status()
    }

    fn token(roles: &str) -> String {
        jwt::generate_jwt_token("tester".to_string(), 9, roles, false).unwrap()
    }

    /// 预置权限缓存（测试进程内共享；内容固定，重复写入等价，不会互相干扰）。
    fn prime_cache() {
        let mut map = PermMap::new();
        map.insert(
            "user".to_string(),
            ["site:view", "site:edit", "appstore:view"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        map.insert(
            "reseller".to_string(),
            ["system.user:view", "site:view"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        // 固定内容，直接覆盖：保证并发测试读到的是同一份数据
        *cache_slot().write().unwrap() = Some(Arc::new(map));
    }

    #[tokio::test]
    async fn middleware_enforces_perm_keys() {
        prime_cache();
        let admin = token("admin");
        let user = token("user");
        let reseller = token("reseller");
        let demo = token("demo");

        // admin 恒直通（即使未配置任何权限点）
        assert_eq!(status("/site/list", Some(&admin)).await, StatusCode::OK);

        // 持有 site:view → 放行；未持有（demo）→ 拒绝，即便角色下限已满足
        assert_eq!(status("/site/list", Some(&user)).await, StatusCode::OK);
        assert_eq!(
            status("/site/list", Some(&demo)).await,
            StatusCode::FORBIDDEN
        );

        // 写操作按 :edit 校验：user 有 edit，reseller 只有 view
        assert_eq!(
            send(Method::POST, "/site/add", Some(&user)).await,
            StatusCode::OK
        );
        assert_eq!(
            send(Method::POST, "/site/add", Some(&reseller)).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn middleware_enforces_matrix() {
        prime_cache();
        let admin = token("admin");
        let reseller = token("reseller");
        let user = token("user");
        let demo = token("demo");

        // 免鉴权
        assert_eq!(status("/health", None).await, StatusCode::OK);

        // 系统级接口：无凭据 401，非 admin 403，admin 放行
        assert_eq!(
            status("/system/config/time", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status("/system/config/time", Some(&user)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status("/system/config/time", Some(&reseller)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status("/system/config/time", Some(&demo)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status("/system/config/time", Some(&admin)).await,
            StatusCode::OK
        );

        // reseller 层级：reseller 放行、普通用户拒绝
        assert_eq!(
            status("/system/user/list", Some(&reseller)).await,
            StatusCode::OK
        );
        assert_eq!(
            status("/system/user/list", Some(&user)).await,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status("/system/user/list", Some(&admin)).await,
            StatusCode::OK
        );

        // 伪造/过期 token 视为未登录
        assert_eq!(
            status("/system/config/time", Some("not-a-token")).await,
            StatusCode::UNAUTHORIZED
        );
    }

    /// 浏览器 WebSocket 不能自定义请求头，token 只能放 query：必须与 Bearer 头等价。
    #[tokio::test]
    async fn websocket_query_token_is_accepted() {
        prime_cache();
        let user = token("user");
        assert_eq!(
            status("/appstore/ws/abc", None).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status(&format!("/appstore/ws/abc?token={user}"), None).await,
            StatusCode::OK
        );
        assert_eq!(
            status("/appstore/ws/abc?token=bad", None).await,
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn public_and_personal_paths() {
        assert_eq!(required_for("/health"), Required::Public);
        assert_eq!(required_for("/auth/login"), Required::Public);
        assert_eq!(required_for("/auth/logout"), Required::User);
        assert_eq!(required_for("/auth/totp/setup"), Required::User);
        assert_eq!(required_for("/user/info"), Required::User);
        assert_eq!(required_for("/user/notices/read"), Required::User);
    }

    #[test]
    fn system_paths_default_to_admin() {
        for p in [
            "/system/config/time",
            "/system/config/time/sync",
            "/system/config/time/timezone",
            "/system/config/network/hostname",
            "/system/config/network/resolver",
            "/system/config/ssh/restart",
            "/system/config/ssh/install",
            "/system/config/services",
            "/system/config/services/action",
            "/system/config/processes",
            "/system/config/processes/kill",
            "/system/config/basic",
            "/system/config/zap",
            "/system/config/firewall/rule/add",
            "/system/job/start",
            "/system/job/stop",
            "/system/status",
            "/system/overview",
            "/system/env",
            "/system/cron/add",
            "/system/nginx/config/save",
            "/system/service-conf/save",
            "/system/ip/add",
            "/system/role/add",
            "/system/audit/list",
            "/system/update/apply",
            "/system/migrate/home",
            "/system/menus/add",
        ] {
            assert_eq!(required_for(p), Required::Admin, "{p} 必须为 admin 专属");
        }
    }

    #[test]
    fn explicitly_relaxed_paths() {
        assert_eq!(required_for("/system/info"), Required::User);
        assert_eq!(required_for("/system/files/list"), Required::User);
        assert_eq!(required_for("/system/menus/tree"), Required::User);
        assert_eq!(required_for("/system/user/list"), Required::Reseller);
        assert_eq!(required_for("/system/package/add"), Required::Reseller);
        assert_eq!(required_for("/system/fpm-specs/list"), Required::Reseller);
        // 更具体的条目覆盖较短前缀
        assert_eq!(required_for("/system/user/resellers"), Required::Admin);
        assert_eq!(required_for("/system/user/home_sync"), Required::Admin);
        assert_eq!(required_for("/system/fpm-specs/add"), Required::Admin);
    }

    #[test]
    fn business_paths() {
        assert_eq!(required_for("/site/list"), Required::User);
        assert_eq!(required_for("/site/users"), Required::Reseller);
        assert_eq!(required_for("/site/sync_all"), Required::Reseller);
        assert_eq!(required_for("/ssl/cert/list"), Required::User);
        assert_eq!(required_for("/terminal/ws/1"), Required::User);
        assert_eq!(required_for("/appstore/packages"), Required::User);
        assert_eq!(required_for("/appstore/repos/add"), Required::Admin);
        assert_eq!(required_for("/appstore/script/run"), Required::Admin);
        assert_eq!(required_for("/appstore/scripts/tree"), Required::Admin);
        assert_eq!(required_for("/dev/api-token/create"), Required::Admin);
    }

    #[test]
    fn perm_key_splits_view_and_edit() {
        assert_eq!(
            perm_key_for("/site/list", &Method::GET).as_deref(),
            Some("site:view")
        );
        assert_eq!(
            perm_key_for("/site/add", &Method::POST).as_deref(),
            Some("site:edit")
        );
        assert_eq!(
            perm_key_for("/site/delete", &Method::DELETE).as_deref(),
            Some("site:edit")
        );
        // 个人接口不设权限点
        assert_eq!(perm_key_for("/user/info", &Method::GET), None);
        assert_eq!(perm_key_for("/health", &Method::GET), None);
    }

    #[test]
    fn seed_permissions_match_role_floor() {
        let admin = default_permissions_for("admin");
        let reseller = default_permissions_for("reseller");
        let user = default_permissions_for("user");
        let demo = default_permissions_for("demo");

        // admin 拿到全部权限点
        assert!(admin.contains(&"system.config:edit".to_string()));
        assert_eq!(admin.len(), all_perm_keys().len());

        // 普通用户：站点/文件/商店可用，服务器配置不可用
        assert!(user.contains(&"site:view".to_string()));
        assert!(user.contains(&"system.file:view".to_string()));
        assert!(!user.contains(&"system.config:view".to_string()));
        assert!(!user.contains(&"system.user:view".to_string()));
        assert_eq!(user, demo);

        // reseller：在普通用户之上追加用户/套餐管理
        assert!(reseller.contains(&"site:view".to_string()));
        assert!(reseller.contains(&"system.user:view".to_string()));
        assert!(reseller.contains(&"system.package:edit".to_string()));
        assert!(!reseller.contains(&"system.config:view".to_string()));
    }

    #[test]
    fn catalog_covers_all_namespaces() {
        let groups = permission_catalog();
        assert!(
            groups
                .iter()
                .any(|g| g.ns == "site" && g.label == "站点管理")
        );
        assert!(groups.iter().any(|g| g.ns == "system.config"));
        // 每个模块含 view / edit 两个动作
        for g in &groups {
            assert_eq!(g.actions.len(), 2);
            assert!(g.actions.iter().any(|a| a.key.ends_with(":view")));
            assert!(g.actions.iter().any(|a| a.key.ends_with(":edit")));
        }
    }

    #[test]
    fn unknown_paths_are_admin_only() {
        // 新增接口未登记时的兜底行为：默认拒绝
        assert_eq!(required_for("/some/new/endpoint"), Required::Admin);
    }

    #[test]
    fn normalize_strips_prefix() {
        assert_eq!(normalize("/api/health"), "/health");
        assert_eq!(normalize("/api/system/config/time"), "/system/config/time");
        assert_eq!(normalize("/system/config/time"), "/system/config/time");
        assert_eq!(normalize("/api"), "/");
    }
}
