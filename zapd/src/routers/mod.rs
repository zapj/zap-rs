use axum::{
    Json, Router,
    body::Body,
    extract::Request,
    http::{Method, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rust_embed::RustEmbed;
use serde_json::json;

use crate::zap::jwt::{claims_from_token, is_demo};

/// 演示账号只读守卫：demo 角色仅允许 GET 请求（浏览），其余写操作一律拒绝。
/// /auth/* 为个人账户操作（登录/登出/改密/2FA），放行以免演示账号被锁死。
#[allow(clippy::result_large_err)] // axum 中间件约定 Result<Response, Response>
async fn demo_readonly_guard(req: Request, next: Next) -> Result<Response, Response> {
    if req.method() == Method::GET || req.uri().path().starts_with("/auth/") {
        return Ok(next.run(req).await);
    }
    // 先拷贝 token 再异步解析（避免借用 req 跨 await）
    let bearer = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(String::from);
    if let Some(token) = bearer
        && let Some(claims) = claims_from_token(&token).await
        && is_demo(&claims)
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "code": -1, "message": "演示账号仅支持浏览，不能执行操作" })),
        )
            .into_response());
    }
    Ok(next.run(req).await)
}

pub mod appstore;
pub mod auth;
pub mod dev;
pub mod fpm_spec;
pub mod notice;
pub mod package;
pub mod site;
pub mod ssh_keys;
pub mod ssh_terminal;
pub mod ssl;
pub mod system_audit;
pub mod system_basic;
pub mod system_config;
pub mod system_cron;
pub mod system_env;
pub mod system_file;
pub mod system_info;
pub mod system_ip;
pub mod system_job;
pub mod system_menu;
pub mod system_migrate;
pub mod system_role;
pub mod system_update;
pub mod user;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
struct Assets;

static INDEX_HTML: &str = "index.html";

async fn index_html() -> Response {
    match Assets::get(INDEX_HTML) {
        Some(content) => {
            let html = String::from_utf8_lossy(&content.data).into_owned();
            let html = inject_base_tag(&html, &crate::config::url_prefix_path());
            Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .body(Body::from(html))
                .unwrap()
        }
        None => not_found().await,
    }
}

/// 向 SPA 首页注入 `<base>` 与 URL 前缀。
///
/// 前端构建产物用相对路径引用资源（vite `base: './'`），因此**必须**注入 `<base>`：
/// 否则在多级路由（如 `/system/users`、`/zap/system/users`）直接刷新时，
/// `./assets/xxx.js` 会被解析成 `/system/assets/xxx.js` 而 404，页面一片空白。
///
/// - 未启用前缀：注入 `<base href="/">`
/// - 启用前缀：注入 `<base href="/zap/">`
///
/// 同时注入 `window.__ZAP_BASE__`（无前缀时为空串），供前端 axios baseURL、
/// vue-router base 与 WebSocket 使用。
fn inject_base_tag(html: &str, prefix_path: &str) -> String {
    let base = if prefix_path.is_empty() {
        "/".to_string()
    } else {
        format!("{prefix_path}/")
    };
    let inject = format!(
        "\n    <base href=\"{base}\">\n    <script>window.__ZAP_BASE__=\"{prefix_path}\"</script>"
    );
    match html.find("<head>") {
        Some(pos) => {
            let at = pos + "<head>".len();
            let (head, tail) = html.split_at(at);
            format!("{head}{inject}{tail}")
        }
        None => format!("{inject}{html}"),
    }
}

async fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404"))
        .unwrap()
}

async fn static_handler(uri: Uri) -> Response {
    let mut full = uri.path().to_string();
    // `Router::nest` 会剥离前缀，这里再幂等剥一次：
    // 保证无论拿到的是完整路径还是剥离后的路径都能命中文件。
    let prefix = crate::config::url_prefix_path();
    if !prefix.is_empty() {
        if full == prefix {
            full = "/".to_string();
        } else if let Some(rest) = full.strip_prefix(&format!("{prefix}/")) {
            full = format!("/{rest}");
        }
    }
    let path = full.trim_start_matches('/');

    if path.is_empty() || path == INDEX_HTML {
        return index_html().await;
    }

    match Assets::get(path) {
        Some(content) => {
            let body = Body::from(content.data);
            let mime = mime_guess::from_path(path).first_or_octet_stream();

            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(body)
                .unwrap()
        }
        None => {
            if path.contains('.') {
                return not_found().await;
            }

            index_html().await
        }
    }
}

/// Health check endpoint — no auth required
async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Local::now().timestamp(),
    }))
}

/// 组装完整路由：前缀取自 `zap.yaml` 的 `server.url_prefix`。
pub fn routers() -> Router {
    build_routers(&crate::config::url_prefix())
}

/// 按给定前缀组装路由（`prefix` 为空表示不启用前缀）。
///
/// 配置 `server.url_prefix` 后，页面与接口全部挂到 `/{prefix}` 下：
/// - 页面：`/zap/dashboard`
/// - 接口：`/zap/api/auth/login`
///
/// 前缀之外的路径（`/`、`/api/*` 等）一律返回 **404**，不做重定向：
/// 这样外部探测根路径时无法发现真实入口，起到隐藏后台入口的作用。
///
/// 未配置前缀时行为与之前完全一致（`/api/*` + 根路径 SPA）。
fn build_routers(prefix: &str) -> Router {
    let inner = Router::new()
        .fallback(static_handler)
        .nest("/api", api_routers());

    let prefix = prefix.trim().trim_matches('/');
    if prefix.is_empty() {
        return inner;
    }

    let nested = format!("/{prefix}");
    // nest 的 catch-all 能匹配 /zap 与 /zap/xxx，但匹配不到 /zap/（尾斜杠），
    // 补一条显式路由，保证 /zap 与 /zap/ 两种写法都能打开首页。
    let nested_slash = format!("{nested}/");
    Router::new()
        .nest(&nested, inner)
        .route(&nested_slash, get(index_html))
        // 前缀之外的任何路径都 404（不回跳，避免暴露前缀）
        .fallback(not_found)
}

fn api_routers() -> Router {
    Router::new()
        // Health
        .route("/health", get(health_check))
        // Auth
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", get(auth::logout))
        .route("/auth/reflash_token", post(auth::reflash_token))
        .route("/auth/change_password", post(auth::change_password))
        // TOTP 2FA
        .route("/auth/totp/setup", get(auth::totp_setup))
        .route("/auth/totp/verify", post(auth::totp_verify))
        .route("/auth/totp/disable", post(auth::totp_disable))
        .route("/auth/totp/status", get(auth::totp_status))
        .route("/user/info", get(user::user_info))
        .route(
            "/user/prefs",
            get(user::user_prefs_get).post(user::user_prefs_save),
        )
        // 站内信（通知中心，登录用户本人）
        .route("/user/notices", get(notice::notices_list))
        .route("/user/notices/unread", get(notice::notices_unread))
        .route("/user/notices/read", post(notice::notices_read))
        .route("/user/notices/read_all", post(notice::notices_read_all))
        .route("/user/notices/delete", post(notice::notices_delete))
        // User management (admin + reseller)
        .route("/system/user/list", get(user::user_list))
        .route("/system/user/add", post(user::user_add))
        .route("/system/user/update", post(user::user_update))
        .route("/system/user/delete", post(user::user_delete))
        .route("/system/user/resellers", get(user::reseller_list))
        .route("/system/user/home_sync", post(user::user_home_sync))
        // 套餐（Packages）：创建客户时选择的资源套餐
        .route("/system/package/list", get(package::package_list))
        .route("/system/package/add", post(package::package_add))
        .route("/system/package/update", post(package::package_update))
        .route("/system/package/delete", post(package::package_delete))
        // Role management (admin only)
        .route("/system/role/list", get(system_role::role_list))
        .route("/system/role/add", post(system_role::role_add))
        .route("/system/role/update", post(system_role::role_update))
        .route("/system/role/delete", post(system_role::role_delete))
        .route(
            "/system/role/permissions",
            get(system_role::role_permissions_get),
        )
        .route(
            "/system/role/permissions/set",
            post(system_role::role_permissions_set),
        )
        // Menu management (admin only)
        .route("/system/menus/tree", get(system_menu::get_menus_tree))
        .route("/system/menus/list", get(system_menu::menu_list))
        .route("/system/menus/add", post(system_menu::menu_add))
        .route("/system/menus/update", post(system_menu::menu_update))
        .route("/system/menus/delete", post(system_menu::menu_delete))
        .route("/system/menus/status", post(system_menu::menu_status))
        // Server config (admin only)
        .route("/system/config/time", get(system_config::get_time))
        .route("/system/config/time/sync", post(system_config::sync_time))
        .route(
            "/system/config/time/timezone",
            post(system_config::set_timezone),
        )
        .route(
            "/system/config/time/timezones",
            get(system_config::list_timezones),
        )
        .route("/system/config/network", get(system_config::network_get))
        .route(
            "/system/config/network/hostname",
            post(system_config::network_set_hostname),
        )
        .route(
            "/system/config/network/resolver",
            post(system_config::network_set_resolver),
        )
        // ── IP 池管理 ────────────────────────────────
        .route("/system/ip/list", get(system_ip::ip_list))
        .route("/system/ip/add", post(system_ip::ip_add))
        .route("/system/ip/delete", post(system_ip::ip_delete))
        .route("/system/ip/update", post(system_ip::ip_update))
        .route(
            "/system/ip/batch-reserved",
            post(system_ip::ip_batch_reserved),
        )
        .route("/system/config/ssh/status", get(system_config::ssh_status))
        .route(
            "/system/config/ssh/restart",
            post(system_config::ssh_restart),
        )
        .route(
            "/system/config/ssh/install",
            post(system_config::ssh_install),
        )
        .route(
            "/system/config/ssh/install/log/{run_id}",
            get(system_config::ssh_install_log),
        )
        // 数据迁移（服务器配置 → 数据迁移，admin only）
        .route(
            "/system/migrate/users",
            get(system_migrate::migrate_users_preview),
        )
        .route(
            "/system/migrate/home",
            post(system_migrate::migrate_home_mv),
        )
        // 基础设置（系统设置 → 基础设置，admin only）
        .route(
            "/system/config/basic",
            get(system_basic::basic_get).post(system_basic::basic_save),
        )
        .route("/system/config/services", get(system_config::list_services))
        .route(
            "/system/config/services/action",
            post(system_config::service_action),
        )
        .route(
            "/system/config/processes",
            get(system_config::list_processes),
        )
        .route(
            "/system/config/processes/kill",
            post(system_config::process_kill),
        )
        // Server runtime env (运行环境状态表，admin only)
        .route("/system/env", get(system_env::env_get))
        .route("/system/env/refresh", post(system_env::env_refresh))
        .route("/system/env/defaults", post(system_env::env_defaults_save))
        // 脚本/自动化：计划任务（admin only）
        .route("/system/cron/list", get(system_cron::cron_list))
        .route("/system/cron/add", post(system_cron::cron_add))
        .route("/system/cron/update", post(system_cron::cron_update))
        .route("/system/cron/delete", post(system_cron::cron_delete))
        .route("/system/cron/toggle", post(system_cron::cron_toggle))
        .route("/system/cron/run_now", post(system_cron::cron_run_now))
        // PHP-FPM 规格模板库（admin 维护；reseller 可读自己名下 + 全局模板）
        .route("/system/fpm-specs/list", get(fpm_spec::spec_list))
        .route("/system/fpm-specs/add", post(fpm_spec::spec_add))
        .route("/system/fpm-specs/update", post(fpm_spec::spec_update))
        .route("/system/fpm-specs/delete", post(fpm_spec::spec_delete))
        // SSH key management (admin only)
        .route("/system/config/ssh/keys", get(ssh_keys::list_keys))
        .route(
            "/system/config/ssh/keys/content",
            get(ssh_keys::get_key_content),
        )
        .route(
            "/system/config/ssh/keys/generate",
            post(ssh_keys::generate_key),
        )
        .route("/system/config/ssh/keys/import", post(ssh_keys::import_key))
        .route("/system/config/ssh/keys/delete", post(ssh_keys::delete_key))
        .route(
            "/system/config/ssh/authorized_keys",
            get(ssh_keys::list_authorized_keys),
        )
        .route(
            "/system/config/ssh/authorize",
            post(ssh_keys::authorize_key),
        )
        .route(
            "/system/config/ssh/deauthorize",
            post(ssh_keys::deauthorize_key),
        )
        // SSH terminal
        .route("/terminal/connections", get(ssh_terminal::list_connections))
        .route(
            "/terminal/connections/{id}",
            get(ssh_terminal::get_connection),
        )
        .route(
            "/terminal/connections/create",
            post(ssh_terminal::create_connection),
        )
        .route(
            "/terminal/connections/{id}/update",
            post(ssh_terminal::update_connection),
        )
        .route(
            "/terminal/connections/{id}/delete",
            post(ssh_terminal::delete_connection),
        )
        .route(
            "/terminal/connections/test",
            get(ssh_terminal::test_connection),
        )
        .route(
            "/terminal/connections/{id}/push-key",
            post(ssh_terminal::push_key_to_host),
        )
        .route("/terminal/push-key", post(ssh_terminal::push_key_direct))
        .route("/terminal/ws/{id}", get(ssh_terminal::ws_terminal))
        // System
        .route("/system/info", get(system_info::system_info))
        .route("/system/status", get(system_info::system_status))
        .route("/system/overview", get(system_info::system_overview))
        .route("/system/job/stop", get(system_job::stop_job))
        .route("/system/job/start", get(system_job::start_job))
        // Audit logs (admin only)
        .route("/system/audit/list", get(system_audit::audit_list))
        // System update (系统设置 → 系统更新, admin only)
        .route("/system/update/status", get(system_update::status_get))
        .route("/system/update/config", post(system_update::config_save))
        .route("/system/update/check", post(system_update::check))
        .route("/system/update/apply", post(system_update::apply))
        .route("/system/update/log/{run_id}", get(system_update::log))
        // File manager
        .route("/system/files/list", get(system_file::file_list))
        .route("/system/files/read", get(system_file::file_read))
        .route("/system/files/write", post(system_file::file_write))
        .route("/system/files/delete", post(system_file::file_delete))
        .route("/system/files/mkdir", post(system_file::file_mkdir))
        .route("/system/files/rename", post(system_file::file_rename))
        .route("/system/files/download", get(system_file::file_download))
        .route("/system/files/upload", post(system_file::file_upload))
        .route("/system/files/info", get(system_file::file_info))
        // AppStore（多 Git 源）
        .route("/appstore/repos", get(appstore::list_repos))
        .route("/appstore/repos/add", post(appstore::repo_add))
        .route("/appstore/repos/remove", post(appstore::repo_remove))
        .route("/appstore/repos/update", post(appstore::repo_update))
        .route("/appstore/packages", get(appstore::packages))
        .route("/appstore/install", post(appstore::install))
        .route("/appstore/uninstall", post(appstore::uninstall))
        .route("/appstore/upgrade", post(appstore::upgrade))
        .route("/appstore/installed", get(appstore::installed_apps))
        .route("/appstore/instance/action", post(appstore::instance_action))
        .route("/appstore/scripts/tree", get(appstore::scripts_tree))
        .route("/appstore/script/read", get(appstore::script_read))
        .route("/appstore/script/write", post(appstore::script_write))
        .route("/appstore/script/run", post(appstore::script_run))
        .route("/appstore/script/stop", post(appstore::script_stop))
        .route("/appstore/run/files", get(appstore::run_files))
        .route("/appstore/run/file/read", get(appstore::run_file_read))
        .route("/appstore/run/file/write", post(appstore::run_file_write))
        .route("/appstore/run/retry", post(appstore::run_retry))
        .route("/appstore/runs", get(appstore::runs))
        .route("/appstore/log/{run_id}", get(appstore::log))
        .route("/appstore/ws/{run_id}", get(appstore::ws_log))
        // 站点管理（admin 全部 / reseller 所属客户 / user 自己的站点）
        .route("/site/list", get(site::site_list))
        .route("/site/users", get(site::site_users))
        .route("/site/add", post(site::site_add))
        .route("/site/update", post(site::site_update))
        .route("/site/delete", post(site::site_delete))
        .route("/site/sync", post(site::site_sync))
        .route("/site/sync_all", post(site::site_sync_all))
        // SSL/TLS：证书管理（手动导入 / 自签名 / Let's Encrypt）
        .route("/ssl/cert/list", get(ssl::cert_list))
        .route("/ssl/cert/detail", get(ssl::cert_detail))
        .route("/ssl/cert/parse", post(ssl::cert_parse))
        .route("/ssl/cert/add", post(ssl::cert_add))
        .route("/ssl/cert/update", post(ssl::cert_update))
        .route("/ssl/cert/delete", post(ssl::cert_delete))
        .route("/ssl/cert/self-sign", post(ssl::cert_self_sign))
        .route("/ssl/cert/letsencrypt", post(ssl::cert_letsencrypt))
        // 开发：API Token 管理 + API 文档
        .route("/dev/api-token/list", get(dev::api_token_list))
        .route("/dev/api-token/create", post(dev::api_token_create))
        .route("/dev/api-token/update", post(dev::api_token_update))
        .route("/dev/api-token/delete", post(dev::api_token_delete))
        .route("/dev/api-docs", get(dev::api_docs))
        .layer(middleware::from_fn(demo_readonly_guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use tower::ServiceExt; // oneshot

    async fn status_of(app: Router, uri: &str) -> StatusCode {
        get(app, uri).await.0
    }

    /// 返回（状态码，响应体文本），用于区分"命中接口"还是"落到 SPA fallback"
    async fn get(app: Router, uri: &str) -> (StatusCode, String) {
        let req = axum::http::Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn no_prefix_keeps_legacy_paths() {
        let app = build_routers("");
        // 健康检查仍在 /api 下
        let (s, body) = get(app.clone(), "/api/health").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("\"status\":\"ok\""), "应命中 health 接口");
        // 未启用前缀时 /zap/... 走 SPA fallback，不会命中接口
        let (_s, body) = get(app, "/zap/api/health").await;
        assert!(!body.contains("\"status\":\"ok\""), "前缀路径不应命中接口");
    }

    #[tokio::test]
    async fn prefix_moves_api_and_pages() {
        let app = build_routers("zap");
        // 接口搬到前缀下
        let (s, body) = get(app.clone(), "/zap/api/health").await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains("\"status\":\"ok\""), "应命中 health 接口");
        // 旧路径不再可访问：一律 404（不重定向，避免暴露前缀）
        assert_eq!(
            status_of(app.clone(), "/api/health").await,
            StatusCode::NOT_FOUND
        );
        // 根路径同样 404
        assert_eq!(status_of(app.clone(), "/").await, StatusCode::NOT_FOUND);
        // 相近但不同的前缀也不应命中
        assert_eq!(
            status_of(app.clone(), "/zap2/api/health").await,
            StatusCode::NOT_FOUND
        );
        // 前缀根路径（含尾斜杠）应能打开首页
        assert_eq!(status_of(app.clone(), "/zap").await, StatusCode::OK);
        assert_eq!(status_of(app, "/zap/").await, StatusCode::OK);
    }

    #[tokio::test]
    async fn prefix_is_normalized() {
        // 首尾斜杠与多级前缀都能正常工作
        assert_eq!(
            status_of(build_routers("/zap/"), "/zap/api/health").await,
            StatusCode::OK
        );
        assert_eq!(
            status_of(build_routers("a/b"), "/a/b/api/health").await,
            StatusCode::OK
        );
    }

    #[test]
    fn base_tag_injection() {
        let html = "<html><head><title>x</title></head><body></body></html>";

        // 有前缀：注入 /zap/ 与全局变量
        let out = inject_base_tag(html, "/zap");
        assert!(out.contains(r#"<base href="/zap/">"#));
        assert!(out.contains(r#"window.__ZAP_BASE__="/zap""#));

        // 无前缀：也要注入 <base href="/">，否则多级路由刷新时资源路径会错
        let out = inject_base_tag(html, "");
        assert!(out.contains(r#"<base href="/">"#));
        assert!(out.contains(r#"window.__ZAP_BASE__="""#));

        // base 必须在页面资源引用之前（紧跟 <head>）
        let out = inject_base_tag(html, "/zap");
        let base_pos = out.find("<base").unwrap();
        let title_pos = out.find("<title>").unwrap();
        assert!(base_pos < title_pos, "base 标签必须在页面资源之前");
    }

    #[test]
    fn normalize_url_prefix_rules() {
        use crate::config::normalize_url_prefix;
        assert_eq!(normalize_url_prefix(""), "");
        assert_eq!(normalize_url_prefix("   "), "");
        assert_eq!(normalize_url_prefix("zap"), "zap");
        assert_eq!(normalize_url_prefix("/zap/"), "zap");
        assert_eq!(normalize_url_prefix("  /zap/  "), "zap");
        assert_eq!(normalize_url_prefix("a/b"), "a/b");
    }
}
