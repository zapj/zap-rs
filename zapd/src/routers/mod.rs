use axum::Router;
use axum::routing::get;

pub fn api_auth_routers () -> Router {
    Router::new().route("/ws", get(handler))
    .route("/", get(index))
}


async fn handler() -> &'static str {
    "Hello, World!"
}

async fn index() -> String {
    "hi index".to_string()
}