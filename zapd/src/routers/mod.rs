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
pub mod system_role;
pub mod user;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
struct Assets;

static INDEX_HTML: &str = "index.html";

async fn index_html() -> Response {
    match Assets::get(INDEX_HTML) {
        Some(content) => {
            let body = Body::from(content.data);
            Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .body(body)
                .unwrap()
        }
        None => not_found().await,
    }
}

async fn not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404"))
        .unwrap()
}

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

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

pub fn routers() -> Router {
    Router::new()
        .fallback(static_handler)
        .nest("/api", api_routers())
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
        // User management (admin + reseller)
        .route("/system/user/list", get(user::user_list))
        .route("/system/user/add", post(user::user_add))
        .route("/system/user/update", post(user::user_update))
        .route("/system/user/delete", post(user::user_delete))
        .route("/system/user/resellers", get(user::reseller_list))
        .route("/system/user/home_sync", post(user::user_home_sync))
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
        .route("/terminal/ws/{id}", get(ssh_terminal::ws_terminal))
        // System
        .route("/system/info", get(system_info::system_info))
        .route("/system/status", get(system_info::system_status))
        .route("/system/overview", get(system_info::system_overview))
        .route("/system/job/stop", get(system_job::stop_job))
        .route("/system/job/start", get(system_job::start_job))
        // Audit logs (admin only)
        .route("/system/audit/list", get(system_audit::audit_list))
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
