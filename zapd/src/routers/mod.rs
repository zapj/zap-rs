use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use rust_embed::RustEmbed;
use serde_json::json;

pub mod appstore;
pub mod auth;
pub mod ssh_keys;
pub mod ssh_terminal;
pub mod system_audit;
pub mod system_config;
pub mod system_info;
pub mod system_file;
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
        // Role management (admin only)
        .route("/system/role/list", get(system_role::role_list))
        .route("/system/role/add", post(system_role::role_add))
        .route("/system/role/update", post(system_role::role_update))
        .route("/system/role/delete", post(system_role::role_delete))
        .route("/system/role/permissions", get(system_role::role_permissions_get))
        .route("/system/role/permissions/set", post(system_role::role_permissions_set))
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
        .route("/system/config/time/timezone", post(system_config::set_timezone))
        .route("/system/config/time/timezones", get(system_config::list_timezones))
        .route("/system/config/ssh/status", get(system_config::ssh_status))
        .route("/system/config/ssh/restart", post(system_config::ssh_restart))
        // SSH key management (admin only)
        .route("/system/config/ssh/keys", get(ssh_keys::list_keys))
        .route("/system/config/ssh/keys/content", get(ssh_keys::get_key_content))
        .route("/system/config/ssh/keys/generate", post(ssh_keys::generate_key))
        .route("/system/config/ssh/keys/import", post(ssh_keys::import_key))
        .route("/system/config/ssh/keys/delete", post(ssh_keys::delete_key))
        .route("/system/config/ssh/authorized_keys", get(ssh_keys::list_authorized_keys))
        .route("/system/config/ssh/authorize", post(ssh_keys::authorize_key))
        .route("/system/config/ssh/deauthorize", post(ssh_keys::deauthorize_key))
        // SSH terminal
        .route("/terminal/connections", get(ssh_terminal::list_connections))
        .route("/terminal/connections/{id}", get(ssh_terminal::get_connection))
        .route("/terminal/connections/create", post(ssh_terminal::create_connection))
        .route("/terminal/connections/{id}/update", post(ssh_terminal::update_connection))
        .route("/terminal/connections/{id}/delete", post(ssh_terminal::delete_connection))
        .route("/terminal/connections/test", get(ssh_terminal::test_connection))
        .route("/terminal/ws/{id}", get(ssh_terminal::ws_terminal))
        // System
        .route("/system/info", get(system_info::system_info))
        .route("/system/status", get(system_info::system_status))
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
        // AppStore
        .route("/appstore/repo/info", get(appstore::repo_info))
        .route("/appstore/repo/update", post(appstore::repo_update))
        .route("/appstore/packages", get(appstore::packages))
        .route("/appstore/install", post(appstore::install))
        .route("/appstore/uninstall", post(appstore::uninstall))
        .route("/appstore/upgrade", post(appstore::upgrade))
        .route("/appstore/scripts/tree", get(appstore::scripts_tree))
        .route("/appstore/script/read", get(appstore::script_read))
        .route("/appstore/script/write", post(appstore::script_write))
        .route("/appstore/script/run", post(appstore::script_run))
        .route("/appstore/script/stop", post(appstore::script_stop))
        .route("/appstore/runs", get(appstore::runs))
        .route("/appstore/log/{run_id}", get(appstore::log))
        .route("/appstore/ws/{run_id}", get(appstore::ws_log))
}