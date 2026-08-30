use serde_json::json;

use crate::verbs::root_cmd;
use zap_proto::Response;

pub async fn status() -> Response {
    tokio::task::spawn_blocking(|| {
        let running = root_cmd("systemctl")
            .args(["is-active", "sshd"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or_else(|_| {
                root_cmd("systemctl")
                    .args(["is-active", "ssh"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                    .unwrap_or(false)
            });

        let port = std::fs::read_to_string("/etc/ssh/sshd_config")
            .ok()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.trim_start().starts_with("Port "))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
            })
            .unwrap_or(22u16);

        let version = root_cmd("sshd")
            .arg("-V")
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stderr)
                    .lines()
                    .next()
                    .unwrap_or("Unknown")
                    .to_string()
            })
            .unwrap_or_else(|_| "Unknown".to_string());

        Response::ok(
            "ok",
            Some(json!({ "running": running, "port": port, "version": version })),
        )
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn restart() -> Response {
    tokio::task::spawn_blocking(|| {
        let out = root_cmd("systemctl").args(["restart", "sshd"]).output();
        match out {
            Ok(o) if o.status.success() => Response::ok("SSH 服务已重启", None),
            Ok(_) => {
                let out2 = root_cmd("systemctl").args(["restart", "ssh"]).output();
                match out2 {
                    Ok(o2) if o2.status.success() => Response::ok("SSH 服务已重启", None),
                    Ok(o2) => Response::err(
                        -1,
                        format!("SSH 重启失败: {}", String::from_utf8_lossy(&o2.stderr)),
                    ),
                    Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
                }
            }
            Err(e) => Response::err(-1, format!("命令执行失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}
