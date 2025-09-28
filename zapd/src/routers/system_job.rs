use axum::Json;
use serde_json::json;

use crate::zap::{self, jwt, ZapJsonResult};

pub async fn stop_job(_:jwt::Claims) -> ZapJsonResult {
    zap::job::stop_system_job().await;
    Ok(Json(json!({
        "code":0,
        "message":"OK",
    })))
}

pub async fn start_job(_:jwt::Claims) -> ZapJsonResult {
    zap::job::start_system_job().await;
    Ok(Json(json!({
        "code":0,
        "message":"OK",
    })))
}