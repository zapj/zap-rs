use axum::{Extension, Json};
use serde_json::json;
use sqlx::{Pool, Sqlite};

use crate::zap::{self, jwt, ZapJsonResult};

pub async fn stop_job(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> ZapJsonResult {
    zap::job::stop_system_job().await;
    Ok(Json(json!({
        "code":0,
        "message":"OK",
    })))
}

pub async fn start_job(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> ZapJsonResult {
    zap::job::start_system_job().await;
    Ok(Json(json!({
        "code":0,
        "message":"OK",
    })))
}