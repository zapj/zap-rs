//! 面板用户家目录骨架初始化（root 执行）。
//!
//! 契约：每个面板用户在 `user.home_dir`（通常为 `/home/{username}`）下拥有
//! 私有空间，站点文档根与站点日志统一规划在：
//! - `{home_dir}/www/{sanitize(site)}-{site_id}` —— 站点文档根
//! - `{home_dir}/logs/{sanitize(site)}-{site_id}` —— 站点 access/error 日志
//!
//! 安全边界：只接受 `/home/` 下的绝对路径；禁止 `..` 穿越。

use std::path::PathBuf;

use serde_json::json;
use zap_proto::Response;

fn home_dir_ok(home: &str) -> bool {
    home.starts_with("/home/")
        && PathBuf::from(home).is_absolute()
        && !home.split('/').any(|s| s == "..")
}

pub async fn home_init(home_dir: &str) -> Response {
    let home_dir = home_dir.to_string();
    tokio::task::spawn_blocking(move || home_init_inner(&home_dir))
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .map_or_else(|e| Response::err(-1, e), |r| r)
}

fn home_init_inner(home_dir: &str) -> Result<Response, String> {
    let home = home_dir.trim();
    if home.is_empty() {
        return Err("home_dir 不能为空".to_string());
    }
    if !home_dir_ok(home) {
        return Err(format!("home_dir 非法（必须为 /home/ 下的绝对路径）: {home}"));
    }
    let home_p = PathBuf::from(home);
    std::fs::create_dir_all(&home_p).map_err(|e| format!("创建家目录失败 {home_p:?}: {e}"))?;
    let mut created: Vec<String> = Vec::new();
    for sub in ["www", "logs"] {
        let d = home_p.join(sub);
        std::fs::create_dir_all(&d).map_err(|e| format!("创建子目录失败 {d:?}: {e}"))?;
        created.push(d.to_string_lossy().to_string());
    }
    Ok(Response::ok(
        format!("家目录已就绪：{home}"),
        Some(json!({ "home_dir": home, "dirs": created })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_dir_ok_validates() {
        assert!(home_dir_ok("/home/zap"));
        assert!(home_dir_ok("/home/zap/www"));
        assert!(!home_dir_ok("home/zap")); // 相对路径
        assert!(!home_dir_ok("/root")); // 不在 /home 下
        assert!(!home_dir_ok("/home/../etc")); // 穿越
        assert!(!home_dir_ok("/home/zap/../../etc"));
        assert!(!home_dir_ok(""));
    }
}
