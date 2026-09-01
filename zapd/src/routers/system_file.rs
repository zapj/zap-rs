use axum::{
    body::Body,
    extract::{Extension, Multipart, Query},
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::zap::{
    audit,
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

/// 非管理员可访问的私有目录前缀（自己的 home 与私有临时目录）。
fn user_allowed_prefix(username: &str) -> (String, String) {
    (format!("/home/{username}"), format!("/tmp/zap-{username}"))
}

/// Check if user has read access to a path.
///
/// 权限模型：
/// - 管理员：任意路径；
/// - 普通用户：仅自己 home（`/home/{username}`）与私有临时目录（`/tmp/zap-{username}`）。
///   彻底移除此前"所有人可读 /var/www、/var/log"的越权隐患。
fn check_access(claims: &Claims, path: &Path) -> Result<(), ZapError> {
    if is_admin(claims) {
        return Ok(());
    }
    let path_str = path.to_string_lossy().to_string();
    let (home, tmp) = user_allowed_prefix(&claims.sub);
    if path_str == home
        || path_str.starts_with(&format!("{home}/"))
        || path_str == tmp
        || path_str.starts_with(&format!("{tmp}/"))
    {
        return Ok(());
    }
    Err(ZapError::New(-1, "没有访问该路径的权限".to_string()))
}

/// Check if user can write to a path（与读权限同一套隔离规则）。
fn check_write_access(claims: &Claims, path: &Path) -> Result<(), ZapError> {
    check_access(claims, path)
}

// ── handlers ───────────────────────────────────────────────

/// GET /system/files/list?path=/
pub async fn file_list(
    claims: Claims,
    Query(query): Query<PathQuery>,
) -> ZapJsonResult {
    // 非管理员默认进入自己的 home 目录
    let raw_path = match (is_admin(&claims), query.path.as_deref()) {
        (false, None) | (false, Some("/")) => format!("/home/{}", claims.sub),
        (_, Some(p)) => p.to_string(),
        (_, None) => "/".to_string(),
    };
    let resolved = resolve_path(&raw_path)?;
    check_access(&claims, &resolved)?;

    // 首次访问自己的 home 时自动创建（zapexec 以 root 执行）
    if !is_admin(&claims) {
        let home = format!("/home/{}", claims.sub);
        if resolved.to_string_lossy() == home && !resolved.exists() {
            let _ = crate::zapexec::call(Request::FileMkdir { path: home }).await;
        }
    }

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
    Extension(client_addr): Extension<SocketAddr>,
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
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "file_write",
        &resolved.to_string_lossy(),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
}

/// POST /system/files/delete
pub async fn file_delete(
    claims: Claims,
    Extension(client_addr): Extension<SocketAddr>,
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
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "file_delete",
        &resolved.to_string_lossy(),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

/// POST /system/files/mkdir
pub async fn file_mkdir(
    claims: Claims,
    Extension(client_addr): Extension<SocketAddr>,
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
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "file_mkdir",
        &resolved.to_string_lossy(),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
}

/// POST /system/files/rename
pub async fn file_rename(
    claims: Claims,
    Extension(client_addr): Extension<SocketAddr>,
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
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "file_rename",
        &format!("{} → {}", old_path.to_string_lossy(), new_path.to_string_lossy()),
        "",
    )
    .await;
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
    Extension(client_addr): Extension<SocketAddr>,
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

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "file_upload",
        &resolved_dir.to_string_lossy(),
        &uploaded.join(", "),
    )
    .await;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zap::jwt::Claims;

    fn claims_for(username: &str) -> Claims {
        Claims {
            id: 1,
            iat: 0,
            sub: username.to_string(),
            iss: "Zap".to_string(),
            exp: 0,
            roles: String::new(),
            pwd_is_default: false,
        }
    }

    fn admin_claims() -> Claims {
        let mut c = claims_for("admin");
        c.roles = "admin".to_string();
        c
    }

    #[test]
    fn admin_can_access_any_path() {
        let c = admin_claims();
        assert!(check_access(&c, Path::new("/etc/shadow")).is_ok());
        assert!(check_access(&c, Path::new("/var/www")).is_ok());
    }

    #[test]
    fn user_can_access_own_home() {
        let c = claims_for("alice");
        assert!(check_access(&c, Path::new("/home/alice")).is_ok());
        assert!(check_access(&c, Path::new("/home/alice/www/index.html")).is_ok());
    }

    #[test]
    fn user_cannot_access_others_home() {
        let c = claims_for("alice");
        assert!(check_access(&c, Path::new("/home/bob")).is_err());
        assert!(check_access(&c, Path::new("/home/bob/secret")).is_err());
        // 前缀混淆攻击：/home/aliceevil 不属于 alice
        assert!(check_access(&c, Path::new("/home/aliceevil")).is_err());
    }

    #[test]
    fn user_cannot_access_system_paths() {
        let c = claims_for("alice");
        // 此前普通用户可读 /var/www、/var/log，现已禁止
        for p in ["/var/www", "/var/log", "/etc/passwd", "/root", "/tmp"] {
            assert!(check_access(&c, Path::new(p)).is_err(), "{p} 应被拒绝");
        }
    }

    #[test]
    fn user_can_access_private_tmp() {
        let c = claims_for("alice");
        assert!(check_access(&c, Path::new("/tmp/zap-alice")).is_ok());
        assert!(check_access(&c, Path::new("/tmp/zap-alice/t.txt")).is_ok());
        assert!(check_access(&c, Path::new("/tmp/zap-bob/t.txt")).is_err());
    }

    #[test]
    fn write_access_same_as_read() {
        let c = claims_for("alice");
        assert!(check_write_access(&c, Path::new("/home/alice/x")).is_ok());
        assert!(check_write_access(&c, Path::new("/var/www")).is_err());
    }
}
