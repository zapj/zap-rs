use axum::{
    body::Body, http::{header, StatusCode, Uri}, response::Response, routing::{post, Route}
};
use axum::Router;
use axum::routing::get;
use rust_embed::RustEmbed;

mod auth;

#[derive(RustEmbed)]
#[folder = "../adminui/dist/"]
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
    .nest("/api",  api_routers())
    // .route("/ws", get(handler))
    // .route("/", get(index))
}

fn api_routers() -> Router {
    Router::new().route("/auth/login", post(auth::login) )
}
