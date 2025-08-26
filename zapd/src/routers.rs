use axum::Router;
use axum::routing::get;

pub fn api_auth_routers () -> Router {
    Router::new().route("/ws", get(handler))
}


async fn handler() -> &'static str {
    "Hello, World!"
}