//! Nginx 服务配置与运行状态管理（root 执行）。
//!
//! 提供「服务器配置 → Nginx 配置」（可视化 + 文件编辑）与
//! 「服务器状态 → Nginx Server」两个页面的后端能力：
//!
//! - 状态探测：安装位置 / 版本 / 主配置路径 / 运行态（systemd 或 pid）
//! - 配置读写：仅允许操作主配置及其 conf 目录树内的 `*.conf`
//!   （面板托管的站点 vhost 位于 `/etc/zap/webservers`，不在白名单内；
//!    文件名以 `zap-` 开头的历史遗留配置也排除，避免误改托管文件）
//! - 保存链路：备份 → 写入 → `nginx -t` 校验 → 失败自动回滚 → 运行中则重载
//! - 服务控制：start / stop / restart / reload（优先 systemd unit `nginx`）

use std::path::{Path, PathBuf};

use serde_json::json;
use zap_proto::Response;

use super::root_cmd;
use super::site;

/// 单个配置文件体积上限（读取与写入共用，防止意外读取超大文件拖垮连接）。
const MAX_CONF_BYTES: u64 = 2 * 1024 * 1024;

/// 从 conf 目录树内收集 *.conf 的最大深度（conf/、conf/conf.d/ 等两层足够）。
const SCAN_DEPTH: usize = 2;

// ── 探测与工具 ─────────────────────────────────────────────

/// (主配置, nginx 二进制)。未探测到安装时返回 None。
fn probe() -> Option<(PathBuf, PathBuf)> {
    let conf = site::find_nginx_conf_file()?;
    let bin = site::nginx_bin(&conf);
    Some((conf, bin))
}

/// 输出 stderr（截断），供校验失败等场景使用。
fn output_err(o: &std::process::Output, fallback: &str) -> String {
    let text = String::from_utf8_lossy(&o.stderr).trim().to_string();
    let text = if text.is_empty() {
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    } else {
        text
    };
    if text.is_empty() {
        fallback.to_string()
    } else {
        text.chars().take(2000).collect()
    }
}

fn quote_bin(bin: &Path) -> String {
    bin.to_string_lossy().replace('\'', "'\\''")
}

/// 读取 nginx 版本（`nginx -v` 的 stderr 首行）。
fn nginx_version(bin: &Path) -> String {
    let o = root_cmd("bash")
        .args(["-c"])
        .arg(format!("'{}' -v 2>&1", quote_bin(bin)))
        .output();
    match o {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stderr).trim().to_string();
            text.lines().next().unwrap_or("").trim().to_string()
        }
        Err(_) => String::new(),
    }
}

/// systemd 是否存在名为 `name` 的 unit（通过 list-unit-files 判断）。
fn systemd_has(name: &str) -> bool {
    root_cmd("systemctl")
        .args(["list-unit-files", "--no-legend", &format!("{name}.service")])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            out.lines().any(|l| l.trim().starts_with(name))
        })
        .unwrap_or(false)
}

/// systemd unit 当前是否 active。
fn systemd_active(name: &str) -> bool {
    root_cmd("systemctl")
        .args(["is-active", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 读取 master pid（/var/run/nginx.pid）。
fn read_pid() -> Option<i64> {
    let text = std::fs::read_to_string("/var/run/nginx.pid").ok()?;
    text.trim().parse::<i64>().ok().filter(|p| *p > 0)
}

/// 反复探测直至稳定（最多约 3 秒），返回最终 running 状态。
fn settle_running(want: bool, tries: u32) -> bool {
    for _ in 0..tries {
        let running = site::nginx_running();
        if running == want {
            return running;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    site::nginx_running()
}

/// 校验路径属于可编辑白名单：主配置本身，或主配置 conf 目录内的 *.conf
/// （排除 zap 托管前缀与备份/临时文件）。
fn validate_conf_path(raw: &str) -> Result<(PathBuf, PathBuf, bool), String> {
    let Some((conf, bin)) = probe() else {
        return Err("Nginx 未安装或未探测到主配置，请先在应用商店安装 Nginx".to_string());
    };
    let conf_dir = conf
        .parent()
        .ok_or_else(|| "主配置路径异常".to_string())?
        .to_path_buf();
    let main_canon = conf
        .canonicalize()
        .map_err(|e| format!("主配置不可访问: {e}"))?;

    let path = PathBuf::from(raw);
    let canon = path
        .canonicalize()
        .map_err(|e| format!("文件不存在或不可访问: {e}"))?;

    let is_main = canon == main_canon;
    if is_main {
        return Ok((main_canon, bin, true));
    }
    // 非主配置：必须位于 conf 目录树内、.conf 结尾、非托管前缀
    let conf_dir_canon = conf_dir
        .canonicalize()
        .map_err(|e| format!("conf 目录不可访问: {e}"))?;
    if !canon.starts_with(&conf_dir_canon) {
        return Err("仅允许编辑 Nginx 主配置 conf 目录内的 *.conf 文件".to_string());
    }
    let name = canon
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    if !name.ends_with(".conf") {
        return Err("仅支持编辑 .conf 配置文件".to_string());
    }
    if name.starts_with("zap-")
        || name.ends_with(".bak")
        || name.ends_with(".tmp")
        || name.contains(".zap")
    {
        return Err("该文件为面板托管的站点/缓存配置，请勿直接编辑".to_string());
    }
    Ok((canon, bin, false))
}

/// 数据区备份目录：`{ZAP_PATH}/data/nginx/backups`。
fn backup_dir() -> PathBuf {
    site::zap_path().join("data/nginx/backups")
}

/// 备份当前文件（保留最近 20 份）。
fn backup_file(path: &Path) -> Result<PathBuf, String> {
    let dir = backup_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建备份目录失败: {e}"))?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("conf");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = dir.join(format!("{ts}-{name}.bak"));
    std::fs::copy(path, &dest).map_err(|e| format!("备份失败: {e}"))?;

    // 修剪旧备份（只保留文件名时间戳最新的 20 份）
    let mut kept: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()
        .map(|rd| rd.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    kept.sort();
    while kept.len() > 20 {
        if let Some(old) = kept.first() {
            let _ = std::fs::remove_file(old);
            kept.remove(0);
        }
    }
    Ok(dest)
}

// ── verbs ──────────────────────────────────────────────────

/// nginx.status：安装 / 版本 / 配置路径 / 运行态。
pub async fn status() -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let Some((conf, bin)) = probe() else {
            return Ok(Response::ok(
                "nginx 未安装",
                Some(json!({ "installed": false })),
            ));
        };
        let running = site::nginx_running();
        Ok(Response::ok(
            "ok",
            Some(json!({
                "installed": true,
                "conf_file": conf.display().to_string(),
                "conf_dir": conf.parent().map(|p| p.display().to_string()).unwrap_or_default(),
                "bin": bin.display().to_string(),
                "version": nginx_version(&bin),
                "running": running,
                "pid": read_pid(),
                "systemd": systemd_has("nginx"),
                "systemd_active": systemd_active("nginx"),
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

/// nginx.conf_list：列出主配置 + conf 目录白名单文件。
pub async fn conf_list() -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let Some((conf, _bin)) = probe() else {
            return Ok(Response::ok(
                "nginx 未安装",
                Some(json!({
                    "installed": false,
                    "files": [],
                })),
            ));
        };
        let conf_dir = conf.parent().unwrap_or(Path::new("/")).to_path_buf();
        let mut files: Vec<serde_json::Value> = Vec::new();
        let meta = std::fs::metadata(&conf).map_err(|e| format!("读取主配置失败: {e}"))?;
        files.push(file_entry(&conf, &conf_dir, true, meta.len()));

        collect_conf_files(&conf_dir, &conf_dir, 0, &mut files);

        Ok(Response::ok(
            "ok",
            Some(json!({
                "installed": true,
                "conf_file": conf.display().to_string(),
                "conf_dir": conf_dir.display().to_string(),
                "files": files,
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

fn file_entry(path: &Path, base: &Path, is_main: bool, size: u64) -> serde_json::Value {
    let rel = path
        .strip_prefix(base)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    json!({
        "path": path.display().to_string(),
        "rel": rel,
        "name": path.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        "is_main": is_main,
        "size": size,
        "mtime": mtime,
    })
}

fn collect_conf_files(dir: &Path, base: &Path, depth: usize, out: &mut Vec<serde_json::Value>) {
    if depth > SCAN_DEPTH {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_conf_files(&p, base, depth + 1, out);
            continue;
        }
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !name.ends_with(".conf")
            || name.starts_with("zap-")
            || name.ends_with(".bak")
            || name.ends_with(".tmp")
            || name.contains(".zap")
        {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(&p) {
            if meta.len() > MAX_CONF_BYTES {
                continue; // 超出读取上限的文件不提供在线编辑
            }
            out.push(file_entry(&p, base, false, meta.len()));
        }
    }
}

/// nginx.conf_read：读取白名单文件内容。
pub async fn conf_read(path: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let (canon, _bin, is_main) = validate_conf_path(&path)?;
        let meta = std::fs::metadata(&canon).map_err(|e| format!("读取文件失败: {e}"))?;
        if meta.len() > MAX_CONF_BYTES {
            return Err("文件过大，不支持在线编辑（请到终端处理）".to_string());
        }
        let content =
            std::fs::read_to_string(&canon).map_err(|e| format!("读取文件失败: {e}"))?;
        Ok(Response::ok(
            "ok",
            Some(json!({
                "path": canon.display().to_string(),
                "is_main": is_main,
                "content": content,
                "size": meta.len(),
                "mtime": meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0),
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

/// nginx.conf_save：备份 → 写入 → `nginx -t` 校验 → 失败回滚 → 运行中重载。
pub async fn conf_save(path: String, content: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        if content.len() as u64 > MAX_CONF_BYTES {
            return Err("内容超过 2MB，请精简后重试".to_string());
        }
        let (canon, bin, is_main) = validate_conf_path(&path)?;

        let old = std::fs::read_to_string(&canon).map_err(|e| format!("读取原文件失败: {e}"))?;

        // 主配置保护：不得删除面板托管的站点 include 行（否则站点将全部失效）
        if is_main && old.contains("sites-enabled") && !content.contains("sites-enabled") {
            return Err(
                "配置中缺少面板托管的 include（sites-enabled 目录），已取消保存以避免站点全部失效。\n\
                 请保留 # zap: 面板托管站点配置 那一行 include"
                    .to_string(),
            );
        }

        let backup = backup_file(&canon)?;

        // 写入（同目录 tmp + rename，原子替换）
        let tmp = canon.with_extension("conf.tmp");
        std::fs::write(&tmp, &content).map_err(|e| format!("写入配置失败: {e}"))?;
        std::fs::rename(&tmp, &canon).map_err(|e| format!("替换配置失败: {e}"))?;

        // nginx -t 校验
        if let Err(e) = site::nginx_test(&bin) {
            // 回滚
            let _ = std::fs::copy(&backup, &canon);
            return Err(format!("nginx -t 校验未通过，已自动回滚原配置：\n{e}"));
        }

        // 重载（未运行则跳过，不视为错误）
        let running = site::nginx_running();
        let (reloaded, reason) = if running {
            match site::reload_nginx(&bin) {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e),
            }
        } else {
            (
                false,
                "nginx 未运行，配置已保存，将在启动时生效".to_string(),
            )
        };

        Ok(Response::ok(
            "ok",
            Some(json!({
                "path": canon.display().to_string(),
                "backup": backup.display().to_string(),
                "tested": true,
                "reloaded": reloaded,
                "reason": reason,
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

/// nginx.control：start / stop / restart / reload。
pub async fn control(action: &str) -> Response {
    let action = action.to_string();
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        if !matches!(action.as_str(), "start" | "stop" | "restart" | "reload") {
            return Err(format!("不支持的 Nginx 操作: {action}"));
        }
        let Some((_conf, bin)) = probe() else {
            return Err("Nginx 未安装".to_string());
        };

        // 优先 systemd unit
        let sysd = systemd_has("nginx");
        let method = if sysd {
            let o = root_cmd("systemctl")
                .args([action.as_str(), "nginx"])
                .output()
                .map_err(|e| format!("执行 systemctl {action} nginx 失败: {e}"))?;
            if !o.status.success() {
                return Err(format!(
                    "systemctl {action} nginx 失败：{}",
                    output_err(&o, "未知错误")
                ));
            }
            // restart 后稍等稳定
            if action == "restart" {
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
            "systemd".to_string()
        } else {
            // 二进制信号方式
            match action.as_str() {
                "reload" => {
                    if !site::nginx_running() {
                        return Err("Nginx 未运行，无需重载".to_string());
                    }
                    site::reload_nginx(&bin)?;
                }
                "stop" => {
                    if site::nginx_running() {
                        let o = root_cmd("bash")
                            .args(["-c"])
                            .arg(format!("'{}' -s quit 2>&1", quote_bin(&bin)))
                            .output()
                            .map_err(|e| format!("执行 nginx -s quit 失败: {e}"))?;
                        if !o.status.success() {
                            return Err(format!(
                                "nginx -s quit 失败：{}",
                                output_err(&o, "未知错误")
                            ));
                        }
                        settle_running(false, 10);
                    }
                }
                "start" | "restart" => {
                    if action == "restart" && site::nginx_running() {
                        let _ = root_cmd("bash")
                            .args(["-c"])
                            .arg(format!("'{}' -s quit 2>&1", quote_bin(&bin)))
                            .output();
                        settle_running(false, 10);
                    }
                    if !site::nginx_running() {
                        let o = root_cmd("bash")
                            .args(["-c"])
                            .arg(format!("'{}' 2>&1", quote_bin(&bin)))
                            .output()
                            .map_err(|e| format!("启动 nginx 失败: {e}"))?;
                        if !o.status.success() {
                            return Err(format!(
                                "启动 nginx 失败：{}",
                                output_err(&o, "未知错误")
                            ));
                        }
                        settle_running(true, 10);
                    }
                }
                _ => unreachable!(),
            }
            "binary".to_string()
        };

        let state = if site::nginx_running() {
            "running"
        } else {
            "stopped"
        };
        Ok(Response::ok(
            "ok",
            Some(json!({
                "action": action,
                "method": method,
                "state": state,
            })),
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_parse_is_robust() {
        // nginx_version 依赖系统命令，这里只验证文本取首行逻辑不 panic
        let s = "nginx version: nginx/1.27.3\nbuilt by gcc 12.2.0\n";
        let first = s.lines().next().unwrap_or("").trim().to_string();
        assert!(first.contains("nginx/1.27.3"));
    }
}
