//! 脚本/自动化：计划任务（cron_jobs）。
//!
//! - cron 表达式为 5 段：`分 时 日 月 周`（标准 crontab 语法）
//! - 调度器每分钟扫描一次 enabled 任务，命中即触发脚本运行。
//!   执行链路复用 AppStore 脚本运行：`appstore_runs` 记录 + 日志监控
//!   （脚本以 root 运行于 zapexec，日志见运行记录 / 实时日志）。

use chrono::{Datelike, TimeZone, Timelike};
use serde::Serialize;
use sqlx::FromRow;
use tokio::time::Duration;
use tracing::{info, warn};

use crate::zap::appstore as ast;
use crate::zap::ZapError;
use crate::zapexec;
use zap_proto::Request;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CronJob {
    pub id: i64,
    pub name: String,
    pub script_path: String,
    pub schedule: String,
    pub remark: String,
    pub enabled: i64,
    pub last_run_at: i64,
    pub last_run_id: String,
    pub next_run_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Cron 表达式（5 段）──────────────────────────────────────

pub struct Cron {
    minutes: [bool; 60],
    hours: [bool; 24],
    doms: [bool; 32],  // 1..=31
    months: [bool; 13], // 1..=12
    dows: [bool; 8],   // 0..=6（解析期允许 7，合并到 0）
    dom_all: bool,
    dow_all: bool,
}

impl Cron {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err("cron 表达式需为 5 段：分 时 日 月 周（如 */5 * * * *）".to_string());
        }
        let mut minutes = [false; 60];
        let mut hours = [false; 24];
        let mut doms = [false; 32];
        let mut months = [false; 13];
        let mut dows = [false; 8];

        parse_field(parts[0], 0, 59, &mut minutes, false)?;
        parse_field(parts[1], 0, 23, &mut hours, false)?;
        parse_field(parts[2], 1, 31, &mut doms, false)?;
        parse_field(parts[3], 1, 12, &mut months, false)?;
        parse_field(parts[4], 0, 7, &mut dows, true)?;
        // 周 7 == 周日 0
        dows[0] |= dows[7];
        dows[7] = false;

        let dom_all = parts[2] == "*";
        let dow_all = parts[4] == "*";
        Ok(Self {
            minutes,
            hours,
            doms,
            months,
            dows,
            dom_all,
            dow_all,
        })
    }

    /// 判断当前时间是否命中（日与周同时限制时按 crontab 语义取“或”）。
    pub fn matches(&self, dt: &chrono::DateTime<chrono::Local>) -> bool {
        if !self.minutes[dt.minute() as usize] {
            return false;
        }
        if !self.hours[dt.hour() as usize] {
            return false;
        }
        if !self.months[dt.month() as usize] {
            return false;
        }
        let dom = dt.day() as usize;
        let dow = dt.weekday().num_days_from_sunday() as usize;
        match (self.dom_all, self.dow_all) {
            (true, true) => true,
            (true, false) => self.dows[dow],
            (false, true) => self.doms[dom],
            (false, false) => self.doms[dom] || self.dows[dow],
        }
    }

    /// 计算 `now` 之后（严格大于当前分钟）的下一次命中 Unix 秒；一年内找不到返回 0。
    pub fn next_run_ts_after(&self, now: &chrono::DateTime<chrono::Local>) -> i64 {
        let base = now
            .with_second(0)
            .map(|t| t.timestamp())
            .unwrap_or_else(|| now.timestamp());
        let cap = base + 366 * 24 * 3600;
        let mut cand = base + 60;
        while cand <= cap {
            if let Some(dt) = chrono::Local.timestamp_opt(cand, 0).single()
                && self.matches(&dt)
            {
                return cand;
            }
            cand += 60;
        }
        0
    }
}

/// 解析单个 cron 字段（支持 `*`、`*/n`、`a-b`、`a-b/n`、`n`、逗号列表）。
fn parse_field(s: &str, min: usize, max: usize, out: &mut [bool], sunday_seven: bool) -> Result<(), String> {
    for item in s.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (range_part, step) = match item.split_once('/') {
            Some((r, st)) => {
                let st: usize = st
                    .trim()
                    .parse()
                    .map_err(|_| format!("cron 步进非法: {item}"))?;
                if st == 0 {
                    return Err(format!("cron 步进不能为 0: {item}"));
                }
                (r, st)
            }
            None => (item, 1),
        };
        let (lo, hi) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            let a: usize = a.trim().parse().map_err(|_| format!("cron 数值非法: {item}"))?;
            let b: usize = b.trim().parse().map_err(|_| format!("cron 数值非法: {item}"))?;
            (a, b)
        } else {
            let v: usize = range_part
                .trim()
                .parse()
                .map_err(|_| format!("cron 数值非法: {item}"))?;
            (v, v)
        };
        if lo > hi || lo < min || hi > max {
            return Err(format!("cron 数值超出范围[{min}-{max}]: {item}"));
        }
        let mut v = lo;
        while v <= hi {
            let idx = if sunday_seven { v % 8 } else { v };
            out[idx] = true;
            v += step;
        }
    }
    Ok(())
}

// ── 数据库操作 ──────────────────────────────────────────────

pub async fn list_jobs() -> Result<Vec<CronJob>, sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    sqlx::query_as::<_, CronJob>("SELECT * FROM cron_jobs ORDER BY id")
        .fetch_all(pool)
        .await
}

pub async fn get_job(id: i64) -> Result<Option<CronJob>, sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    sqlx::query_as::<_, CronJob>("SELECT * FROM cron_jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn insert_job(name: &str, script_path: &str, schedule: &str, remark: &str) -> Result<i64, sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    sqlx::query(
        r#"INSERT INTO cron_jobs (name, script_path, schedule, remark, enabled, created_at, updated_at)
           VALUES (?, ?, ?, ?, 1, ?, ?)"#,
    )
    .bind(name)
    .bind(script_path)
    .bind(schedule)
    .bind(remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
        .fetch_one(pool)
        .await
        .unwrap_or(0))
}

pub async fn update_job(
    id: i64,
    name: &str,
    script_path: &str,
    schedule: &str,
    remark: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    sqlx::query(
        r#"UPDATE cron_jobs
           SET name = ?, script_path = ?, schedule = ?, remark = ?, enabled = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(name)
    .bind(script_path)
    .bind(schedule)
    .bind(remark)
    .bind(enabled as i64)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_job(id: i64) -> Result<(), sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    sqlx::query("DELETE FROM cron_jobs WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_job_enabled(id: i64, enabled: bool) -> Result<(), sqlx::Error> {
    let pool = crate::db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    sqlx::query("UPDATE cron_jobs SET enabled = ?, updated_at = ? WHERE id = ?")
        .bind(enabled as i64)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn mark_last_run(id: i64, run_id: &str) {
    let pool = crate::db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();
    let _ = sqlx::query("UPDATE cron_jobs SET last_run_at = ?, last_run_id = ?, updated_at = ? WHERE id = ?")
        .bind(now)
        .bind(run_id)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await;
}

async fn mark_next_run(id: i64, next: i64) {
    let pool = crate::db::get_db_pool().await;
    let _ = sqlx::query("UPDATE cron_jobs SET next_run_at = ?, updated_at = strftime('%s','now') WHERE id = ?")
        .bind(next)
        .bind(id)
        .execute(pool)
        .await;
}

/// 计算任务的“下次运行时间”（若已过期或为 0 则重新计算并回写），用于列表展示。
pub async fn refresh_next_run(job: &mut CronJob) {
    if job.enabled == 0 {
        return;
    }
    let now = chrono::Local::now();
    if job.next_run_at > now.timestamp() {
        return;
    }
    let expr = match Cron::parse(&job.schedule) {
        Ok(e) => e,
        Err(_) => return,
    };
    let next = expr.next_run_ts_after(&now);
    job.next_run_at = next;
    if next > 0 {
        mark_next_run(job.id, next).await;
    }
}

// ── 执行与调度 ──────────────────────────────────────────────

/// 执行一次脚本运行（与「自定义脚本 → 运行」同一链路）。
/// `action` 用于运行记录区分（cron / manual）。
pub async fn launch_script_run(path: &str, action: &str) -> Result<String, ZapError> {
    let run_id = ast::generate_run_id();
    let log_path = ast::log_path_for(&run_id);
    ast::register_run(&run_id, action, path, "admin", &log_path).await?;
    let resp = zapexec::call(Request::AppstoreScriptRun {
        path: path.to_string(),
        run_id: run_id.clone(),
    })
    .await?;
    if resp.code != 0 {
        ast::finish_run(&run_id, "failed", resp.code as i64).await;
        return Err(ZapError::New(resp.code, resp.message));
    }
    ast::watch_log(run_id.clone(), log_path);
    Ok(run_id)
}

/// 启动计划任务调度器（后台每分钟评估一次）。
pub fn start() {
    tokio::spawn(async move {
        info!("脚本计划任务调度器已启动");
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        let mut last_minute: i64 = -1;
        loop {
            interval.tick().await;
            let now = chrono::Local::now();
            let minute_key = now.timestamp() / 60;
            if minute_key == last_minute {
                continue;
            }
            last_minute = minute_key;
            tick_once(&now).await;
        }
    });
}

async fn tick_once(now: &chrono::DateTime<chrono::Local>) {
    let jobs = match list_jobs().await {
        Ok(j) => j,
        Err(e) => {
            warn!("读取计划任务失败: {e}");
            return;
        }
    };
    let ts = now.timestamp();
    for job in jobs {
        if job.enabled == 0 {
            continue;
        }
        let expr = match Cron::parse(&job.schedule) {
            Ok(e) => e,
            Err(e) => {
                warn!("计划任务 #{} 表达式非法({}): {}", job.id, job.schedule, e);
                continue;
            }
        };
        // 展示用 next_run_at 已过期：先推进（无论是否命中本次）
        if job.next_run_at <= ts {
            let next = expr.next_run_ts_after(now);
            if next > 0 {
                mark_next_run(job.id, next).await;
            }
        }
        // 命中：防重（上一次运行须在 50s 前，避免同一分钟内重复触发）
        if expr.matches(now) && job.last_run_at < ts - 50 {
            let job_id = job.id;
            let path = job.script_path.clone();
            let run_id = ast::generate_run_id();
            mark_last_run(job_id, &run_id).await;
            tokio::spawn(async move {
                match launch_script_run(&path, "cron").await {
                    Ok(rid) => info!("计划任务 #{job_id} 已触发: {path} ({rid})"),
                    Err(e) => warn!("计划任务 #{job_id} 触发失败: {path}: {e}"),
                }
            });
        }
    }
}
