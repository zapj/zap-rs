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
//! 匹配方式：先剥掉 URL 前缀与 `/api`，再取**最长前缀**命中项。

use axum::{
    Json,
    extract::Request,
    http::{StatusCode, header},
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

/// 权限矩阵：`(路径前缀, 所需角色)`。
///
/// 未列出的路径一律按 `Admin` 处理，典型如：
/// `/system/config/*`、`/system/nginx/*`、`/system/service-conf/*`、`/system/ip/*`、
/// `/system/env*`、`/system/cron/*`、`/system/role/*`、`/system/audit/*`、
/// `/system/update/*`、`/system/migrate/*`、`/system/job/*`、`/system/status`、
/// `/system/overview`、`/system/menus/*`（`/tree` 除外）、`/dev/*`。
const RULES: &[(&str, Required)] = &[
    // ── 免鉴权 ───────────────────────────────────────────────
    ("/health", Required::Public),
    ("/auth/login", Required::Public),
    // ── 登录态即可访问的个人接口 ─────────────────────────────
    ("/auth", Required::User),
    ("/user", Required::User),
    // 菜单树：所有角色渲染侧边栏都要读
    ("/system/menus/tree", Required::User),
    // 文件管理：handler 内已把非管理员限制在自己的 home 与私有 tmp
    ("/system/files", Required::User),
    // 仪表盘：普通用户首页（cpanel 视图）会读取 /system/info
    ("/system/info", Required::User),
    // ── reseller 可管理名下客户 ──────────────────────────────
    ("/system/user", Required::Reseller),
    ("/system/user/resellers", Required::Admin),
    ("/system/user/home_sync", Required::Admin),
    ("/system/package", Required::Reseller),
    ("/system/fpm-specs/list", Required::Reseller),
    ("/site/users", Required::Reseller),
    ("/site/sync_all", Required::Reseller),
    // ── 业务对象：handler 按 owner / reseller 归属收敛可见范围 ─
    ("/site", Required::User),
    ("/ssl", Required::User),
    ("/terminal", Required::User),
    // ── 应用商店：源码与脚本管理仅管理员，其余按 handler 内规则 ─
    ("/appstore/repos/add", Required::Admin),
    ("/appstore/repos/remove", Required::Admin),
    ("/appstore/repos/update", Required::Admin),
    ("/appstore/script", Required::Admin),
    ("/appstore/scripts/tree", Required::Admin),
    ("/appstore", Required::User),
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

/// 查询路径所需角色：最长前缀命中；未命中 → `Admin`（默认拒绝）。
fn required_for(path: &str) -> Required {
    let mut best: Option<(usize, Required)> = None;
    for (prefix, req) in RULES {
        if !prefix_hit(path, prefix) {
            continue;
        }
        if best.is_none_or(|(len, _)| prefix.len() > len) {
            best = Some((prefix.len(), *req));
        }
    }
    best.map(|(_, r)| r).unwrap_or(Required::Admin)
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

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, middleware, routing::get};
    use tower::ServiceExt; // oneshot

    fn app() -> Router {
        Router::new()
            .route("/health", get(|| async { "ok" }))
            .route("/system/config/time", get(|| async { "ok" }))
            .route("/system/user/list", get(|| async { "ok" }))
            .route("/appstore/ws/{run_id}", get(|| async { "ok" }))
            .layer(middleware::from_fn(guard))
    }

    async fn status(uri: &str, token: Option<&str>) -> StatusCode {
        let mut req = axum::http::Request::builder().uri(uri);
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

    #[tokio::test]
    async fn middleware_enforces_matrix() {
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
