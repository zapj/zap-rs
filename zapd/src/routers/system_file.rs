use axum::{
    body::Body,
    extract::{Multipart, Query},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::zap::{
    jwt::{is_admin, Claims},
    ZapError, ZapJsonResult,
};

// ── request types ──────────────────────────────────────────

#[derive(Deserialize)]
pub struct PathQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
pub struct FileOpPayload {
    path: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    new_path: String,
}

#[derive(Deserialize)]
pub struct MkdirPayload {
    path: String,
}

#[derive(Deserialize)]
pub struct DeletePayload {
    path: String,
}

// ── path helpers ───────────────────────────────────────────

/// Resolve and sanitize a path, preventing directory traversal.
fn resolve_path(requested: &str) -> Result<PathBuf, ZapError> {
    let clean = requested
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != "..")
        .collect::<Vec<_>>()
        .join("/");

    let resolved = PathBuf::from("/").join(&clean);

    // Canonicalize if the path exists, otherwise just normalize
    match resolved.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => {
            let mut normalized = PathBuf::from("/");
            for seg in clean.split('/').filter(|s| !s.is_empty()) {
                normalized.push(seg);
            }
            if normalized.starts_with("/") {
                Ok(normalized)
            } else {
                Err(ZapError::New(-1, "非法路径".to_string()))
            }
        }
    }
}

/// Check if user has read access to a path.
fn check_access(claims: &Claims, path: &Path) -> Result<(), ZapError> {
    if is_admin(claims) {
        return Ok(());
    }
    let path_str = path.to_string_lossy().to_string();
    let allowed: &[&str] = &["/home", "/tmp", "/var/www", "/var/log"];
    let readonly: &[&str] = &["/etc", "/usr", "/opt", "/srv"];
    for prefix in allowed.iter().chain(readonly) {
        if path_str.starts_with(prefix) {
            return Ok(());
        }
    }
    Err(ZapError::New(-1, "没有访问该路径的权限".to_string()))
}

/// Check if user can write to a path.
fn check_write_access(claims: &Claims, path: &Path) -> Result<(), ZapError> {
    if is_admin(claims) {
        return Ok(());
    }
    let path_str = path.to_string_lossy().to_string();
    let allowed: &[&str] = &["/home", "/tmp", "/var/www"];
    for prefix in allowed {
        if path_str.starts_with(prefix) {
            return Ok(());
        }
    }
    Err(ZapError::New(-1, "没有写入该路径的权限".to_string()))
}

// ── file info ──────────────────────────────────────────────

#[derive(serde::Serialize)]
struct FileInfo {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: String,
    permissions: String,
}

async fn get_file_info(path: &Path) -> Option<FileInfo> {
    let metadata = fs::metadata(path).await.ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| {
            let secs = d.as_secs();
            chrono::DateTime::from_timestamp(secs as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let perms = if metadata.is_dir() {
        "drwxr-xr-x".to_string()
    } else if cfg!(unix) {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", metadata.permissions().mode())
    } else {
        "-rw-r--r--".to_string()
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
        permissions: perms,
    })
}

// ── handlers ───────────────────────────────────────────────

/// GET /system/files/list?path=/
pub async fn file_list(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> ZapJsonResult {
    let raw_path = query.path.as_deref().unwrap_or("/");
    let resolved = resolve_path(raw_path)?;
    check_access(&claims, &resolved)?;

    let md = fs::metadata(&resolved).await?;
    if !md.is_dir() {
        return Err(ZapError::New(-1, "路径不是目录".to_string()));
    }

    let mut entries: Vec<FileInfo> = Vec::new();
    let mut read_dir = fs::read_dir(&resolved).await?;

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        if let Some(info) = get_file_info(&entry.path()).await {
            entries.push(info);
        }
    }

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "current_path": resolved.to_string_lossy(),
            "parent_path": resolved.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or("/".to_string()),
            "entries": entries
        }
    })))
}

/// GET /system/files/read?path=...
pub async fn file_read(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> Result<Response, ZapError> {
    let raw_path = query.path.as_deref().unwrap_or("");
    if raw_path.is_empty() {
        return Err(ZapError::New(-1, "缺少路径参数".to_string()));
    }
    let resolved = resolve_path(raw_path)?;
    check_access(&claims, &resolved)?;

    let md = fs::metadata(&resolved).await?;
    if md.is_dir() {
        return Err(ZapError::New(-1, "不能读取目录".to_string()));
    }

    let content = fs::read_to_string(&resolved).await.map_err(|e| {
        ZapError::New(-1, format!("读取文件失败: {}", e))
    })?;

    Ok(Json(json!({
        "code": 0,
        "message": "ok",
        "data": {
            "path": resolved.to_string_lossy(),
            "content": content,
            "size": content.len()
        }
    }))
    .into_response())
}

/// POST /system/files/write
pub async fn file_write(
    claims: Claims,
    Json(payload): Json<FileOpPayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    if let Ok(md) = fs::metadata(&resolved).await {
        if md.is_dir() {
            return Err(ZapError::New(-1, "不能覆盖目录".to_string()));
        }
    }

    if let Some(parent) = resolved.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::write(&resolved, &payload.content).await?;

    Ok(Json(json!({
        "code": 0,
        "message": "保存成功",
        "data": { "path": resolved.to_string_lossy() }
    })))
}

/// POST /system/files/delete
pub async fn file_delete(
    claims: Claims,
    Json(payload): Json<DeletePayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    // Safety: block critical paths
    let path_str = resolved.to_string_lossy().to_string();
    if path_str == "/" || path_str == "/etc" || path_str == "/root" || path_str == "/boot" {
        return Err(ZapError::New(-1, "不能删除系统关键目录".to_string()));
    }

    let md = fs::metadata(&resolved).await?;
    if md.is_dir() {
        fs::remove_dir_all(&resolved).await?;
    } else {
        fs::remove_file(&resolved).await?;
    }

    Ok(Json(json!({ "code": 0, "message": "删除成功" })))
}

/// POST /system/files/mkdir
pub async fn file_mkdir(
    claims: Claims,
    Json(payload): Json<MkdirPayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    if resolved.exists() {
        return Err(ZapError::New(-1, "目录已存在".to_string()));
    }

    fs::create_dir_all(&resolved).await?;

    Ok(Json(json!({
        "code": 0,
        "message": "创建成功",
        "data": { "path": resolved.to_string_lossy() }
    })))
}

/// POST /system/files/rename
pub async fn file_rename(
    claims: Claims,
    Json(payload): Json<FileOpPayload>,
) -> ZapJsonResult {
    let old_path = resolve_path(&payload.path)?;
    check_write_access(&claims, &old_path)?;

    if !old_path.exists() {
        return Err(ZapError::New(-1, "源文件不存在".to_string()));
    }

    let new_path = resolve_path(&payload.new_path)?;
    check_write_access(&claims, &new_path)?;

    if new_path.exists() {
        return Err(ZapError::New(-1, "目标已存在".to_string()));
    }

    fs::rename(&old_path, &new_path).await?;

    Ok(Json(json!({
        "code": 0,
        "message": "重命名成功",
        "data": {
            "old_path": old_path.to_string_lossy(),
            "new_path": new_path.to_string_lossy()
        }
    })))
}

/// GET /system/files/download?path=...
pub async fn file_download(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> Result<Response, ZapError> {
    let raw_path = query.path.as_deref().unwrap_or("");
    if raw_path.is_empty() {
        return Err(ZapError::New(-1, "缺少路径参数".to_string()));
    }
    let resolved = resolve_path(raw_path)?;
    check_access(&claims, &resolved)?;

    let md = fs::metadata(&resolved).await?;
    if md.is_dir() {
        return Err(ZapError::New(-1, "不能下载目录".to_string()));
    }

    let file_name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());

    let content = fs::read(&resolved).await.map_err(|e| {
        ZapError::New(-1, format!("读取文件失败: {}", e))
    })?;

    let mime = mime_guess::from_path(&resolved).first_or_octet_stream();

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(content))
        .unwrap())
}

/// POST /system/files/upload
pub async fn file_upload(
    claims: Claims,
    Query(query): Query<PathQuery>,
    mut multipart: Multipart,
) -> Result<Response, ZapError> {
    let target_dir = query.path.as_deref().unwrap_or("/tmp");
    let resolved_dir = resolve_path(target_dir)?;
    check_write_access(&claims, &resolved_dir)?;

    if !resolved_dir.exists() {
        fs::create_dir_all(&resolved_dir).await?;
    }

    let md = fs::metadata(&resolved_dir).await?;
    if !md.is_dir() {
        return Err(ZapError::New(-1, "目标路径不是目录".to_string()));
    }

    let mut uploaded: Vec<String> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("unnamed").to_string();
        let safe_name = Path::new(&file_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let dest = resolved_dir.join(&safe_name);
        let data = field.bytes().await.map_err(|e| {
            ZapError::New(-1, format!("上传失败: {}", e))
        })?;
        fs::write(&dest, &data).await?;
        uploaded.push(safe_name);
    }

    if uploaded.is_empty() {
        return Err(ZapError::New(-1, "没有上传文件".to_string()));
    }

    Ok(Json(json!({
        "code": 0,
        "message": "上传成功",
        "data": { "files": uploaded, "target_dir": resolved_dir.to_string_lossy() }
    }))
    .into_response())
}

/// GET /system/files/info?path=...
pub async fn file_info(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> ZapJsonResult {
    let raw_path = query.path.as_deref().unwrap_or("/");
    let resolved = resolve_path(raw_path)?;
    check_access(&claims, &resolved)?;

    match get_file_info(&resolved).await {
        Some(info) => Ok(Json(json!({ "code": 0, "message": "ok", "data": info }))),
        None => Err(ZapError::New(-1, "文件不存在".to_string())),
    }
}
