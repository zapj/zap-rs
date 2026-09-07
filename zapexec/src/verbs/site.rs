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

pub(super) fn zap_path() -> PathBuf {
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

/// 站点骨架目录：`{ZAP_PATH}/scripts/zap/skel/`，其中的 index.html 为新站点的默认首页模板。
/// 运维可直接修改该模板（支持 __SITE_NAME__ / __SITE_ID__ / __SITE_DOMAINS__ /
/// __SITE_ROOT__ / __CREATED_AT__ 占位符），下次建站即生效。
fn skel_file() -> PathBuf {
    zap_path().join("scripts/zap/skel/index.html")
}

/// skel 模板缺失时的兜底页（保证离线/精简部署也能建站成功）
fn fallback_index_html() -> String {
    "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n<meta charset=\"utf-8\">\n\
     <title>站点已创建</title>\n</head>\n<body>\n<h1>站点已创建</h1>\n\
     <p>此页面由 Zap 面板自动生成，将站点文件放入本目录即可。</p>\n</body>\n</html>\n"
        .to_string()
}

/// 用 skel 模板渲染站点默认首页（模板不存在时回退内置页面）
fn render_index_html(site_id: i64, name: &str, domains: &[String], root: &Path) -> String {
    let tpl = match std::fs::read_to_string(skel_file()) {
        Ok(s) if !s.trim().is_empty() => s,
        _ => return fallback_index_html(),
    };
    let created_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    apply_placeholders(&tpl, site_id, name, domains, root, &created_at)
}

/// 纯函数：替换 skel 模板中的占位符（单测覆盖）
fn apply_placeholders(
    tpl: &str,
    site_id: i64,
    name: &str,
    domains: &[String],
    root: &Path,
    created_at: &str,
) -> String {
    let domains_text = if domains.is_empty() {
        "未绑定域名".to_string()
    } else {
        domains.join("、")
    };
    tpl.replace("__SITE_NAME__", name)
        .replace("__SITE_ID__", &site_id.to_string())
        .replace("__SITE_DOMAINS__", &domains_text)
        .replace("__SITE_ROOT__", &root.to_string_lossy())
        .replace("__CREATED_AT__", created_at)
}

fn ensure_web_root(
    root: &Path,
    site_id: i64,
    name: &str,
    domains: &[String],
) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| format!("{e}"))?;
    let index = root.join("index.html");
    if !index.exists() {
        std::fs::write(&index, render_index_html(site_id, name, domains, root))
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

/// 面板托管的默认站点（常驻 sites-enabled）：
/// 停止/未绑定的域名落到这里，直接断开连接（444），
/// 避免被同机的其它站点按 default_server 规则"接走"造成串站。
fn render_default_vhost() -> String {
    concat!(
        "# Generated by Zap Panel — 默认站点（未匹配 / 已停止的域名）— DO NOT EDIT\n",
        "server {\n",
        "    listen 80 default_server;\n",
        "    listen [::]:80 default_server;\n",
        "    server_name _;\n",
        "    return 444;\n",
        "}\n"
    )
    .to_string()
}

/// 维护页目录：`{ZAP_PATH}/data/www/_zap`（面板自管，不占用站点目录）
fn maintenance_dir() -> PathBuf {
    zap_path().join("data/www/_zap")
}

/// 维护页：不存在时生成一份默认页面（管理员可直接改这个文件定制内容）
fn ensure_maintenance_page() -> Result<PathBuf, String> {
    let dir = maintenance_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建维护页目录失败: {e}"))?;
    let file = dir.join("maintenance.html");
    if !file.exists() {
        let html = concat!(
            "<!DOCTYPE html>\n",
            "<html lang=\"zh-CN\">\n",
            "<head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">",
            "<title>站点维护中</title>\n",
            "<style>body{font-family:system-ui,-apple-system,\"PingFang SC\",\"Microsoft YaHei\",sans-serif;",
            "display:flex;align-items:center;justify-content:center;height:100vh;margin:0;",
            "background:#f5f7fa;color:#303133}",
            ".box{text-align:center;padding:32px}",
            "h1{font-size:20px;margin:0 0 8px}p{color:#909399;font-size:14px;margin:0}</style>\n",
            "</head>\n<body><div class=\"box\">",
            "<h1>站点维护中</h1><p>我们正在维护该站点，请稍后再访问。</p>",
            "</div></body></html>\n"
        );
        std::fs::write(&file, html).map_err(|e| format!("写入维护页失败: {e}"))?;
    }
    // nginx worker 以 www 组读取，页面需可读
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
    Ok(dir)
}

/// 维护态 vhost：503 + 维护页（站点域名仍匹配，但不再走业务目录/PHP）
fn render_maintenance_vhost(site_id: i64, name: &str, domains: &[String], maint: &str) -> String {
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
    format!(
        "# Generated by Zap Panel — site \"{comment}\" (id={site_id}) 维护中 — DO NOT EDIT\n\
         server {{\n\
         \x20   listen 80;\n\
         \x20   listen [::]:80;\n\
         \x20   server_name {server_name};\n\
         \x20   root {maint};\n\
         \x20   error_page 503 /maintenance.html;\n\
         \n\
         \x20   location = /maintenance.html {{\n\
         \x20       root {maint};\n\
         \x20       default_type text/html;\n\
         \x20   }}\n\
         \n\
         \x20   location / {{\n\
         \x20       return 503;\n\
         \x20   }}\n\
         }}\n"
    )
}

/// 发布面板默认站点（best-effort）：与主配置已有的 default_server 冲突时自动回滚，
/// 只告警不阻断站点本身的发布。
fn ensure_default_vhost(conf_file: &Path, bin: &Path) {
    let edir = super::webconf::enabled_dir("nginx");
    let injected = match super::webconf::ensure_include(conf_file, &edir, "nginx") {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("默认站点未发布（无法注入 include）: {e}");
            return;
        }
    };
    if let Err(e) =
        super::webconf::publish_named("nginx", "00-default.conf", &render_default_vhost())
    {
        tracing::warn!("默认站点发布失败: {e}");
        return;
    }
    if let Err(e) = nginx_test(bin) {
        let _ = super::webconf::purge_named("nginx", "00-default.conf");
        if injected {
            super::webconf::restore_include(conf_file);
        }
        tracing::warn!("默认站点导致 nginx -t 失败，已回滚（不影响站点本身）: {e}");
    }
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
    mode: Option<String>,
    php_socket: Option<String>,
    web_root: Option<String>,
    log_root: Option<String>,
    owner_user: Option<String>,
) -> Response {
    tokio::task::spawn_blocking(move || -> Result<Response, String> {
        vhost_sync_inner(
            site_id, &name, &domains, enabled, mode, php_socket, web_root, log_root, owner_user,
        )
    })
    .await
    .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
    .unwrap_or_else(|e| Response::err(-1, e))
}

/// 站点运行状态：running / stopped / maintenance（None 时按 enabled 推导，兼容老面板）
fn run_mode(mode: Option<&str>, enabled: bool) -> &str {
    match mode.unwrap_or("").trim().to_lowercase().as_str() {
        "stopped" | "stop" => "stopped",
        "maintenance" | "maintain" => "maintenance",
        "running" | "start" => "running",
        _ => {
            if enabled {
                "running"
            } else {
                "stopped"
            }
        }
    }
}

#[allow(clippy::too_many_arguments)] // 同 vhost_sync，同步所需配置字段固定
fn vhost_sync_inner(
    site_id: i64,
    name: &str,
    domains: &[String],
    enabled: bool,
    mode: Option<String>,
    php_socket: Option<String>,
    web_root: Option<String>,
    log_root: Option<String>,
    owner_user: Option<String>,
) -> Result<Response, String> {
    let state = run_mode(mode.as_deref(), enabled);
    let conf_file = match find_nginx_conf_file() {
        Some(c) => c,
        None => {
            return if state == "running" {
                Err(
                    "未找到 Nginx 安装（安装根 /usr/local/apps 下无 conf/nginx.conf）。\
                     请先在「应用商店 → Web服务器 → Nginx」安装并部署 Nginx"
                        .to_string(),
                )
            } else {
                // 站点停用且无 Nginx：无事可清理
                Ok(Response::ok("站点已停用（Nginx 未安装，无需清理）", None))
            };
        }
    };
    let bin = nginx_bin(&conf_file);
    // 生效配置统一放 /etc/zap/webservers/nginx/{sites-available,sites-enabled}；
    // 旧位置（<nginx prefix>/conf/sites-enabled）仅用于一次性迁移。
    let legacy_dir = vhosts_dir(&conf_file).ok();
    let edir = super::webconf::enabled_dir("nginx");
    let avail = super::webconf::available_path("nginx", site_id);

    if let Some(dir) = &legacy_dir {
        super::webconf::migrate_legacy(dir, "nginx", site_id)?;
    }

    // 默认站点常驻（停止/未匹配域名不再落到别的站点上）
    ensure_default_vhost(&conf_file, &bin);

    if state == "stopped" {
        if super::webconf::unpublish("nginx", site_id)? && nginx_running() {
            reload_nginx(&bin)?;
        }
        super::webconf::backup_snapshot(site_id, "nginx", "# 站点已停止\n");
        return Ok(Response::ok(
            "站点已停止（配置保留在 sites-available，访问由默认站点接管）",
            None,
        ));
    }

    // 维护态：只发布维护页 vhost，不触碰业务目录与 PHP
    if state == "maintenance" {
        let maint = ensure_maintenance_page()?;
        let content = render_maintenance_vhost(site_id, name, domains, &maint.to_string_lossy());
        let previous = std::fs::read_to_string(&avail).ok();
        super::webconf::publish("nginx", site_id, &content)?;
        if let Err(e) = nginx_test(&bin) {
            match previous {
                Some(prev) => {
                    let _ = std::fs::write(&avail, prev);
                }
                None => {
                    let _ = super::webconf::purge("nginx", site_id);
                }
            }
            return Err(format!("nginx -t 校验失败，已回滚维护配置：\n{e}"));
        }
        super::webconf::backup_snapshot(site_id, "nginx", &content);
        if nginx_running() {
            reload_nginx(&bin)?;
            return Ok(Response::ok("站点已切到维护页，Nginx 已重载", None));
        }
        return Ok(Response::ok("维护页配置已写入并通过校验", None));
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
    ensure_web_root(&root, site_id, name, domains)?;
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

    // 面板侧快照（渲染源 / 入参 / 历史版本）：失败不影响发布，仅作排障与回滚副本
    let meta = json!({
        "site_id": site_id,
        "name": name,
        "domains": domains,
        "web_root": root.to_string_lossy(),
        "log_root": log_root,
        "owner_user": owner_user,
        "php_socket": php_socket,
        "enabled": enabled,
    });
    if let Err(e) = super::webconf::write_snapshot(site_id, "nginx", &content, &meta) {
        tracing::warn!("写入站点配置快照失败（不影响发布）: {e}");
    }

    // 主配置幂等注入 include（指向新的 sites-enabled）；返回 true 表示本次改了主配置
    let injected = super::webconf::ensure_include(&conf_file, &edir, "nginx")?;

    // 发布前保留上一版内容，便于校验失败时回滚
    let previous = std::fs::read_to_string(&avail).ok();
    if let Err(e) = super::webconf::publish("nginx", site_id, &content) {
        if injected {
            super::webconf::restore_include(&conf_file);
        }
        return Err(e);
    }

    // 校验：失败即回滚（恢复上一版或撤下本次发布），绝不带着坏配置 reload
    if let Err(e) = nginx_test(&bin) {
        match previous {
            Some(prev) => {
                super::webconf::backup_snapshot(site_id, "nginx", &content);
                let _ = std::fs::write(&avail, prev);
            }
            None => {
                let _ = super::webconf::purge("nginx", site_id);
            }
        }
        if injected {
            super::webconf::restore_include(&conf_file);
        }
        return Err(format!("nginx -t 校验失败，已回滚本次配置：\n{e}"));
    }
    // 通过校验：留一份历史版本用于回滚
    super::webconf::backup_snapshot(site_id, "nginx", &content);

    let data = json!({
        "site_id": site_id,
        "available": avail.to_string_lossy().to_string(),
        "enabled": super::webconf::enabled_path("nginx", site_id).to_string_lossy().to_string(),
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
        // 旧布局遗留文件一并清理
        if let Ok(vdir) = vhosts_dir(&conf_file) {
            let legacy = vdir.join(super::webconf::vhost_name(site_id));
            if legacy.exists() {
                let _ = std::fs::remove_file(&legacy);
            }
        }
        if !super::webconf::purge("nginx", site_id)? {
            return Ok(Response::ok("vhost 不存在，无需清理", None));
        }
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
    fn skel_placeholders_are_replaced() {
        let tpl =
            "N=__SITE_NAME__;I=__SITE_ID__;D=__SITE_DOMAINS__;R=__SITE_ROOT__;C=__CREATED_AT__";
        let out = apply_placeholders(
            tpl,
            7,
            "blog",
            &["a.com".into(), "b.com".into()],
            Path::new("/home/u/www/blog-7"),
            "2026-09-06 10:00:00",
        );
        assert_eq!(
            out,
            "N=blog;I=7;D=a.com、b.com;R=/home/u/www/blog-7;C=2026-09-06 10:00:00"
        );
    }

    #[test]
    fn skel_placeholders_without_domain() {
        let out = apply_placeholders("D=__SITE_DOMAINS__", 1, "x", &[], Path::new("/r"), "");
        assert_eq!(out, "D=未绑定域名");
    }

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

    #[test]
    fn run_mode_resolution() {
        // 老面板只有 enabled：true→running，false→stopped
        assert_eq!(run_mode(None, true), "running");
        assert_eq!(run_mode(None, false), "stopped");
        assert_eq!(run_mode(Some("STOPPED"), true), "stopped");
        assert_eq!(run_mode(Some("maintenance"), true), "maintenance");
        // 未知值回退 enabled
        assert_eq!(run_mode(Some("bogus"), false), "stopped");
    }

    #[test]
    fn default_and_maintenance_vhost_render() {
        let d = render_default_vhost();
        assert!(d.contains("listen 80 default_server"));
        assert!(d.contains("return 444"), "默认站点应直接断开，避免串站");

        let m =
            render_maintenance_vhost(3, "blog", &["a.com".into(), "b.com".into()], "/srv/maint");
        assert!(m.contains("server_name a.com b.com"));
        assert!(m.contains("root /srv/maint"));
        assert!(m.contains("error_page 503 /maintenance.html"));
        assert!(m.contains("return 503"));
    }
}
