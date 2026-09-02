use serde_json::json;

use crate::verbs::root_cmd;
use zap_proto::Response;

pub async fn sync() -> Response {
    tokio::task::spawn_blocking(|| {
        let chrony_ok = root_cmd("chronyc")
            .args(["-a", "makestep"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if chrony_ok {
            return Response::ok("已通过 chrony 同步时间", None);
        }

        match root_cmd("ntpdate").args(["pool.ntp.org"]).output() {
            Ok(o) if o.status.success() => Response::ok("已通过 ntpdate 同步时间", None),
            Ok(o) => Response::err(
                -1,
                format!("ntpdate 失败: {}", String::from_utf8_lossy(&o.stderr)),
            ),
            Err(e) => Response::err(-1, format!("未找到时间同步工具: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn set_timezone(timezone: &str) -> Response {
    if timezone.is_empty() {
        return Response::err(-1, "时区不能为空");
    }
    let tz = timezone.to_string();
    tokio::task::spawn_blocking(move || {
        match root_cmd("timedatectl").args(["set-timezone", &tz]).output() {
            Ok(o) if o.status.success() => Response::ok("时区设置成功", None),
            Ok(o) => Response::err(
                -1,
                format!("时区设置失败: {}", String::from_utf8_lossy(&o.stderr)),
            ),
            Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn list_timezones() -> Response {
    tokio::task::spawn_blocking(|| {
        match root_cmd("timedatectl").args(["list-timezones"]).output() {
            Ok(o) if o.status.success() => {
                let zones: Vec<String> = String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Response::ok("ok", Some(json!(zones)))
            }
            Ok(o) => Response::err(-1, String::from_utf8_lossy(&o.stderr).to_string()),
            Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn get() -> Response {
    tokio::task::spawn_blocking(|| {
        let now = chrono::Local::now();
        let tz = current_timezone();
        Response::ok(
            "ok",
            Some(json!({
                "datetime": now.format("%Y-%m-%d %H:%M:%S").to_string(),
                "timestamp": now.timestamp(),
                "timezone": tz,
                "timezone_offset": now.offset().to_string(),
            })),
        )
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

fn current_timezone() -> String {
    if let Ok(tz) = std::fs::read_to_string("/etc/timezone") {
        return tz.trim().to_string();
    }
    if let Ok(o) = root_cmd("timedatectl")
        .args(["show", "--property=Timezone", "--value"])
        .output()
        && o.status.success()
    {
        return String::from_utf8_lossy(&o.stdout).trim().to_string();
    }
    "Unknown".to_string()
}
