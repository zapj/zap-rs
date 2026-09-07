//! 站点配置（Nginx / Apache）的目录布局与发布（root 执行）。
//!
//! 三层职责（与面板确认的规划一致）：
//!
//! 1. 面板数据区（zapd / zapadm 可写，仅作渲染源与历史，不被引擎 include）
//!
//! ```text
//! {ZAP_PATH}/data/sites/<site_id>/
//!   ├── site.json     渲染入参快照（域名 / 根目录 / php socket / 启用状态）
//!   ├── nginx.conf    最近一次渲染结果
//!   ├── apache.conf
//!   └── backup/       最近 N 次通过校验的版本（回滚用）
//! ```
//!
//! 2. 生效配置（webserver 实际 include；只有 zapexec / root 能改）
//!
//! ```text
//! /etc/zap/webservers/<kind>/sites-available/zap-site-<id>.conf
//! /etc/zap/webservers/<kind>/sites-enabled/zap-site-<id>.conf → 软链到 available
//! ```
//!
//! 与 webserver 的安装位置（/usr/local/apps/...）解耦：
//! 卸载 / 换版本 / 换 ZAP_APPS_DIR 都不会丢站点配置，也不会残留旧文件。
//!
//! 3. 主配置一次性注入 include（幂等，改动前备份 `*.zap.bak`）
//!
//! ```text
//! nginx.conf: include /etc/zap/webservers/nginx/sites-enabled/*.conf;
//! httpd.conf: Include /etc/zap/webservers/apache/sites-enabled/*.conf
//! ```
//!
//! 权限：目录 root:zapadm 0750、文件 root:zapadm 0640
//! （nginx / apache 主进程是 root，可读；zapd 以 zapadm 组可读；其它用户无权限）

use std::path::{Path, PathBuf};

/// 面板托管的站点配置根（不属于任何具体 webserver 安装目录）。
/// 环境变量 `ZAP_WEBSERVER_ROOT` 仅用于开发联调与单测，生产不要设置。
const WEBSERVER_ROOT: &str = "/etc/zap/webservers";

fn root() -> PathBuf {
    std::env::var("ZAP_WEBSERVER_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(WEBSERVER_ROOT))
}

/// 面板数据区站点目录：`{ZAP_PATH}/data/sites/<site_id>`
fn site_data_dir(site_id: i64) -> PathBuf {
    super::site::zap_path()
        .join("data/sites")
        .join(site_id.to_string())
}

/// zapd 运行用户（数据区文件归属它；生效配置文件的属组也是它）。
const ZAP_USER: &str = "zapadm";

pub(super) fn vhost_name(site_id: i64) -> String {
    format!("zap-site-{site_id}.conf")
}

pub(super) fn available_dir(kind: &str) -> PathBuf {
    root().join(kind).join("sites-available")
}

pub(super) fn enabled_dir(kind: &str) -> PathBuf {
    root().join(kind).join("sites-enabled")
}

/// 创建 `<kind>` 的 sites-available / sites-enabled，并收敛为 root:zapadm 0750。
pub(super) fn ensure_dirs(kind: &str) -> Result<(PathBuf, PathBuf), String> {
    let adir = available_dir(kind);
    let edir = enabled_dir(kind);
    let root_dir = root();
    let kind_dir = root_dir.join(kind);
    std::fs::create_dir_all(&adir).map_err(|e| format!("创建 {} 失败: {e}", adir.display()))?;
    std::fs::create_dir_all(&edir).map_err(|e| format!("创建 {} 失败: {e}", edir.display()))?;
    for d in [&root_dir, &kind_dir, &adir, &edir] {
        set_owner(d, 0o750, true);
    }
    Ok((adir, edir))
}

pub(super) fn available_path(kind: &str, site_id: i64) -> PathBuf {
    available_dir(kind).join(vhost_name(site_id))
}

pub(super) fn enabled_path(kind: &str, site_id: i64) -> PathBuf {
    enabled_dir(kind).join(vhost_name(site_id))
}

/// 写入面板侧快照（渲染结果 + 入参 json），归属 zapadm:zapadm 0640。
/// 失败不影响发布，仅告警级别：快照只是排障 / 回滚用的副本。
pub(super) fn write_snapshot(
    site_id: i64,
    kind: &str,
    content: &str,
    meta: &serde_json::Value,
) -> Result<PathBuf, String> {
    let dir = site_data_dir(site_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建站点数据目录失败: {e}"))?;
    set_owner(&dir, 0o750, false);

    let conf = dir.join(format!("{kind}.conf"));
    let tmp = dir.join(format!("{kind}.conf.tmp"));
    std::fs::write(&tmp, content).map_err(|e| format!("写入站点配置快照失败: {e}"))?;
    std::fs::rename(&tmp, &conf).map_err(|e| format!("写入站点配置快照失败: {e}"))?;
    set_owner(&conf, 0o640, false);

    let meta_path = dir.join("site.json");
    if let Ok(text) = serde_json::to_string_pretty(meta) {
        let _ = std::fs::write(&meta_path, text);
        set_owner(&meta_path, 0o640, false);
    }
    Ok(conf)
}

/// 把已通过校验的版本留一份到 backup/（最多保留 10 份，按时间倒序）。
pub(super) fn backup_snapshot(site_id: i64, kind: &str, content: &str) {
    let dir = site_data_dir(site_id).join("backup");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    set_owner(&dir, 0o750, false);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file = dir.join(format!("{kind}.{ts}.conf"));
    if std::fs::write(&file, content).is_ok() {
        set_owner(&file, 0o640, false);
        prune_backup(&dir, kind, 10);
    }
}

fn prune_backup(dir: &Path, kind: &str, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(kind))
        })
        .collect();
    if files.len() <= keep {
        return;
    }
    files.sort();
    let drop = files.len() - keep;
    for f in files.into_iter().take(drop) {
        let _ = std::fs::remove_file(f);
    }
}

/// 发布站点配置：写入 sites-available 并在 sites-enabled 建软链（原子替换）。
pub(super) fn publish(kind: &str, site_id: i64, content: &str) -> Result<PathBuf, String> {
    let (adir, edir) = ensure_dirs(kind)?;
    let avail = adir.join(vhost_name(site_id));
    let tmp = adir.join(format!("{}.tmp", vhost_name(site_id)));

    std::fs::write(&tmp, content).map_err(|e| format!("写入站点配置失败: {e}"))?;
    set_owner(&tmp, 0o640, true);
    std::fs::rename(&tmp, &avail).map_err(|e| format!("发布站点配置失败: {e}"))?;

    let link = edir.join(vhost_name(site_id));
    if !link.is_symlink() && link.exists() {
        // 历史遗留的实体文件（非软链）：删除后重建软链，保证启用/停用语义一致
        let _ = std::fs::remove_file(&link);
    }
    if !link.is_symlink() {
        std::os::unix::fs::symlink(&avail, &link)
            .map_err(|e| format!("创建站点启用软链失败: {e}"))?;
    }
    Ok(avail)
}

/// 停用：仅移除 sites-enabled 软链，配置保留在 sites-available。
pub(super) fn unpublish(kind: &str, site_id: i64) -> Result<bool, String> {
    let link = enabled_path(kind, site_id);
    if !link.is_symlink() && !link.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&link).map_err(|e| format!("移除站点启用软链失败: {e}"))?;
    Ok(true)
}

/// 站点删除：available 与 enabled 一并清理。
pub(super) fn purge(kind: &str, site_id: i64) -> Result<bool, String> {
    let mut removed = unpublish(kind, site_id)?;
    let avail = available_path(kind, site_id);
    if avail.exists() {
        std::fs::remove_file(&avail).map_err(|e| format!("移除站点配置失败: {e}"))?;
        removed = true;
    }
    Ok(removed)
}

/// 旧布局迁移：`<webserver prefix>/conf/sites-enabled/zap-site-<id>.conf`
/// → `sites-available/`。跨设备时用复制 + 删除，成功返回 true。
pub(super) fn migrate_legacy(legacy_dir: &Path, kind: &str, site_id: i64) -> Result<bool, String> {
    let old = legacy_dir.join(vhost_name(site_id));
    if !old.exists() {
        return Ok(false);
    }
    let (adir, _) = ensure_dirs(kind)?;
    let avail = adir.join(vhost_name(site_id));
    if !avail.exists() {
        if std::fs::rename(&old, &avail).is_err() {
            std::fs::copy(&old, &avail).map_err(|e| format!("迁移旧站点配置失败: {e}"))?;
            let _ = std::fs::remove_file(&old);
        }
        set_owner(&avail, 0o640, true);
    } else {
        let _ = std::fs::remove_file(&old);
    }
    Ok(true)
}

/// 确保 webserver 主配置 include 了面板的 sites-enabled 目录（幂等）。
/// 返回 true 表示本次修改了主配置（调用方需在校验失败时回滚）。
pub(super) fn ensure_include(conf_file: &Path, dir: &Path, kind: &str) -> Result<bool, String> {
    let text = std::fs::read_to_string(conf_file).map_err(|e| format!("读取主配置失败: {e}"))?;
    let needle = dir.to_string_lossy().to_string();
    if text.contains(&needle) {
        return Ok(false);
    }
    let line = if kind == "apache" {
        format!("Include {needle}/*.conf")
    } else {
        format!("include {needle}/*.conf;")
    };
    let idx = text
        .rfind('}')
        .ok_or_else(|| "主配置缺少结束符 '}'，无法自动注入 include".to_string())?;

    let bak = backup_path(conf_file);
    if !bak.exists() {
        let _ = std::fs::copy(conf_file, &bak);
    }
    let mut out = String::with_capacity(text.len() + line.len() + 64);
    out.push_str(&text[..idx]);
    out.push_str("\n    # zap: 面板托管站点配置（由 zapexec 自动维护，勿手工修改本行）\n    ");
    out.push_str(&line);
    out.push('\n');
    out.push_str(&text[idx..]);
    std::fs::write(conf_file, out).map_err(|e| format!("写入主配置失败: {e}"))?;
    Ok(true)
}

/// include 注入后校验失败时回滚主配置。
pub(super) fn restore_include(conf_file: &Path) -> bool {
    let bak = backup_path(conf_file);
    if !bak.exists() {
        return false;
    }
    std::fs::copy(&bak, conf_file).is_ok()
}

fn backup_path(conf_file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.zap.bak", conf_file.display()))
}

/// 设置属主 / 权限：
/// - `root_group = true` → root:zapadm（生效配置：root 可写，zapd 组可读）
/// - `root_group = false` → zapadm:zapadm（面板数据区：zapd 可读写）
fn set_owner(path: &Path, mode: u32, root_group: bool) {
    use std::os::unix::fs::PermissionsExt;

    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    let (uid, gid) = match ids_of(ZAP_USER) {
        Some((u, g)) => (u, g),
        None => return, // zapadm 不存在（开发机）：只收敛权限
    };
    let owner_uid = if root_group { 0 } else { uid };
    let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return;
    };
    unsafe {
        libc::chown(cpath.as_ptr(), owner_uid, gid);
    }
}

/// 查询用户的 uid / gid（失败返回 None）。
fn ids_of(name: &str) -> Option<(libc::uid_t, libc::gid_t)> {
    let cname = std::ffi::CString::new(name).ok()?;
    unsafe {
        let pw = libc::getpwnam(cname.as_ptr());
        if pw.is_null() {
            return None;
        }
        Some(((*pw).pw_uid, (*pw).pw_gid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 环境变量是进程级的，用例并发跑会互相干扰：用锁串行化。
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 把目录根与 ZAP_PATH 指到临时目录，避免污染真实的 /etc/zap。
    struct Tmp {
        dir: PathBuf,
        _guard: std::sync::MutexGuard<'static, ()>,
    }

    impl Tmp {
        fn new(tag: &str) -> Self {
            let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let dir =
                std::env::temp_dir().join(format!("zap-webconf-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            // SAFETY: 持锁期间独占修改环境变量，这些用例也不会再 spawn 线程
            unsafe {
                std::env::set_var("ZAP_WEBSERVER_ROOT", &dir);
                std::env::set_var("ZAP_PATH", &dir);
            }
            Self { dir, _guard: guard }
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            // SAFETY: 同上，持锁独占
            unsafe {
                std::env::remove_var("ZAP_WEBSERVER_ROOT");
                std::env::remove_var("ZAP_PATH");
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn publish_then_unpublish_keeps_available() {
        let _t = Tmp::new("pub");
        let avail = publish("nginx", 7, "server {}\n").unwrap();
        assert!(avail.ends_with("sites-available/zap-site-7.conf"));
        assert_eq!(std::fs::read_to_string(&avail).unwrap(), "server {}\n");

        let link = enabled_path("nginx", 7);
        assert!(link.is_symlink(), "启用目录应是软链");
        assert_eq!(std::fs::read_to_string(&link).unwrap(), "server {}\n");

        assert!(unpublish("nginx", 7).unwrap());
        assert!(!link.is_symlink(), "停用后软链应移除");
        assert!(avail.exists(), "停用后配置应保留在 sites-available");

        assert!(purge("nginx", 7).unwrap());
        assert!(!avail.exists());
    }

    #[test]
    fn snapshot_written_into_panel_data_dir() {
        let _t = Tmp::new("snap");
        let meta = serde_json::json!({ "site_id": 9, "domains": ["a.com"] });
        let conf = write_snapshot(9, "nginx", "server {}\n", &meta).unwrap();
        assert!(conf.ends_with("data/sites/9/nginx.conf"));
        assert_eq!(std::fs::read_to_string(&conf).unwrap(), "server {}\n");
        assert!(conf.with_file_name("site.json").exists());
        backup_snapshot(9, "nginx", "server {}\n");
        assert!(site_data_dir(9).join("backup").exists());
    }

    #[test]
    fn ensure_include_is_idempotent_and_rollbackable() {
        let _t = Tmp::new("inc");
        let conf = std::env::temp_dir().join(format!("nginx-{}.conf", std::process::id()));
        std::fs::write(&conf, "http {\n    include mime.types;\n}\n").unwrap();
        let edir = enabled_dir("nginx");

        assert!(ensure_include(&conf, &edir, "nginx").unwrap());
        let text = std::fs::read_to_string(&conf).unwrap();
        assert!(text.contains(&format!("include {}/*.conf;", edir.display())));
        assert!(
            text.trim_end().ends_with('}'),
            "include 必须插在最后一个右花括号之前"
        );
        assert!(backup_path(&conf).exists(), "改主配置前必须备份");

        // 幂等：已包含时不再改动
        assert!(!ensure_include(&conf, &edir, "nginx").unwrap());
        // 回滚：恢复备份后不再包含
        assert!(restore_include(&conf));
        assert!(
            !std::fs::read_to_string(&conf)
                .unwrap()
                .contains("zap/webservers")
        );
        let _ = std::fs::remove_file(&conf);
        let _ = std::fs::remove_file(backup_path(&conf));
    }

    #[test]
    fn migrate_legacy_moves_old_vhost_file() {
        let _t = Tmp::new("mig");
        let legacy = std::env::temp_dir().join(format!("legacy-{}", std::process::id()));
        std::fs::create_dir_all(&legacy).unwrap();
        let old = legacy.join("zap-site-3.conf");
        std::fs::write(&old, "legacy\n").unwrap();

        assert!(migrate_legacy(&legacy, "nginx", 3).unwrap());
        let avail = available_path("nginx", 3);
        assert_eq!(std::fs::read_to_string(&avail).unwrap(), "legacy\n");
        assert!(!old.exists(), "迁移后旧文件应删除");
        let _ = std::fs::remove_dir_all(&legacy);
    }
}
