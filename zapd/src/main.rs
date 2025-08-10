use std::collections::HashMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use anyhow::Ok;
use axum::extract::State;
use axum::Extension;
use axum::{
    http::StatusCode,
    response::{IntoResponse},
    routing::{get,post},
    Router,
};
use sqlx::{Executor, SqlitePool};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::time::sleep;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod db;
mod config;
// use db::prepare_database;

struct AppState {
    jwt: HashMap<String, String>
}


#[tokio::main]
async fn main()-> anyhow::Result<()> {
    println!("{:?}",config::new());
    // Enable tracing.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    // let pool = db::prepare_database().await?;
    let conn = db::prepare_database().await.unwrap();
    // Create a regular axum app.
    let app = Router::new()
        .route("/slow", get(|| sleep(Duration::from_secs(5))))
        .route("/forever", get(std::future::pending::<()>))
        .route("/hello",get(handler))
        .layer((
            TraceLayer::new_for_http(),
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            TimeoutLayer::new(Duration::from_secs(10)),
            Extension(conn),
        ));
    let app = app.fallback(handler_404);
    // Create a `TcpListener` using tokio.
    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    // Run the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
    Ok(())
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 page")
}

async fn handler(Extension(pool): Extension<SqlitePool>) {
    // state.lock().unwrap().jwt.insert("k".to_string(), "v".to_string());
    let _ = pool.execute("create table user (id int,name text)").await;
}


async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}