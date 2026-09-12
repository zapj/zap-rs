//! 文件管理器 verb。
//!
//! zapd 已完成授权（admin 全量 / 普通用户路径白名单），这里只做路径 sanitize
//! （防 `..` 穿越）、关键路径保护（禁删 `/`、`/etc`、`/root`、`/boot`）以及
//! 以 root 权限执行实际文件操作。二进制内容（download/upload）用 base64 传输。

use std::collections::HashMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use serde_json::json;

use zap_proto::{Response, b64_decode, b64_encode};

#[derive(serde::Serialize)]
struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: String,
    /// 权限文本：八进制 4 位（如 `0755`，含 setuid/setgid/sticky 时为 `4755`）
    permissions: String,
    /// 权限原始数值（仅低 12 位），供前端「修改权限」对话框回填
    mode: u32,
    owner: String,
    group: String,
}

/// 解析 /etc/passwd 或 /etc/group 为 id → 名称映射。
/// 以 `#` 开头的行（uid/gid 溢出 65535 时的 NSS 保留行）忽略。
fn id_name_map(file: &str, name_idx: usize, id_idx: usize) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(file) {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split(':').collect();
            let (Some(name), Some(id_str)) = (parts.get(name_idx), parts.get(id_idx)) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if let Ok(id) = id_str.parse::<u32>() {
                map.entry(id).or_insert_with(|| name.to_string());
            }
        }
    }
    map
}

/// id → 名称缓存，带「文件指纹」（mtime + 长度）校验。
/// 常驻进程里 `/etc/passwd`、`/etc/group` 会随新建用户/组而变化，
/// 指纹变了就重新加载，避免新用户一直显示为数字 uid/gid。
struct IdNameCache {
    file: &'static str,
    name_idx: usize,
    id_idx: usize,
    fingerprint: Option<(u64, u32, u64)>,
    map: HashMap<u32, String>,
}

impl IdNameCache {
    fn new(file: &'static str, name_idx: usize, id_idx: usize) -> Self {
        Self {
            file,
            name_idx,
            id_idx,
            fingerprint: None,
            map: HashMap::new(),
        }
    }

    /// 文件指纹：mtime（秒 + 纳秒）+ 文件长度
    fn current_fingerprint(&self) -> Option<(u64, u32, u64)> {
        let metadata = std::fs::metadata(self.file).ok()?;
        let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some((modified.as_secs(), modified.subsec_nanos(), metadata.len()))
    }

    /// 查 id → 名称；文件有变化时先重载映射。
    fn get(&mut self, id: u32) -> Option<String> {
        let fingerprint = self.current_fingerprint();
        if self.fingerprint != fingerprint {
            self.map = id_name_map(self.file, self.name_idx, self.id_idx);
            self.fingerprint = fingerprint;
        }
        self.map.get(&id).cloned()
    }
}

fn passwd_cache() -> &'static Mutex<IdNameCache> {
    static CACHE: OnceLock<Mutex<IdNameCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(IdNameCache::new("/etc/passwd", 0, 2)))
}

fn group_cache() -> &'static Mutex<IdNameCache> {
    static CACHE: OnceLock<Mutex<IdNameCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(IdNameCache::new("/etc/group", 0, 2)))
}

/// 查 id → 名称：查不到（或缓存中毒）时回退数字 id
fn resolve_id(cache: &'static Mutex<IdNameCache>, id: u32) -> String {
    cache
        .lock()
        .ok()
        .and_then(|mut cache| cache.get(id))
        .unwrap_or_else(|| id.to_string())
}

/// uid → 用户名（查不到时回退数字 uid）
fn owner_name(uid: u32) -> String {
    resolve_id(passwd_cache(), uid)
}

/// gid → 组名（查不到时回退数字 gid）
fn group_name(gid: u32) -> String {
    resolve_id(group_cache(), gid)
}

/// 渲染权限为八进制 4 位文本：`0755` / `0644`，含 setuid/setgid/sticky（`4755` / `1777`）。
/// 与面板「权限」列展示、「修改权限」对话框回填保持一致（cPanel 风格）。
fn mode_text(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}

fn resolve_path(requested: &str) -> PathBuf {
    let clean = requested
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != "..")
        .collect::<Vec<_>>()
        .join("/");
    let resolved = PathBuf::from("/").join(&clean);
    match resolved.canonicalize() {
        Ok(c) => c,
        Err(_) => resolved,
    }
}

fn file_info(path: &Path) -> Option<FileInfo> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let mode = metadata.permissions().mode() & 0o7777;
    Some(FileInfo {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified,
        permissions: mode_text(mode),
        mode,
        owner: owner_name(metadata.uid()),
        group: group_name(metadata.gid()),
    })
}

fn is_critical_path(path: &Path) -> bool {
    matches!(
        path.to_string_lossy().as_ref(),
        "/" | "/etc" | "/root" | "/boot"
    )
}

// ── 动词实现 ───────────────────────────────────────────────

pub async fn list(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        let md = match std::fs::metadata(&resolved) {
            Ok(m) => m,
            Err(e) => return Response::err(-1, format!("路径不存在: {e}")),
        };
        if !md.is_dir() {
            return Response::err(-1, "路径不是目录");
        }
        let mut entries: Vec<FileInfo> = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&resolved) {
            for entry in rd.flatten() {
                if let Some(info) = file_info(&entry.path()) {
                    entries.push(info);
                }
            }
        }
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Response::ok(
            "ok",
            Some(json!({
                "current_path": resolved.to_string_lossy(),
                "parent_path": resolved
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "/".to_string()),
                "entries": entries,
            })),
        )
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn read(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        let md = match std::fs::metadata(&resolved) {
            Ok(m) => m,
            Err(e) => return Response::err(-1, format!("路径不存在: {e}")),
        };
        if md.is_dir() {
            return Response::err(-1, "不能读取目录");
        }
        match std::fs::read_to_string(&resolved) {
            Ok(content) => Response::ok(
                "ok",
                Some(json!({
                    "path": resolved.to_string_lossy(),
                    "content": content,
                    "size": content.len(),
                })),
            ),
            Err(e) => Response::err(-1, format!("读取文件失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn write(path: String, content: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        if let Ok(md) = std::fs::metadata(&resolved)
            && md.is_dir()
        {
            return Response::err(-1, "不能覆盖目录");
        }
        if let Some(parent) = resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&resolved, &content) {
            Ok(_) => Response::ok(
                "保存成功",
                Some(json!({ "path": resolved.to_string_lossy() })),
            ),
            Err(e) => Response::err(-1, format!("写入失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn delete(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        if is_critical_path(&resolved) {
            return Response::err(-1, "不能删除系统关键目录");
        }
        let md = match std::fs::metadata(&resolved) {
            Ok(m) => m,
            Err(e) => return Response::err(-1, format!("路径不存在: {e}")),
        };
        let result = if md.is_dir() {
            std::fs::remove_dir_all(&resolved)
        } else {
            std::fs::remove_file(&resolved)
        };
        match result {
            Ok(_) => Response::ok("删除成功", None),
            Err(e) => Response::err(-1, format!("删除失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn mkdir(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        if resolved.exists() {
            return Response::err(-1, "目录已存在");
        }
        match std::fs::create_dir_all(&resolved) {
            Ok(_) => Response::ok(
                "创建成功",
                Some(json!({ "path": resolved.to_string_lossy() })),
            ),
            Err(e) => Response::err(-1, format!("创建失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn rename(path: String, new_path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let old_path = resolve_path(&path);
        let new_path = resolve_path(&new_path);
        if !old_path.exists() {
            return Response::err(-1, "源文件不存在");
        }
        if new_path.exists() {
            return Response::err(-1, "目标已存在");
        }
        match std::fs::rename(&old_path, &new_path) {
            Ok(_) => Response::ok(
                "重命名成功",
                Some(json!({
                    "old_path": old_path.to_string_lossy(),
                    "new_path": new_path.to_string_lossy(),
                })),
            ),
            Err(e) => Response::err(-1, format!("重命名失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn download(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        let md = match std::fs::metadata(&resolved) {
            Ok(m) => m,
            Err(e) => return Response::err(-1, format!("路径不存在: {e}")),
        };
        if md.is_dir() {
            return Response::err(-1, "不能下载目录");
        }
        let file_name = resolved
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "download".to_string());
        match std::fs::read(&resolved) {
            Ok(bytes) => Response::ok(
                "ok",
                Some(json!({
                    "name": file_name,
                    "content": b64_encode(&bytes),
                })),
            ),
            Err(e) => Response::err(-1, format!("读取文件失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn upload(path: String, name: String, content: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let dir = resolve_path(&path);
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        let md = match std::fs::metadata(&dir) {
            Ok(m) => m,
            Err(e) => return Response::err(-1, format!("目标路径不存在: {e}")),
        };
        if !md.is_dir() {
            return Response::err(-1, "目标路径不是目录");
        }
        let safe_name = Path::new(&name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        let bytes = match b64_decode(&content) {
            Ok(b) => b,
            Err(e) => return Response::err(-1, format!("内容解码失败: {e}")),
        };
        let dest = dir.join(&safe_name);
        match std::fs::write(&dest, &bytes) {
            Ok(_) => Response::ok("上传成功", Some(json!({ "name": safe_name }))),
            Err(e) => Response::err(-1, format!("上传失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

pub async fn info(path: String) -> Response {
    tokio::task::spawn_blocking(move || {
        let resolved = resolve_path(&path);
        match file_info(&resolved) {
            Some(info) => Response::ok("ok", Some(json!(info))),
            None => Response::err(-1, "文件不存在"),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

/// 修改文件/目录权限（八进制，仅低 12 位，含 setuid/setgid/sticky）。
pub async fn chmod(path: String, mode: u32) -> Response {
    tokio::task::spawn_blocking(move || {
        if mode & !0o7777 != 0 {
            return Response::err(-1, "权限值非法：仅支持 0-7777（八进制）");
        }
        let resolved = resolve_path(&path);
        if is_critical_path(&resolved) {
            return Response::err(-1, "不能修改系统关键目录的权限");
        }
        if !resolved.exists() {
            return Response::err(-1, "路径不存在");
        }
        match std::fs::set_permissions(&resolved, std::fs::Permissions::from_mode(mode)) {
            Ok(_) => match file_info(&resolved) {
                Some(info) => Response::ok("权限修改成功", Some(json!(info))),
                None => Response::ok(
                    "权限修改成功",
                    Some(json!({
                        "path": resolved.to_string_lossy(),
                        "permissions": mode_text(mode),
                        "mode": mode,
                    })),
                ),
            },
            Err(e) => Response::err(-1, format!("修改权限失败: {e}")),
        }
    })
    .await
    .unwrap_or_else(|e| Response::err(-1, format!("任务执行失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 缓存必须能感知文件变化：常驻进程里新建用户后应立刻显示用户名，
    /// 而不是一直回退成数字 uid。
    #[test]
    fn id_name_cache_reloads_after_file_change() {
        let file = "/tmp/zap_id_name_cache_test";
        std::fs::write(file, "alice:x:1000:1000::/home/alice:/bin/bash\n").unwrap();

        let mut cache = IdNameCache::new(file, 0, 2);
        assert_eq!(cache.get(1000).as_deref(), Some("alice"));
        assert_eq!(cache.get(1001), None);

        // 模拟新建用户（追加一行：长度与 mtime 都变化）
        std::fs::write(
            file,
            "alice:x:1000:1000::/home/alice:/bin/bash\nbob:x:1001:1001::/home/bob:/bin/bash\n",
        )
        .unwrap();

        assert_eq!(cache.get(1001).as_deref(), Some("bob"));
        let _ = std::fs::remove_file(file);
    }
}
