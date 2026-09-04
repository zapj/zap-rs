//! 脚本/自动化 → 计划任务（admin only）：cron_jobs CRUD / 启停 / 立即运行。

use std::net::SocketAddr;

use axum::{Json, extract::Extension};
use serde::Deserialize;
use serde_json::json;

use crate::zap::{
    ZapError, ZapJsonResult, audit,
    jwt::{ValidatedClaims, is_admin},
    script_cron::{self, Cron},
};

fn ensure_admin(claims: &ValidatedClaims) -> Result<(), ZapError> {
    if is_admin(claims) {
        Ok(())
    } else {
        Err(ZapError::New(-1, "权限不足，仅管理员可使用".to_string()))
    }
}

/// 校验脚本路径：位于 custom/scripts/ 下且无目录穿越。
fn check_script_path(path: &str) -> Result<(), ZapError> {
    let p = path.trim();
    if !p.starts_with("scripts/") {
        return Err(ZapError::New(-1, "只允许选择 scripts/ 下的脚本".to_string()));
    }
    if p.contains("..") || p.starts_with('/') {
        return Err(ZapError::New(-1, "脚本路径不合法".to_string()));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CronJobPayload {
    pub name: String,
    pub script_path: String,
    pub schedule: String,
    #[serde(default)]
    pub remark: String,
}

#[derive(Debug, Deserialize)]
pub struct CronJobUpdatePayload {
    pub id: i64,
    pub name: String,
    pub script_path: String,
    pub schedule: String,
    #[serde(default)]
    pub remark: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct CronIdPayload {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CronTogglePayload {
    pub id: i64,
    pub enabled: bool,
}

/// GET /system/cron/list
pub async fn cron_list(claims: ValidatedClaims) -> ZapJsonResult {
    ensure_admin(&claims)?;
    let mut jobs = script_cron::list_jobs().await?;
    for job in &mut jobs {
        script_cron::refresh_next_run(job).await;
    }
    Ok(Json(json!({ "code": 0, "data": { "jobs": jobs } })))
}

/// POST /system/cron/add
pub async fn cron_add(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CronJobPayload>,
) -> ZapJsonResult {
    ensure_admin(&claims)?;
    let name = payload.name.trim().to_string();
    let path = payload.script_path.trim().to_string();
    let schedule = payload.schedule.trim().to_string();
    if name.is_empty() {
        return Err(ZapError::New(-1, "任务名称不能为空".to_string()));
    }
    check_script_path(&path)?;
    Cron::parse(&schedule)
        .map_err(|e| ZapError::New(-1, format!("cron 表达式错误：{e}")))?;
    let id = script_cron::insert_job(&name, &path, &schedule, payload.remark.trim()).await?;
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "cron_add",
        &name,
        &path,
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "计划任务已创建", "data": { "id": id } })))
}

/// POST /system/cron/update
pub async fn cron_update(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CronJobUpdatePayload>,
) -> ZapJsonResult {
    ensure_admin(&claims)?;
    let name = payload.name.trim().to_string();
    let path = payload.script_path.trim().to_string();
    let schedule = payload.schedule.trim().to_string();
    if name.is_empty() {
        return Err(ZapError::New(-1, "任务名称不能为空".to_string()));
    }
    check_script_path(&path)?;
    Cron::parse(&schedule)
        .map_err(|e| ZapError::New(-1, format!("cron 表达式错误：{e}")))?;
    if script_cron::get_job(payload.id).await?.is_none() {
        return Err(ZapError::New(-1, "计划任务不存在".to_string()));
    }
    script_cron::update_job(payload.id, &name, &path, &schedule, payload.remark.trim(), payload.enabled)
        .await?;
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "cron_update",
        &name,
        &path,
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "已保存" })))
}

/// POST /system/cron/delete
pub async fn cron_delete(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CronIdPayload>,
) -> ZapJsonResult {
    ensure_admin(&claims)?;
    if let Some(job) = script_cron::get_job(payload.id).await? {
        script_cron::delete_job(payload.id).await?;
        audit::log(
            Some(&claims),
            Some(client_addr.ip().to_string().as_str()),
            "cron_delete",
            &job.name,
            &job.script_path,
        )
        .await;
    }
    Ok(Json(json!({ "code": 0, "message": "已删除" })))
}

/// POST /system/cron/toggle
pub async fn cron_toggle(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CronTogglePayload>,
) -> ZapJsonResult {
    ensure_admin(&claims)?;
    script_cron::set_job_enabled(payload.id, payload.enabled).await?;
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        if payload.enabled { "cron_enable" } else { "cron_disable" },
        &payload.id.to_string(),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "已更新" })))
}

/// POST /system/cron/run_now —— 立即运行一次
pub async fn cron_run_now(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CronIdPayload>,
) -> ZapJsonResult {
    ensure_admin(&claims)?;
    let job = script_cron::get_job(payload.id)
        .await?
        .ok_or_else(|| ZapError::New(-1, "计划任务不存在".to_string()))?;
    let run_id = script_cron::launch_script_run(&job.script_path, "manual").await?;
    // 立即运行同样刷新最近运行记录
    let now = chrono::Local::now().timestamp();
    let _ = sqlx::query(
        "UPDATE cron_jobs SET last_run_at = ?, last_run_id = ?, updated_at = strftime('%s','now') WHERE id = ?",
    )
    .bind(now)
    .bind(&run_id)
    .bind(job.id)
    .execute(crate::db::get_db_pool().await)
    .await;
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "cron_run_now",
        &job.name,
        &job.script_path,
    )
    .await;
    Ok(Json(json!({
        "code": 0,
        "message": "已触发运行",
        "data": { "run_id": run_id }
    })))
}
