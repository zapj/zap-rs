use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::process::Command;
use tracing::info;

use crate::zap::{jwt::ValidatedClaims, ZapError, ZapJsonResult};

// ── Time ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SetTimezonePayload {
    pub timezone: String,
}

/// Get server time info
pub async fn get_time(_claims: ValidatedClaims) -> ZapJsonResult {
    let now = chrono::Local::now();
    let tz = get_current_timezone();

    Ok(Json(json!({
        "code": 0,
        "data": {
            "datetime": now.format("%Y-%m-%d %H:%M:%S").to_string(),
            "timestamp": now.timestamp(),
            "timezone": tz,
            "timezone_offset": now.offset().to_string(),
        }
    })))
}

/// Sync time via NTP
pub async fn sync_time(_claims: ValidatedClaims) -> ZapJsonResult {
    // Try chrony first, then ntpdate
    let (msg, ok) = if Command::new("chronyc")
        .args(["-a", "makestep"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        ("已通过 chrony 同步时间".to_string(), true)
    } else {
        let output = Command::new("ntpdate")
            .args(["pool.ntp.org"])
            .output();

        match output {
            Ok(o) if o.status.success() => ("已通过 ntpdate 同步时间".to_string(), true),
            Ok(o) => (
                format!("ntpdate 失败: {}", String::from_utf8_lossy(&o.stderr)),
                false,
            ),
            Err(e) => (format!("未找到时间同步工具: {}", e), false),
        }
    };

    if ok {
        info!("Time synced via NTP");
        Ok(Json(json!({ "code": 0, "message": msg })))
    } else {
        Err(ZapError::New(-1, msg))
    }
}

/// Set system timezone
pub async fn set_timezone(
    _claims: ValidatedClaims,
    Json(payload): Json<SetTimezonePayload>,
) -> ZapJsonResult {
    if payload.timezone.is_empty() {
        return Err(ZapError::New(-1, "时区不能为空".to_string()));
    }

    let output = Command::new("timedatectl")
        .args(["set-timezone", &payload.timezone])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            info!("Timezone set to {}", payload.timezone);
            Ok(Json(json!({ "code": 0, "message": "时区设置成功" })))
        }
        Ok(o) => Err(ZapError::New(
            -1,
            format!("时区设置失败: {}", String::from_utf8_lossy(&o.stderr)),
        )),
        Err(e) => Err(ZapError::New(-1, format!("命令执行失败: {}", e))),
    }
}

/// Get list of available timezones
pub async fn list_timezones(_claims: ValidatedClaims) -> ZapJsonResult {
    let output = Command::new("timedatectl")
        .args(["list-timezones"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let zones: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(Json(json!({ "code": 0, "data": zones })))
        }
        Ok(o) => Err(ZapError::New(
            -1,
            format!("{}", String::from_utf8_lossy(&o.stderr)),
        )),
        Err(e) => Err(ZapError::New(-1, format!("命令执行失败: {}", e))),
    }
}

fn get_current_timezone() -> String {
    // Try reading /etc/timezone or use timedatectl
    if let Ok(tz) = std::fs::read_to_string("/etc/timezone") {
        return tz.trim().to_string();
    }
    if let Ok(output) = Command::new("timedatectl")
        .args(["show", "--property=Timezone", "--value"])
        .output()
    {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    "Unknown".to_string()
}

// ── SSH ────────────────────────────────────────────────────

/// Get SSH server status
pub async fn ssh_status(_claims: ValidatedClaims) -> ZapJsonResult {
    let (running, port, version) = get_ssh_info();

    Ok(Json(json!({
        "code": 0,
        "data": {
            "running": running,
            "port": port,
            "version": version,
        }
    })))
}

/// Restart SSH server
pub async fn ssh_restart(_claims: ValidatedClaims) -> ZapJsonResult {
    let output = Command::new("systemctl")
        .args(["restart", "sshd"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            info!("SSH server restarted");
            Ok(Json(json!({ "code": 0, "message": "SSH 服务已重启" })))
        }
        Ok(_o) => {
            // Try ssh instead of sshd (Debian/Ubuntu naming)
            let output2 = Command::new("systemctl")
                .args(["restart", "ssh"])
                .output();

            match output2 {
                Ok(o2) if o2.status.success() => {
                    info!("SSH server restarted (ssh service)");
                    Ok(Json(json!({ "code": 0, "message": "SSH 服务已重启" })))
                }
                Ok(o2) => Err(ZapError::New(
                    -1,
                    format!("SSH 重启失败: {}", String::from_utf8_lossy(&o2.stderr)),
                )),
                Err(e) => Err(ZapError::New(-1, format!("命令执行失败: {}", e))),
            }
        }
        Err(e) => Err(ZapError::New(-1, format!("命令执行失败: {}", e))),
    }
}

fn get_ssh_info() -> (bool, u16, String) {
    // Check if sshd is running
    let running = Command::new("systemctl")
        .args(["is-active", "sshd"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or_else(|_| {
            Command::new("systemctl")
                .args(["is-active", "ssh"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false)
        });

    // Get SSH port from config
    let port = std::fs::read_to_string("/etc/ssh/sshd_config")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.trim_start().starts_with("Port "))
                .and_then(|line| line.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(22);

    // Get SSH version
    let version = Command::new("sshd")
        .arg("-V")
        .output()
        .map(|o| {
            // sshd -V writes to stderr
            let s = String::from_utf8_lossy(&o.stderr);
            s.lines().next().unwrap_or("Unknown").to_string()
        })
        .unwrap_or_else(|_| "Unknown".to_string());

    (running, port, version)
}
