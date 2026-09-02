use serde_json::json;

use crate::verbs::root_cmd;
use zap_proto::Response;

const ALLOWED_ACTIONS: &[&str] = &["start", "stop", "restart", "reload", "enable", "disable"];

/// 列出系统所有服务（systemd）
pub async fn list() -> Response {
    tokio::task::spawn_blocking(|| {
        let out = root_cmd("systemctl")
            .args([
                "list-units",
                "--type=service",
                "--all",
                "--no-pager",
                "--no-legend",
                "--plain",
            ])
            .output();

        match out {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                let services: Vec<_> = text
                    .lines()
                    .filter_map(|line| {
                        let line = line.trim();
                        if line.is_empty() {
                            return None;
                        }
                        let mut it = line.split_whitespace();
                        let name = it.next()?.to_string();
                        if !name.ends_with(".service") {
                            return None;
                        }
                        let load = it.next().unwrap_or("").to_string();
                        let active = it.next().unwrap_or("").to_string();
                        let sub = it.next().unwrap_or("").to_string();
                        let rest: Vec<&str> = it.collect();
                        let description = rest.join(" ");
                        Some(json!({
                            "name": name,
                            "load": load,
                            "active": active,
                            "sub": sub,
                            "description": description,
                        }))
                    })
                    .collect();
                Response::ok("ok", Some(json!({ "services": services })))
            }
            Ok(_) => Response::err(
                -1,
                format!(
                    "获取服务列表失败: {}",
                    String::from_utf8_lossy(&out.unwrap().stderr)
                ),
            ),
            Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

/// 对服务执行 start/stop/restart/reload/enable/disable
pub async fn action(name: &str, action: &str) -> Response {
    if !ALLOWED_ACTIONS.contains(&action) {
        return Response::err(-1, format!("不支持的操作: {action}"));
    }
    let name = name.to_string();
    let action = action.to_string();
    tokio::task::spawn_blocking(move || {
        let out = root_cmd("systemctl").args([&action, &name]).output();
        match out {
            Ok(o) if o.status.success() => {
                // 读取操作后的实际状态
                let status = root_cmd("systemctl")
                    .args(["is-active", &name])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                Response::ok(
                    "ok",
                    Some(json!({ "name": name, "action": action, "status": status })),
                )
            }
            Ok(o) => Response::err(
                -1,
                format!("操作失败: {}", String::from_utf8_lossy(&o.stderr).trim()),
            ),
            Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}
