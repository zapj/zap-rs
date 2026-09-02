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
