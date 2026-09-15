//! 用户资源用量采集（磁盘 / 带宽）。
//!
//! - **磁盘**：定时对 `user.home_dir` 执行 `du -sb`，写回
//!   `user.disk_used_bytes` / `disk_stat_at`；
//! - **带宽**：增量解析站点 `access.log`（nginx main / combined 的
//!   `$body_bytes_sent`），按站点记 `site.traffic_*`，再按归属用户汇总
//!   写入 `user.bandwidth_used_bytes`（按月，`bandwidth_period` 为 YYYYMM）。
//!
//! 日志路径取 `site.log_root/access.log`（不依赖目录命名）。

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use chrono::Local;
use tokio::process::Command;
use tracing::{debug, warn};

use crate::db::get_db_pool;

/// 统计周期（YYYYMM）：跨月自动重置本月计数
fn period_now() -> String {
    Local::now().format("%Y%m").to_string()
}

/// 从 access.log 单行取 `$body_bytes_sent`（nginx main / combined）。
///
/// 取请求行结束引号后的第二个字段：`<status> <bytes> "<referer>" ...`，
/// 可容忍 referer / UA 中的空格；`-` 与解析失败按 0 计。
fn parse_body_bytes(line: &str) -> u64 {
    let Some(start) = line.find('"') else {
        return 0;
    };
    let rest = &line[start + 1..];
    let Some(end) = rest.find('"') else {
        return 0;
    };
    let mut it = rest[end + 1..].split_whitespace();
    let _status = it.next();
    match it.next() {
        Some(v) => v.parse::<u64>().unwrap_or(0),
        None => 0,
    }
}

/// 目录字节数：`du -sb` 优先，不支持 `-b` 时回退 `du -sk` × 1024
async fn du_bytes(dir: &str) -> Option<u64> {
    async fn run(args: [&str; 2]) -> Option<String> {
        let out = Command::new("du").args(args).output().await.ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    }

    if let Some(s) = run(["-sb", dir]).await
        && let Ok(v) = s.split_whitespace().next().unwrap_or("").parse::<u64>()
    {
        return Some(v);
    }
    let s = run(["-sk", dir]).await?;
    let kb: u64 = s.split_whitespace().next()?.parse().ok()?;
    Some(kb.saturating_mul(1024))
}

/// 采集全部用户的家目录磁盘用量
pub async fn collect_disk_usage() {
    let pool = get_db_pool().await;
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, home_dir FROM user WHERE home_dir <> ''")
            .fetch_all(pool)
            .await
            .unwrap_or_default();
    let now = Local::now().timestamp();

    for (id, home_dir) in rows {
        if !Path::new(&home_dir).exists() {
            continue;
        }
        match du_bytes(&home_dir).await {
            Some(bytes) => {
                let _ = sqlx::query(
                    "UPDATE user SET disk_used_bytes = ?, disk_stat_at = ? WHERE id = ?",
                )
                .bind(bytes as i64)
                .bind(now)
                .bind(id)
                .execute(pool)
                .await;
            }
            None => warn!("磁盘用量采集失败: user={} home={}", id, home_dir),
        }
    }
}

/// 从 `offset` 起累加日志中的响应字节数（同步 IO，调用方包 spawn_blocking）
fn sum_bytes(path: &Path, offset: u64) -> std::io::Result<u64> {
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut total = 0u64;
    let mut reader = BufReader::new(f);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        total = total.saturating_add(parse_body_bytes(line.trim_end()));
    }
    Ok(total)
}

/// 采集站点流量并汇总到用户（本月出站字节数）
pub async fn collect_bandwidth() {
    let pool = get_db_pool().await;
    let period = period_now();
    let rows: Vec<(i64, i64, String, i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, log_root, traffic_offset, traffic_inode, traffic_month, \
                traffic_month_bytes, traffic_total_bytes \
         FROM site WHERE log_root <> ''",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let now = Local::now().timestamp();

    for (id, _user_id, log_root, offset, inode, month, month_bytes, total) in rows {
        let path: PathBuf = Path::new(&log_root).join("access.log");
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let cur_inode = meta.ino().to_string();
        let size = meta.len();
        // 日志轮转 / 重建：inode 变化或文件变小 → 从头重新统计
        let start = if cur_inode != inode || size < offset as u64 {
            0
        } else {
            offset as u64
        };
        if size <= start {
            continue;
        }

        let p = path.clone();
        let delta = match tokio::task::spawn_blocking(move || sum_bytes(&p, start)).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                warn!("流量日志解析失败: site={} err={}", id, e);
                0
            }
            Err(_) => 0,
        };
        if delta == 0 {
            debug!("站点 {} 本轮无新增流量", id);
        }

        let new_month_bytes = if month == period {
            month_bytes.saturating_add(delta as i64)
        } else {
            delta as i64
        };
        let _ = sqlx::query(
            "UPDATE site SET traffic_offset = ?, traffic_inode = ?, traffic_total_bytes = ?, \
             traffic_month_bytes = ?, traffic_month = ?, traffic_stat_at = ? WHERE id = ?",
        )
        .bind(size as i64)
        .bind(&cur_inode)
        .bind(total.saturating_add(delta as i64))
        .bind(new_month_bytes)
        .bind(&period)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await;
    }

    // 按归属用户汇总本月流量（无站点的用户归零）
    let _ = sqlx::query(
        "UPDATE user SET bandwidth_used_bytes = COALESCE( \
            (SELECT SUM(s.traffic_month_bytes) FROM site s \
              WHERE s.user_id = user.id AND s.traffic_month = ?), 0), \
         bandwidth_period = ?, bandwidth_stat_at = ?",
    )
    .bind(&period)
    .bind(&period)
    .bind(now)
    .execute(pool)
    .await;
}

/// 一次跑完磁盘 + 带宽（供定时任务调用）
pub async fn collect_all() {
    collect_disk_usage().await;
    collect_bandwidth().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_body_bytes() {
        let line = r#"1.2.3.4 - - [15/Sep/2026:10:00:00 +0800] "GET / HTTP/1.1" 200 1234 "http://ref" "Mozilla/5.0 (X11)""#;
        assert_eq!(parse_body_bytes(line), 1234);

        // referer / UA 含空格不影响取值
        let line2 = r#"1.2.3.4 - alice [15/Sep/2026:10:00:00 +0800] "POST /a?b=c HTTP/1.0" 404 56 "-" "curl 8.0 x""#;
        assert_eq!(parse_body_bytes(line2), 56);

        // 无字节位（dash）与异常行
        assert_eq!(parse_body_bytes(r#"- - - [x] "GET / HTTP/1.1" 200 -"#), 0);
        assert_eq!(parse_body_bytes("garbage"), 0);
    }
}
