use std::net::SocketAddr;

use axum::{
    Json,
    extract::{Extension, Path, Query},
};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::zap::appstore as ast;
use crate::zap::{ZapError, ZapJsonResult, audit, jwt::ValidatedClaims};
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

// ── System Services ─────────────────────────────────────────

/// Get list of system services (systemd)
pub async fn list_services(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::ServiceList).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

#[derive(Debug, Deserialize)]
pub struct ServiceActionPayload {
    pub name: String,
    pub action: String,
}

/// start / stop / restart / reload / enable / disable a service
pub async fn service_action(
    _claims: ValidatedClaims,
    Json(payload): Json<ServiceActionPayload>,
) -> ZapJsonResult {
    if payload.name.is_empty() {
        return Err(ZapError::New(-1, "服务名称不能为空".to_string()));
    }
    let resp = crate::zapexec::call(Request::ServiceAction {
        name: payload.name,
        action: payload.action,
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(
        json!({ "code": 0, "message": resp.message, "data": resp.data }),
    ))
}

// ── Process Management ─────────────────────────────────────

/// 获取运行中的进程列表
pub async fn list_processes(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::ProcessList).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

#[derive(Debug, Deserialize)]
pub struct ProcessKillPayload {
    pub pid: u32,
    #[serde(default)]
    pub signal: Option<String>,
}

/// 终止进程（缺省 TERM，signal=9 为 KILL）
pub async fn process_kill(
    _claims: ValidatedClaims,
    Json(payload): Json<ProcessKillPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::ProcessKill {
        pid: payload.pid,
        signal: payload.signal,
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(
        json!({ "code": 0, "message": resp.message, "data": resp.data }),
    ))
}

// ── SSH Install ──────────────────────────────────────────────

/// 安装 openssh-server（后台异步，日志写入 run 记录，供前端轮询）
pub async fn ssh_install(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
) -> ZapJsonResult {
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(
        &run_id,
        "ssh_install",
        "openssh-server",
        &claims.sub,
        &log_path,
    )
    .await?;

    let resp = crate::zapexec::call(Request::SshInstall {
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path.clone());
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssh_install",
        &claims.sub,
        "openssh-server",
    )
    .await;
    info!("SSH install started: {run_id}");
    Ok(Json(json!({
        "code": 0,
        "message": "安装已启动",
        "data": { "run_id": run_id, "log": log_path }
    })))
}

#[derive(Debug, Deserialize)]
pub struct SshInstallLogQuery {
    pub offset: Option<u64>,
}

/// 轮询安装日志
pub async fn ssh_install_log(
    _claims: ValidatedClaims,
    Path(run_id): Path<String>,
    Query(q): Query<SshInstallLogQuery>,
) -> ZapJsonResult {
    let run = ast::get_run(&run_id)
        .await?
        .ok_or_else(|| ZapError::New(-1, "任务不存在".to_string()))?;
    let (content, exit_code, done) = ast::read_log(&run.log_path, q.offset.unwrap_or(0)).await?;
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": { "content": content, "exit_code": exit_code, "done": done, "status": run.status }
    })))
}
