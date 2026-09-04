//! 面板用户家目录骨架 / Linux 系统账号管理（root 执行）。
//!
//! 契约：每个面板用户在 `user.home_dir`（通常为 `/home/{linux_user}`）下拥有
//! 私有空间：
//! - `{home_dir}/www/{sanitize(site)}-{site_id}` —— 站点文档根（web tree）
//! - `{home_dir}/logs/{sanitize(site)}-{site_id}` —— 站点 access/error 日志（log tree）
//! - `{home_dir}/tmp` —— PHP session / 上传临时目录（open_basedir 白名单）
//!
//! 两种虚拟主机运行模式：
//! - `owner = None`（统一 www）：web tree 归 `www:www`，PHP-FPM 全局 www pool 运行
//! - `owner = Some(linux_user)`（独立系统用户）：web tree 归 `{u}:www`
//!   （nginx worker 以组 www 读取静态文件），PHP-FPM 每用户 pool 以 `{u}` 运行
//!
//! 安全边界：家目录只接受 `/home/` 下绝对路径；禁止 `..`；Linux 账号名白名单校验。

use std::path::{Path, PathBuf};

use serde_json::json;
use zap_proto::Response;

use super::root_cmd;

/// 家目录路径校验：必须是绝对路径、不含 `..`、且不是文件系统根。
/// 允许任意挂载点前缀（/home、/home2、/data/home 等），便于磁盘扩容迁移。
fn home_dir_ok(home: &str) -> bool {
    PathBuf::from(home).is_absolute()
        && !home.split('/').any(|s| s == "..")
        && home != "/"
        && !home.contains(' ')
}

fn linux_user_ok(u: &str) -> bool {
    if u.is_empty() || u.len() > 32 {
        return false;
    }
    let mut chars = u.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn cmd_err(o: &std::process::Output, fallback: &str) -> String {
    let text = String::from_utf8_lossy(&o.stderr).trim().to_string();
    let text = if text.is_empty() {
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    } else {
        text
    };
    if text.is_empty() {
        fallback.to_string()
    } else {
        text
    }
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ── user.home_init ──────────────────────────────────────────

pub async fn home_init(home_dir: &str, owner: Option<&str>) -> Response {
    let home_dir = home_dir.to_string();
    let owner = owner.map(|s| s.to_string());
    tokio::task::spawn_blocking(move || home_init_inner(&home_dir, owner.as_deref()))
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

fn run_bash(script: &str) -> Result<(), String> {
    let o = root_cmd("bash")
        .args(["-c", script])
        .output()
        .map_err(|e| format!("执行命令失败: {e}"))?;
    if o.status.success() {
        Ok(())
    } else {
        Err(cmd_err(&o, "命令执行失败"))
    }
}

fn home_init_inner(home_dir: &str, owner: Option<&str>) -> Result<Response, String> {
    let home = home_dir.trim();
    if home.is_empty() {
        return Err("home_dir 不能为空".to_string());
    }
    if !home_dir_ok(home) {
        return Err(format!(
            "home_dir 非法（必须为挂载点下的绝对路径，不含 ..）: {home}"
        ));
    }
    if let Some(u) = owner
        && !linux_user_ok(u)
    {
        return Err(format!("非法的 Linux 账号名: {u}"));
    }
    let run = owner.unwrap_or("www");
    // 进程主组：system 模式用账号独立组（不放进 www 组，避免跨用户读 php 源码）
    let run_group = if owner.is_some() { run } else { "www" };
    let home_p = PathBuf::from(home);
    std::fs::create_dir_all(&home_p).map_err(|e| format!("创建家目录失败 {home_p:?}: {e}"))?;
    let mut created: Vec<String> = Vec::new();
    for sub in ["www", "logs", "tmp"] {
        let d = home_p.join(sub);
        std::fs::create_dir_all(&d).map_err(|e| format!("创建子目录失败 {d:?}: {e}"))?;
        created.push(d.to_string_lossy().to_string());
    }
    let q = |p: &str| sh_quote(p);

    // web tree（www）：递归归 {run}:www；组保持 www（nginx worker 经组位读取静态文件）
    run_bash(&format!(
        "chown -R {}:www {}",
        q(run),
        q(&format!("{home}/www"))
    ))?;
    run_bash(&format!("chmod 750 {}", q(&format!("{home}/www"))))?;
    // log tree（logs）：nginx 写 access/error.log，恒归 www:www，组可写
    run_bash(&format!("chown -R www:www {}", q(&format!("{home}/logs"))))?;
    run_bash(&format!("chmod 770 {}", q(&format!("{home}/logs"))))?;
    // session/upload 临时目录：进程身份独占
    run_bash(&format!(
        "chown -R {}:{} {}",
        q(run),
        q(run_group),
        q(&format!("{home}/tmp"))
    ))?;
    run_bash(&format!("chmod 700 {}", q(&format!("{home}/tmp"))))?;
    // 家目录顶层：system 模式独立归属 + o+x（nginx 可进入但不可列），www 模式归 www
    if owner.is_some() {
        run_bash(&format!("chown {}:{} {}", q(run), q(run_group), q(home)))?;
        run_bash(&format!("chmod 711 {}", q(home)))?;
    } else {
        run_bash(&format!("chown www:www {}", q(home)))?;
        run_bash(&format!("chmod 750 {}", q(home)))?;
    }

    let mode = if owner.is_some() { "system" } else { "www" };
    Ok(Response::ok(
        format!("家目录已就绪：{home}（运行模式 {mode}）"),
        Some(json!({ "home_dir": home, "dirs": created, "mode": mode })),
    ))
}

// ── user.system_init（创建 Linux 账号，幂等）──────────────────

pub async fn system_init(linux_user: &str, home_dir: &str) -> Response {
    let linux_user = linux_user.to_string();
    let home_dir = home_dir.to_string();
    tokio::task::spawn_blocking(move || system_init_inner(&linux_user, &home_dir))
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

fn system_init_inner(linux_user: &str, home_dir: &str) -> Result<Response, String> {
    if !linux_user_ok(linux_user) {
        return Err(format!("非法的 Linux 账号名: {linux_user}"));
    }
    if !home_dir_ok(home_dir) {
        return Err(format!(
            "home_dir 非法（必须为挂载点下的绝对路径，不含 ..）: {home_dir}"
        ));
    }
    let id = root_cmd("id")
        .args(["-u", linux_user])
        .output()
        .map_err(|e| format!("执行 id 失败: {e}"))?;
    if id.status.success() {
        return Ok(Response::ok(
            format!("Linux 账号 {linux_user} 已存在（跳过创建）"),
            None,
        ));
    }
    // nologin shell（Debian/Ubuntu 通常在 /usr/sbin/nologin）
    let shell = root_cmd("bash")
        .args([
            "-c",
            "command -v nologin 2>/dev/null || echo /usr/sbin/nologin",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/sbin/nologin".to_string());
    // -M：不自动创建家目录（目录由 user.home_init 建好并赋权）
    let o = root_cmd("useradd")
        .args(["-M", "-s", &shell, "-d", home_dir, linux_user])
        .output()
        .map_err(|e| format!("执行 useradd 失败: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "创建 Linux 账号失败：{}",
            cmd_err(&o, "useradd 返回非零")
        ));
    }
    Ok(Response::ok(
        format!("Linux 账号 {linux_user} 已创建（home={home_dir}）"),
        Some(json!({ "linux_user": linux_user, "home_dir": home_dir, "shell": shell })),
    ))
}

// ── user.system_remove（移除 Linux 账号，幂等）────────────────

pub async fn system_remove(linux_user: &str) -> Response {
    // 先清该用户在所有 PHP 实例中的 pool 并 reload（否则删账号后 fpm -t 报错）
    let clean = super::php::pool_clean(linux_user.to_string()).await;
    if clean.code != 0 {
        return clean;
    }
    let linux_user = linux_user.to_string();
    tokio::task::spawn_blocking(move || system_remove_inner(&linux_user))
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

fn system_remove_inner(linux_user: &str) -> Result<Response, String> {
    if !linux_user_ok(linux_user) {
        return Err(format!("非法的 Linux 账号名: {linux_user}"));
    }
    let id = root_cmd("id")
        .args(["-u", linux_user])
        .output()
        .map_err(|e| format!("执行 id 失败: {e}"))?;
    if !id.status.success() {
        return Ok(Response::ok(
            format!("Linux 账号 {linux_user} 不存在（跳过）"),
            None,
        ));
    }
    let o = root_cmd("userdel")
        .arg(linux_user)
        .output()
        .map_err(|e| format!("执行 userdel 失败: {e}"))?;
    if !o.status.success() {
        return Err(format!(
            "移除 Linux 账号失败：{}",
            cmd_err(&o, "userdel 返回非零")
        ));
    }
    Ok(Response::ok(
        format!("Linux 账号 {linux_user} 已移除"),
        None,
    ))
}

// ── user.home_migrate（家目录跨挂载点迁移）──────────────────

pub async fn migrate_home(src_home: &str, dest_home: &str, owner: Option<&str>) -> Response {
    let src_home = src_home.to_string();
    let dest_home = dest_home.to_string();
    let owner = owner.map(|s| s.to_string());
    tokio::task::spawn_blocking(move || migrate_home_inner(&src_home, &dest_home, owner.as_deref()))
        .await
        .unwrap_or_else(|e| Ok(Response::err(-1, format!("任务执行失败: {e}"))))
        .unwrap_or_else(|e| Response::err(-1, e))
}

fn migrate_home_inner(
    src_home: &str,
    dest_home: &str,
    owner: Option<&str>,
) -> Result<Response, String> {
    let src = src_home.trim().trim_end_matches('/').to_string();
    let dest = dest_home.trim().trim_end_matches('/').to_string();
    for (name, p) in [("源家目录", src.as_str()), ("目标家目录", dest.as_str())] {
        if p.is_empty() || !home_dir_ok(p) {
            return Err(format!("{name}非法（必须为挂载点下的绝对路径，不含 ..）: {p}"));
        }
    }
    if src == dest {
        return Err("源与目标家目录相同，无需迁移".to_string());
    }
    let src_name = src.rsplit('/').next().unwrap_or("");
    let dest_name = dest.rsplit('/').next().unwrap_or("");
    if src_name != dest_name {
        return Err(format!(
            "目标家目录名称必须与源一致（源为 {src_name}）: {dest_name}"
        ));
    }

    let q = |p: &str| sh_quote(p);
    let src_p = Path::new(&src);
    let dest_p = Path::new(&dest);
    if !src_p.exists() {
        return Err(format!("源家目录不存在: {src}"));
    }
    if dest_p.exists() {
        // 目标已存在：仅允许空目录（数据不能覆盖），清空后继续
        let entries = dest_p
            .read_dir()
            .map_err(|e| format!("读取目标目录失败 {dest}: {e}"))?
            .next()
            .transpose()
            .map_err(|e| format!("读取目标目录失败 {dest}: {e}"))?;
        if entries.is_some() {
            return Err(format!("目标家目录已存在且非空，拒绝覆盖: {dest}"));
        }
        run_bash(&format!("rmdir {}", q(&dest)))?;
    }
    // 目标挂载点目录须已存在（挂载检查由上层/系统负责），仅创建到目标家目录
    if let Some(parent) = dest_p.parent()
        && !parent.exists()
    {
        run_bash(&format!("mkdir -p {}", q(&parent.to_string_lossy())))?;
    }

    // 优先 mv（同文件系统瞬时完成）；跨文件系统（EXDEV）自动回退 cp -a 后删除源。
    // 回退场景下属主会变为 root，随后统一由 home_init 重置归属。
    run_bash(&format!(
        "mv {} {} 2>/dev/null || (mkdir -p {} && cp -a {}/. {}/ && rm -rf {})",
        q(&src),
        q(&dest),
        q(&dest),
        q(&src),
        q(&dest),
        q(&src),
    ))?;

    // 按当前运行模式重置家目录骨架属主/权限（幂等），并补全 www/logs/tmp 子目录。
    // 文件搬移已成功，权限重置失败仅追加提示、不阻断迁移结果（避免数据与记录状态不一致）。
    let mut warn_msg = String::new();
    if let Err(e) = home_init_inner(&dest, owner) {
        warn_msg = format!("（权限重置提示：{e}）");
    }

    // system 模式：同步 Linux 系统账号家目录指针（账号可能不存在，失败不阻断）
    if let Some(u) = owner {
        let _ = run_bash(&format!(
            "usermod -d {} {} 2>/dev/null || true",
            q(&dest),
            q(u)
        ));
    }

    Ok(Response::ok(
        format!("数据迁移完成：{src} → {dest}{warn_msg}"),
        Some(json!({ "src_home": src, "dest_home": dest })),
    ))
}
