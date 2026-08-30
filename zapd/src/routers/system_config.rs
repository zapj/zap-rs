use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::zap::{jwt::ValidatedClaims, ZapError, ZapJsonResult};
use zap_proto::Request;

// ── Time ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetTimezonePayload {
    pub timezone: String,
}

/// Get server time info
pub async fn get_time(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::TimeGet).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

/// Sync time via NTP
pub async fn sync_time(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::TimeSync).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

/// Set system timezone
pub async fn set_timezone(
    _claims: ValidatedClaims,
    Json(payload): Json<SetTimezonePayload>,
) -> ZapJsonResult {
    if payload.timezone.is_empty() {
        return Err(ZapError::New(-1, "时区不能为空".to_string()));
    }

    let resp = crate::zapexec::call(Request::TimeSetTimezone {
        timezone: payload.timezone,
    })
    .await?;

    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

/// Get list of available timezones
pub async fn list_timezones(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::TimeListTimezones).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

// ── SSH ────────────────────────────────────────────────────

/// Get SSH server status
pub async fn ssh_status(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshStatus).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

/// Restart SSH server
pub async fn ssh_restart(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshRestart).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}
