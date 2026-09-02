//! 文件管理器 verb。
//!
//! zapd 已完成授权（admin 全量 / 普通用户路径白名单），这里只做路径 sanitize
//! （防 `..` 穿越）、关键路径保护（禁删 `/`、`/etc`、`/root`、`/boot`）以及
//! 以 root 权限执行实际文件操作。二进制内容（download/upload）用 base64 传输。

use std::path::{Path, PathBuf};
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
    permissions: String,
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
    let permissions = if metadata.is_dir() {
        "drwxr-xr-x".to_string()
    } else {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", metadata.permissions().mode())
    };
    Some(FileInfo {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string()),
        path: path.to_string_lossy().to_string(),
        is_dir: metadata.is_dir(),
        size: metadata.len(),
        modified,
        permissions,
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
        if let Ok(md) = std::fs::metadata(&resolved) {
            if md.is_dir() {
                return Response::err(-1, "不能覆盖目录");
            }
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
