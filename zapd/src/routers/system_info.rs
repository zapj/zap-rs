use axum::Json;
use serde_json::json;

use crate::zap::{self, ZapJsonResult, jwt::ValidatedClaims};

pub async fn system_info(_: ValidatedClaims) -> ZapJsonResult {
    zap::system_info::get_system_info().await
}

pub async fn system_status(_: ValidatedClaims) -> ZapJsonResult {
    zap::system_info::get_system_status().await
}

pub async fn system_overview(_: ValidatedClaims) -> ZapJsonResult {
    zap::system_info::get_system_overview().await
}

/// 面板自身信息（About Zap）：版本、构建日期、开源协议等。
/// 静态信息，无需频繁刷新。
pub async fn about(_: ValidatedClaims) -> ZapJsonResult {
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": {
            "version": env!("CARGO_PKG_VERSION"),
            "build_date": option_env!("BUILD_DATE").unwrap_or("unknown"),
            "license": "GPL-3.0-or-later",
            "docs_path": "/docs/manual",
            "api_docs_path": "/dev/api-docs",
        }
    })))
}
