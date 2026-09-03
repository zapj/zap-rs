//! 站点 Nginx vhost 同步（root 执行）。
//!
//! 契约（与 appstore 安装的 Nginx 应用配合）：
//! - vhost 文件写入 `<nginx prefix>/conf/sites-enabled/zap-site-{id}.conf`
//!   （目录由 nginx.conf 中的 `include sites-enabled/*.conf` / `conf.d/*.conf` 自动探测）
//! - 站点文档根：`{ZAP_PATH}/data/www/{sanitize(name)}-{id}/`，首次同步自动创建并写占位 index.html
//! - 配置写入后先执行 `nginx -t` 校验，失败即回滚删除文件，绝不带着坏配置 reload
//! - PHP 联动：`php_socket` 形如 `unix:/path` 或 `host:port`；为 None 时不生成 PHP location
//!
//! 安全边界：文件名由 site_id 决定；名称仅用于注释与目录名（sanitize 后）；
//! 不执行任何来自站点输入的命令，只做文件渲染 + 白名单 nginx 校验/reload。

use std::path::{Path, PathBuf};

use serde_json::json;
use zap_proto::Response;

use super::root_cmd;

fn zap_path() -> PathBuf {
    std::env::var("ZAP_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/usr/local/zap"))
}

// ── Nginx 探测 ───────────────────────────────────────────────

/// 查找已部署 Nginx 的主配置 conf/nginx.conf：
/// 1) 环境变量 `ZAP_NGINX_PREFIX/conf/nginx.conf`（若手工部署可指定）
/// 2) 软件安装根（默认 /usr/local/apps，`ZAP_APPS_DIR` 可覆盖）下递归寻找
///    含 `include`（sites-enabled/conf.d）的运行时配置
pub(super) fn find_nginx_conf_file() -> Option<PathBuf> {
    if let Ok(prefix) = std::env::var("ZAP_NGINX_PREFIX") {
        let p = PathBuf::from(prefix).join("conf/nginx.conf");
        if p.is_file() {
            return Some(p);
        }
    }
    let mut cands = Vec::new();
    collect_nginx_confs(&super::install_root(), 0, &mut cands);
    // 优先选择带 sites-enabled include 的运行时配置
    cands.sort_by_key(|p| !runtime_nginx_hint(p));
    cands.into_iter().find(|p| runtime_nginx_hint(p))
}

fn collect_nginx_confs(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let conf = p.join("conf").join("nginx.conf");
        if conf.is_file() {
            out.push(conf);
        } else {
            collect_nginx_confs(&p, depth + 1, out);
        }
    }
}

fn runtime_nginx_hint(conf: &Path) -> bool {
    std::fs::read_to_string(conf)
        .map(|s| {
            s.contains("include")
                && (s.contains("sites-enabled") || s.contains("conf.d") || s.contains("vhost"))
        })
        .unwrap_or(false)
}

/// 解析 nginx.conf 里 include 的站点配置目录（相对 conf 目录）。
/// 优先 sites-enabled，其次 conf.d / vhost。
pub(super) fn vhosts_dir(nginx_conf: &Path) -> Result<PathBuf, String> {
    let conf_dir = nginx_conf
        .parent()
        .ok_or_else(|| "nginx.conf 父目录无效".to_string())?;
    let text =
        std::fs::read_to_string(nginx_conf).map_err(|e| format!("读取 nginx.conf 失败: {e}"))?;
    let mut targets: Vec<&str> = text
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            let rest = t.strip_prefix("include")?.trim();
            let rest = rest.trim_end_matches(';').trim();
            if rest.is_empty()
                || rest.contains("mime.types")
                || rest.contains("fastcgi")
                || rest.starts_with("http_")
            {
                return None;
            }
            Some(rest)
        })
        .collect();
    targets.dedup();
    let chosen = targets
        .iter()
        .find(|t| t.contains("sites-enabled"))
        .or_else(|| {
            targets
                .iter()
                .find(|t| t.contains("conf.d") || t.contains("vhost"))
        })
        .or_else(|| targets.first());
    match chosen {
        Some(raw) => {
            // raw 形如 sites-enabled/*.conf 或 conf.d/*.conf；目录取第一段
            let name = raw
                .split(['/', ' '])
                .next()
                .unwrap_or(raw)
                .trim()
                .trim_end_matches('*');
            if name.is_empty() {
                return Err("无法从 nginx.conf include 中识别站点配置目录".to_string());
            }
            Ok(conf_dir.join(name))
        }
        None => Err(
            "nginx.conf 未 include 站点配置目录（sites-enabled / conf.d），请先在 Nginx 主配置中添加 include".to_string(),
        ),
    }
}

pub(super) fn nginx_bin(nginx_conf: &Path) -> PathBuf {
    let sbin = nginx_conf
        .parent()
        .and_then(|c| c.parent())
        .map(|p| p.join("sbin").join("nginx"))
        .filter(|p| p.is_file());
    sbin.unwrap_or_else(|| PathBuf::from("nginx"))
}

pub(super) fn nginx_running() -> bool {
    for pid_file in [PathBuf::from("/var/run/nginx.pid")] {
        if let Ok(content) = std::fs::read_to_string(&pid_file)
            && let Ok(pid) = content.trim().parse::<i32>()
            && pid > 0
            && root_cmd("kill")
                .args(["-0", &pid.to_string()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        {
            return true;
        }
    }
    root_cmd("pgrep")
        .args(["-x", "nginx"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// 提取 nginx 命令的 stderr（截断）
fn output_err(o: &std::process::Output, fallback: &str) -> String {
    let text = String::from_utf8_lossy(&o.stderr).trim().to_string();
    let text = if text.is_empty() {
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    } else {
        text
    };
    let text = if text.is_empty() {
        fallback.to_string()
    } else {
        text
    };
    let mut out = text.lines().take(12).collect::<Vec<_>>().join("\n");
    if out.len() > 1500 {
        out = out.chars().take(1500).collect();
    }
    out
}

// ── 文档根 ───────────────────────────────────────────────────

fn sanitize_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            // 保留字母/数字/_/-，其余（含 . 与空格等）统一转 '-'，避免隐藏目录/路径穿越
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    out = out.trim_matches('-').to_string();
    if out.is_empty() {
        out = "site".to_string();
    }
    if out.chars().count() > 48 {
        out = out.chars().take(48).collect();
    }
    out
}

fn default_web_root(name: &str, site_id: i64) -> PathBuf {
    zap_path()
        .join("data/www")
        .join(format!("{}-{site_id}", sanitize_name(name)))
}

fn ensure_web_root(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| format!("{e}"))?;
    let index = root.join("index.html");
    if !index.exists() {
        std::fs::write(
            &index,
            "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n\
             <title>站点已创建</title>\n</head>\n<body>\n<h1>站点已创建</h1>\n\
             <p>此页面由 Zap 面板自动生成，将站点文件放入本目录即可。</p>\n</body>\n</html>\n",
        )
        .map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

// ── 渲染（纯函数，单测覆盖）──────────────────────────────────

fn render_vhost(
    site_id: i64,
    name: &str,
    domains: &[String],
    root: &str,
    php_socket: Option<&str>,
    access_log: Option<&str>,
    error_log: Option<&str>,
) -> String {
    let comment = name.chars().filter(|c| !c.is_control()).collect::<String>();
    let server_name = {
        let parts: Vec<&str> = domains
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            "_".to_string()
        } else {
            parts.join(" ")
        }
    };
    let mut out = String::new();
    out.push_str(&format!(
        "# Generated by Zap Panel — site \"{comment}\" (id={site_id}) — DO NOT EDIT\n"
    ));
    out.push_str("server {\n");
    out.push_str("    listen 80;\n");
    out.push_str("    listen [::]:80;\n");
    out.push_str(&format!("    server_name {server_name};\n"));
    out.push_str(&format!("    root {root};\n"));
    if let Some(p) = access_log {
        out.push_str(&format!("    access_log {p};\n"));
    }
    if let Some(p) = error_log {
        out.push_str(&format!("    error_log {p};\n"));
    }
    if php_socket.is_some() {
        out.push_str("    index index.php index.html;\n");
    } else {
        out.push_str("    index index.html;\n");
    }
    out.push_str("\n    location / {\n        try_files $uri $uri/ =404;\n    }\n");
    if let Some(sock) = php_socket {
        out.push_str("\n    # PHP 实例联动\n");
        out.push_str("    location ~ \\.php$ {\n");
        out.push_str(&format!("        fastcgi_pass {sock};\n"));
        out.push_str("        fastcgi_index index.php;\n        include fastcgi_params;\n");
        out.push_str(
            "        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n    }\n",
        );
    }
    out.push_str("}\n");
    out
}

// ── 同步 / 移除入口 ──────────────────────────────────────────

/// 目录参数校验：必须是绝对路径且不含 `..`（zapd 传入的家目录/站点路径来自用户表，双保险）
fn dir_arg_ok(p: &str) -> bool {
    p.starts_with('/') && !p.split('/').any(|s| s == "..")
}

/// 递归收敛站点树属主与权限（幂等）：
/// - web tree：chown -R {owner}:www；目录 750 / 文件 640（nginx 以组 www 读取）
/// - log tree（is_log=true）：chown -R www:www；目录 770 / 文件 660（nginx 写入日志）
fn fix_tree_owner(root: &Path, owner: &str, is_log: bool) -> Result<(), String> {
    let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let dir_mode = if is_log { "770" } else { "750" };
    let file_mode = if is_log { "660" } else { "640" };
    let root_s = root.to_string_lossy();
    let script = format!(
        "chown -R {}:www {} && find {} -type d -exec chmod {} \\; && find {} -type f -exec chmod {} \\;",
        q(owner),
        q(&root_s),
        q(&root_s),
        dir_mode,
        q(&root_s),
        file_mode
    );
    let o = root_cmd("bash")
        .args(["-c", &script])
        .output()
        .map_err(|e| format!("收敛站点树属主/权限失败: {e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!(
            "chown/chmod 站点树失败：{}",
            output_err(&o, "未知错误")
        ))
    }
}

#[allow(clippy::too_many_arguments)] // 同步站点完整 vhost 配置所需，参数固定且各司其职
pub async fn vhost_sync(
    site_id: i64,
    name: String,
    domains: Vec<String>,
    enabled: bool,
    php_socket: Option<String>,
    web_root: Option<String>,
    log_root: Option<String>,
    owner_user: Option<String>,
) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        vhost_sync_inner(
            site_id, &name, &domains, enabled, php_socket, web_root, log_root, owner_user,
        )
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

#[allow(clippy::too_many_arguments)] // 同 vhost_sync，同步所需配置字段固定
fn vhost_sync_inner(
    site_id: i64,
    name: &str,
    domains: &[String],
    enabled: bool,
    php_socket: Option<String>,
    web_root: Option<String>,
    log_root: Option<String>,
    owner_user: Option<String>,
) -> Result<Response, String> {
    let conf_file = match find_nginx_conf_file() {
        Some(c) => c,
        None => {
            return if enabled {
                Err("未找到 Nginx 安装（安装根 /usr/local/apps 下无 conf/nginx.conf）。\
                     请先在「应用商店 → Web服务器 → Nginx」安装并部署 Nginx"
                    .to_string())
            } else {
                // 站点停用且无 Nginx：无事可清理
                Ok(Response::ok("站点已停用（Nginx 未安装，无需清理）", None))
            };
        }
    };
    let bin = nginx_bin(&conf_file);
    let vdir = vhosts_dir(&conf_file)?;
    let vhost_file = vdir.join(format!("zap-site-{site_id}.conf"));

    if !enabled {
        if vhost_file.exists() {
            std::fs::remove_file(&vhost_file).map_err(|e| format!("移除 vhost 失败: {e}"))?;
            if nginx_running() {
                reload_nginx(&bin)?;
            }
        }
        return Ok(Response::ok("站点已停用，vhost 已移除", None));
    }

    // 文档根：优先采用面板入库的 web_root（位于归属用户家目录下）；
    // 为空/未提供时回退 {ZAP_PATH}/data/www/{sanitize(name)}-{id}
    let root = match web_root.as_deref() {
        Some(w) if !w.trim().is_empty() => {
            if !dir_arg_ok(w) {
                return Err("web_root 必须是形如 /home/u/www/xxx 的绝对路径".to_string());
            }
            PathBuf::from(w)
        }
        _ => default_web_root(name, site_id),
    };
    // create_dir_all 会递归创建归属用户家目录骨架（/home/{u}/www/...）
    ensure_web_root(&root)?;
    // 站点树属主/权限收敛：
    // - web tree：owner_user（独立系统用户模式）或 www；组恒为 www，目录 750 / 文件 640
    //   （nginx worker 以组 www 读静态文件，php-fpm 以 owner 身份读写）
    // - log tree：恒归 www:www，目录 770 / 文件 660（nginx 写 access/error.log）
    let web_owner = owner_user
        .as_deref()
        .filter(|u| !u.is_empty())
        .unwrap_or("www");
    fix_tree_owner(&root, web_owner, false)?;

    // 日志：面板规划了 log_root（{home}/logs/{site}）时生成独立 access/error 日志
    let (mut access_log, mut error_log) = (None, None);
    if let Some(lr) = log_root.as_deref() {
        let lr = lr.trim();
        if !lr.is_empty() {
            if !dir_arg_ok(lr) {
                return Err("log_root 必须是形如 /home/u/logs/xxx 的绝对路径".to_string());
            }
            let ldir = PathBuf::from(lr);
            std::fs::create_dir_all(&ldir).map_err(|e| format!("创建站点日志目录失败: {e}"))?;
            fix_tree_owner(&ldir, "www", true)?;
            access_log = Some(ldir.join("access.log").to_string_lossy().to_string());
            error_log = Some(ldir.join("error.log").to_string_lossy().to_string());
        }
    }

    let content = render_vhost(
        site_id,
        name,
        domains,
        &root.to_string_lossy(),
        php_socket.as_deref(),
        access_log.as_deref(),
        error_log.as_deref(),
    );

    std::fs::create_dir_all(&vdir).map_err(|e| format!("创建 vhost 目录失败: {e}"))?;
    std::fs::write(&vhost_file, &content).map_err(|e| format!("写入 vhost 失败: {e}"))?;

    // 校验：失败回滚删除文件，绝不带着坏配置 reload
    if let Err(e) = nginx_test(&bin) {
        let _ = std::fs::remove_file(&vhost_file);
        return Err(format!("nginx -t 校验失败，已回滚本次配置：\n{e}"));
    }

    let data = json!({
        "site_id": site_id,
        "vhost": vhost_file.to_string_lossy().to_string(),
        "root": root.to_string_lossy().to_string(),
    });
    if nginx_running() {
        reload_nginx(&bin)?;
        Ok(Response::ok("站点配置已同步，Nginx 已重载", Some(data)))
    } else {
        Ok(Response::ok(
            "站点配置已写入并通过校验；Nginx 当前未运行，启动后自动生效",
            Some(data),
        ))
    }
}

pub async fn vhost_remove(site_id: i64, name: String) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        let Some(conf_file) = find_nginx_conf_file() else {
            return Ok(Response::ok("Nginx 未安装，无需清理", None));
        };
        let bin = nginx_bin(&conf_file);
        let vdir = vhosts_dir(&conf_file)?;
        let vhost_file = vdir.join(format!("zap-site-{site_id}.conf"));
        if !vhost_file.exists() {
            return Ok(Response::ok("vhost 不存在，无需清理", None));
        }
        std::fs::remove_file(&vhost_file).map_err(|e| format!("移除 vhost 失败: {e}"))?;
        if nginx_running() {
            reload_nginx(&bin)?;
        }
        Ok(Response::ok(
            format!("vhost 已移除（site {site_id} {name}）"),
            None,
        ))
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

fn nginx_test(bin: &Path) -> Result<(), String> {
    let o = root_cmd("bash")
        .args(["-c"])
        .arg(format!(
            "'{}' -t 2>&1",
            bin.to_string_lossy().replace('\'', "'\\''")
        ))
        .output()
        .map_err(|e| format!("执行 nginx -t 失败: {e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(output_err(&o, "nginx -t 返回非零"))
    }
}

fn reload_nginx(bin: &Path) -> Result<(), String> {
    let o = root_cmd("bash")
        .args(["-c"])
        .arg(format!(
            "'{}' -s reload 2>&1",
            bin.to_string_lossy().replace('\'', "'\\''")
        ))
        .output()
        .map_err(|e| format!("执行 nginx -s reload 失败: {e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(format!(
            "nginx -s reload 失败：{}",
            output_err(&o, "未知错误")
        ))
    }
}

// ── 单测 ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_static_vhost() {
        let s = render_vhost(
            1,
            "blog",
            &["a.com".into(), "b.com".into()],
            "/zap/www/blog-1",
            None,
            None,
            None,
        );
        assert!(s.contains("server_name a.com b.com;"));
        assert!(s.contains("root /zap/www/blog-1;"));
        assert!(s.contains("index index.html;"));
        assert!(!s.contains("fastcgi"));
        assert!(!s.contains("index.php"));
        assert!(!s.contains("access_log"));
    }

    #[test]
    fn render_php_unix_socket() {
        let s = render_vhost(
            2,
            "app",
            &["app.example.com".into()],
            "/zap/www/app-2",
            Some("unix:/var/run/php-fpm-8.3.sock"),
            None,
            None,
        );
        assert!(s.contains("index index.php index.html;"));
        assert!(s.contains("fastcgi_pass unix:/var/run/php-fpm-8.3.sock;"));
        assert!(s.contains("SCRIPT_FILENAME $document_root$fastcgi_script_name"));
    }

    #[test]
    fn render_php_tcp() {
        let s = render_vhost(
            3,
            "x",
            &[],
            "/zap/www/x-3",
            Some("127.0.0.1:9000"),
            None,
            None,
        );
        assert!(s.contains("server_name _;"));
        assert!(s.contains("fastcgi_pass 127.0.0.1:9000;"));
    }

    #[test]
    fn render_with_site_logs() {
        let s = render_vhost(
            7,
            "b",
            &["b.com".into()],
            "/home/u/www/b-7",
            None,
            Some("/home/u/logs/b-7/access.log"),
            Some("/home/u/logs/b-7/error.log"),
        );
        assert!(s.contains("root /home/u/www/b-7;"));
        assert!(s.contains("access_log /home/u/logs/b-7/access.log;"));
        assert!(s.contains("error_log /home/u/logs/b-7/error.log;"));
    }

    #[test]
    fn sanitize_names() {
        assert_eq!(sanitize_name("我的 博客"), "site"); // 全中文/空格 → 全 '-'，trim 后空 → site
        assert_eq!(sanitize_name("my blog/x"), "my-blog-x");
        assert_eq!(sanitize_name(".."), "site");
        assert_eq!(sanitize_name("ABC_123"), "ABC_123");
    }
}
