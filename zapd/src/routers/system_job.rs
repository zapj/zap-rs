use axum::Json;
use serde_json::json;

use crate::zap::{self, jwt::ValidatedClaims, ZapJsonResult};

pub async fn stop_job(_: ValidatedClaims) -> ZapJsonResult {
    zap::job::stop_system_job().await;
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
    })))
}

pub async fn start_job(_: ValidatedClaims) -> ZapJsonResult {
    zap::job::start_system_job().await;
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
    })))
}
