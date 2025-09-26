use axum::{
    body::Body, http::{header, StatusCode, Uri}, response::Response, routing::post
};
use axum::Router;
use axum::routing::get;
use rust_embed::RustEmbed;


pub mod auth;

pub mod user;
pub mod system_info;
pub mod system_menu;
pub mod system_job;

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

pub fn routers () -> Router {
    Router::new()
    .fallback(static_handler)
    // 动态生成 prefix + /api
    .nest("/api",  api_routers())
}

fn api_routers() -> Router {
    
    Router::new().route("/auth/login", post(auth::login) )
        .route("/user/info", get(user::user_info))
        .route("/system/info", get(system_info::system_info))
        .route("/system/status", get(system_info::system_status))
        .route("/system/job/stop", get(system_job::stop_job))
        .route("/system/job/start", get(system_job::start_job))
        .route("/system/menus/tree",get(system_menu::get_menus_tree))
}
