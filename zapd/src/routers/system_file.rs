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

use crate::zap::{
    jwt::{is_admin, Claims},
    ZapError, ZapJsonResult,
};
use zap_proto::Request;

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
// 授权（基于 JWT 角色）仍在 zapd 完成；实际文件操作转发给 zapexec（root）。

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

// ── handlers ───────────────────────────────────────────────

/// GET /system/files/list?path=/
pub async fn file_list(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> ZapJsonResult {
    let raw_path = query.path.as_deref().unwrap_or("/");
    let resolved = resolve_path(raw_path)?;
    check_access(&claims, &resolved)?;

    let resp = crate::zapexec::call(Request::FileList {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "ok", "data": resp.data })))
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

    let resp = crate::zapexec::call(Request::FileRead {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "ok", "data": resp.data })).into_response())
}

/// POST /system/files/write
pub async fn file_write(
    claims: Claims,
    Json(payload): Json<FileOpPayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    let resp = crate::zapexec::call(Request::FileWrite {
        path: resolved.to_string_lossy().to_string(),
        content: payload.content,
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
}

/// POST /system/files/delete
pub async fn file_delete(
    claims: Claims,
    Json(payload): Json<DeletePayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    let resp = crate::zapexec::call(Request::FileDelete {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

/// POST /system/files/mkdir
pub async fn file_mkdir(
    claims: Claims,
    Json(payload): Json<MkdirPayload>,
) -> ZapJsonResult {
    let resolved = resolve_path(&payload.path)?;
    check_write_access(&claims, &resolved)?;

    let resp = crate::zapexec::call(Request::FileMkdir {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
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

    let resp = crate::zapexec::call(Request::FileRename {
        path: old_path.to_string_lossy().to_string(),
        new_path: new_path.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
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

    let resp = crate::zapexec::call(Request::FileDownload {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }

    let data = resp.data.unwrap_or_else(|| json!({}));
    let file_name = data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("download")
        .to_string();
    let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let bytes = zap_proto::b64_decode(content)
        .map_err(|e| ZapError::Error(format!("内容解码失败: {e}")))?;

    let mime = mime_guess::from_path(&resolved).first_or_octet_stream();

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_name),
        )
        .body(Body::from(bytes))
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

    let mut uploaded: Vec<String> = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("unnamed").to_string();
        let safe_name = Path::new(&file_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        let data = field.bytes().await.map_err(|e| {
            ZapError::New(-1, format!("上传失败: {}", e))
        })?;

        let resp = crate::zapexec::call(Request::FileUpload {
            path: resolved_dir.to_string_lossy().to_string(),
            name: safe_name.clone(),
            content: zap_proto::b64_encode(&data),
        })
        .await?;
        if resp.code != 0 {
            return Err(ZapError::New(resp.code, resp.message));
        }
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

    let resp = crate::zapexec::call(Request::FileInfo {
        path: resolved.to_string_lossy().to_string(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "ok", "data": resp.data })))
}
