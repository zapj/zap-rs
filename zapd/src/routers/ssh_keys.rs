use axum::Json;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

use crate::zap::{jwt::ValidatedClaims, ZapError, ZapJsonResult};

// ── helpers ────────────────────────────────────────────────

fn ssh_dir() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".ssh"))
        .unwrap_or_else(|_| PathBuf::from("/root/.ssh"))
}

fn ensure_ssh_dir() -> Result<(), ZapError> {
    let dir = ssh_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ZapError::Error(format!("无法创建 .ssh 目录: {}", e))
        })?;
        // Set correct permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).ok();
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct SshKeyInfo {
    name: String,
    key_type: String,
    bits: u32,
    fingerprint: String,
    comment: String,
    public_key: String,
    authorized: bool,
    created_at: String,
}

fn parse_pub_file(path: &std::path::Path) -> Option<SshKeyInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let parts: Vec<&str> = content.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let key_type = parts[0].to_string();
    let _key_data = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let comment = parts.get(2).unwrap_or(&"").to_string();

    let name = path
        .file_stem()
        .map(|n| {
            let s = n.to_string_lossy();
            s.strip_suffix(".pub").unwrap_or(&s).to_string()
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get fingerprint via ssh-keygen
    let fingerprint = Command::new("ssh-keygen")
        .args(["-lf", &path.to_string_lossy()])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let out = String::from_utf8_lossy(&o.stdout);
                out.split_whitespace().nth(1).map(|s| s.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get bits from fingerprint
    let bits = fingerprint
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let created_at = path
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_default();

    // Check if this key is in authorized_keys
    let auth_path = ssh_dir().join("authorized_keys");
    let authorized = if auth_path.exists() {
        std::fs::read_to_string(&auth_path)
            .map(|ak| ak.lines().any(|line| line.trim() == content.trim()))
            .unwrap_or(false)
    } else {
        false
    };

    Some(SshKeyInfo {
        name,
        key_type,
        bits,
        fingerprint,
        comment,
        public_key: content.trim().to_string(),
        authorized,
        created_at,
    })
}

// ── handlers ───────────────────────────────────────────────

/// List all SSH keys in ~/.ssh/
pub async fn list_keys(_claims: ValidatedClaims) -> ZapJsonResult {
    ensure_ssh_dir()?;
    let dir = ssh_dir();
    let mut keys = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "pub").unwrap_or(false) {
                if let Some(info) = parse_pub_file(&path) {
                    keys.push(info);
                }
            }
        }
    }

    keys.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(json!({ "code": 0, "data": keys })))
}

/// Get the public key content for a specific key
pub async fn get_key_content(
    _claims: ValidatedClaims,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ZapJsonResult {
    let name = params.get("name").cloned().unwrap_or_default();
    if name.is_empty() {
        return Err(ZapError::New(-1, "缺少密钥名称".to_string()));
    }
    // Prevent path traversal
    if name.contains('/') || name.contains("..") {
        return Err(ZapError::New(-1, "无效的密钥名称".to_string()));
    }

    let path = ssh_dir().join(format!("{}.pub", name));
    if !path.exists() {
        return Err(ZapError::New(-1, "密钥不存在".to_string()));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| ZapError::Error(format!("读取失败: {}", e)))?;

    Ok(Json(json!({ "code": 0, "data": { "name": name, "public_key": content.trim() } })))
}

// ── generate ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenerateKeyPayload {
    pub name: String,
    pub key_type: Option<String>, // rsa, ed25519, ecdsa
    pub bits: Option<u32>,        // for rsa
    pub comment: Option<String>,
}

pub async fn generate_key(
    _claims: ValidatedClaims,
    Json(payload): Json<GenerateKeyPayload>,
) -> ZapJsonResult {
    if payload.name.is_empty() || payload.name.contains('/') || payload.name.contains("..") {
        return Err(ZapError::New(-1, "无效的密钥名称".to_string()));
    }

    ensure_ssh_dir()?;
    let dir = ssh_dir();
    let key_path = dir.join(&payload.name);
    let pub_path = dir.join(format!("{}.pub", &payload.name));

    if key_path.exists() || pub_path.exists() {
        return Err(ZapError::New(-1, "密钥已存在".to_string()));
    }

    let key_type = payload.key_type.unwrap_or_else(|| "ed25519".to_string());
    let comment = payload.comment.unwrap_or_else(|| format!("{}@zap", payload.name));

    let mut cmd = Command::new("ssh-keygen");
    cmd.args(["-t", &key_type])
        .args(["-f", &key_path.to_string_lossy()])
        .args(["-C", &comment])
        .args(["-N", ""]) // no passphrase
        .arg("-q"); // quiet

    if key_type == "rsa" {
        let bits = payload.bits.unwrap_or(4096);
        cmd.args(["-b", &bits.to_string()]);
    }

    let output = cmd.output().map_err(|e| ZapError::Error(format!("ssh-keygen 执行失败: {}", e)))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(ZapError::New(-1, format!("密钥生成失败: {}", err)));
    }

    // Set correct permissions on private key
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).ok();
        std::fs::set_permissions(&pub_path, std::fs::Permissions::from_mode(0o644)).ok();
    }

    info!("SSH key generated: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "密钥生成成功", "data": { "name": payload.name } })))
}

// ── import ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ImportKeyPayload {
    pub name: String,
    pub private_key: String,
    pub public_key: Option<String>,
}

pub async fn import_key(
    _claims: ValidatedClaims,
    Json(payload): Json<ImportKeyPayload>,
) -> ZapJsonResult {
    if payload.name.is_empty() || payload.name.contains('/') || payload.name.contains("..") {
        return Err(ZapError::New(-1, "无效的密钥名称".to_string()));
    }

    ensure_ssh_dir()?;
    let dir = ssh_dir();
    let key_path = dir.join(&payload.name);
    let pub_path = dir.join(format!("{}.pub", &payload.name));

    if key_path.exists() || pub_path.exists() {
        return Err(ZapError::New(-1, "密钥已存在".to_string()));
    }

    // Write private key
    std::fs::write(&key_path, payload.private_key.trim())
        .map_err(|e| ZapError::Error(format!("写入私钥失败: {}", e)))?;

    // Write or derive public key
    if let Some(pub_key) = &payload.public_key {
        if !pub_key.trim().is_empty() {
            std::fs::write(&pub_path, pub_key.trim())
                .map_err(|e| ZapError::Error(format!("写入公钥失败: {}", e)))?;
        }
    }

    // If no public key provided, try to derive it
    if !pub_path.exists() {
        let output = Command::new("ssh-keygen")
            .args(["-y", "-f", &key_path.to_string_lossy()])
            .output();

        if let Ok(o) = output {
            if o.status.success() {
                let pub_content = String::from_utf8_lossy(&o.stdout);
                std::fs::write(&pub_path, pub_content.trim()).ok();
            }
        }
    }

    // Set permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).ok();
        if pub_path.exists() {
            std::fs::set_permissions(&pub_path, std::fs::Permissions::from_mode(0o644)).ok();
        }
    }

    info!("SSH key imported: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "密钥导入成功" })))
}

// ── delete ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DeleteKeyPayload {
    pub name: String,
}

pub async fn delete_key(
    _claims: ValidatedClaims,
    Json(payload): Json<DeleteKeyPayload>,
) -> ZapJsonResult {
    if payload.name.is_empty() || payload.name.contains('/') || payload.name.contains("..") {
        return Err(ZapError::New(-1, "无效的密钥名称".to_string()));
    }

    let dir = ssh_dir();
    let key_path = dir.join(&payload.name);
    let pub_path = dir.join(format!("{}.pub", &payload.name));

    if !key_path.exists() && !pub_path.exists() {
        return Err(ZapError::New(-1, "密钥不存在".to_string()));
    }

    if key_path.exists() {
        std::fs::remove_file(&key_path).map_err(|e| ZapError::Error(format!("删除失败: {}", e)))?;
    }
    if pub_path.exists() {
        std::fs::remove_file(&pub_path).map_err(|e| ZapError::Error(format!("删除失败: {}", e)))?;
    }

    // Also remove from authorized_keys
    if let Some(pub_content) = pub_path_opt(&dir, &payload.name) {
        remove_from_authorized_keys_by_content(&pub_content);
    }

    info!("SSH key deleted: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "删除成功" })))
}

fn pub_path_opt(dir: &std::path::Path, name: &str) -> Option<String> {
    let path = dir.join(format!("{}.pub", name));
    if path.exists() {
        std::fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
    } else {
        None
    }
}

// ── authorized_keys ────────────────────────────────────────

#[derive(serde::Serialize)]
struct AuthorizedKeyEntry {
    index: usize,
    key_type: String,
    key_data_short: String,
    comment: String,
    full_line: String,
}

/// List all entries in authorized_keys
pub async fn list_authorized_keys(_claims: ValidatedClaims) -> ZapJsonResult {
    let path = ssh_dir().join("authorized_keys");
    if !path.exists() {
        return Ok(Json(json!({ "code": 0, "data": [] })));
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| ZapError::Error(format!("读取失败: {}", e)))?;

    let entries: Vec<AuthorizedKeyEntry> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();
            AuthorizedKeyEntry {
                index: i,
                key_type: parts.first().unwrap_or(&"").to_string(),
                key_data_short: parts
                    .get(1)
                    .map(|d| {
                        if d.len() > 40 {
                            format!("{}...", &d[..40])
                        } else {
                            d.to_string()
                        }
                    })
                    .unwrap_or_default(),
                comment: parts.get(2).unwrap_or(&"").to_string(),
                full_line: line.trim().to_string(),
            }
        })
        .collect();

    Ok(Json(json!({ "code": 0, "data": entries })))
}

/// Authorize a key (add to authorized_keys)
#[derive(Debug, Deserialize)]
pub struct AuthorizeKeyPayload {
    pub name: String, // key name from ~/.ssh/
}

pub async fn authorize_key(
    _claims: ValidatedClaims,
    Json(payload): Json<AuthorizeKeyPayload>,
) -> ZapJsonResult {
    if payload.name.is_empty() || payload.name.contains('/') || payload.name.contains("..") {
        return Err(ZapError::New(-1, "无效的密钥名称".to_string()));
    }

    ensure_ssh_dir()?;
    let dir = ssh_dir();
    let pub_path = dir.join(format!("{}.pub", &payload.name));

    if !pub_path.exists() {
        return Err(ZapError::New(-1, "公钥不存在，请先生成或导入密钥".to_string()));
    }

    let pub_content = std::fs::read_to_string(&pub_path)
        .map_err(|e| ZapError::Error(format!("读取公钥失败: {}", e)))?;
    let pub_line = pub_content.trim();

    let auth_path = dir.join("authorized_keys");
    let mut existing = if auth_path.exists() {
        std::fs::read_to_string(&auth_path).unwrap_or_default()
    } else {
        String::new()
    };

    // Check if already authorized
    if existing.lines().any(|l| l.trim() == pub_line) {
        return Ok(Json(json!({ "code": 0, "message": "该密钥已授权" })));
    }

    // Append
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(pub_line);
    existing.push('\n');

    std::fs::write(&auth_path, &existing)
        .map_err(|e| ZapError::Error(format!("写入失败: {}", e)))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&auth_path, std::fs::Permissions::from_mode(0o600)).ok();
    }

    info!("SSH key authorized: {}", payload.name);
    Ok(Json(json!({ "code": 0, "message": "授权成功" })))
}

/// Deauthorize a key (remove from authorized_keys by index)
#[derive(Debug, Deserialize)]
pub struct DeauthorizeKeyPayload {
    pub index: usize,
}

pub async fn deauthorize_key(
    _claims: ValidatedClaims,
    Json(payload): Json<DeauthorizeKeyPayload>,
) -> ZapJsonResult {
    let auth_path = ssh_dir().join("authorized_keys");
    if !auth_path.exists() {
        return Err(ZapError::New(-1, "authorized_keys 不存在".to_string()));
    }

    let content = std::fs::read_to_string(&auth_path)
        .map_err(|e| ZapError::Error(format!("读取失败: {}", e)))?;

    let lines: Vec<&str> = content.lines().collect();
    if payload.index >= lines.len() {
        return Err(ZapError::New(-1, "无效的索引".to_string()));
    }

    let new_content: String = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != payload.index)
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n");
    let new_content = new_content.trim().to_string() + "\n";

    std::fs::write(&auth_path, new_content)
        .map_err(|e| ZapError::Error(format!("写入失败: {}", e)))?;

    Ok(Json(json!({ "code": 0, "message": "取消授权成功" })))
}

fn remove_from_authorized_keys_by_content(pub_line: &str) {
    let auth_path = ssh_dir().join("authorized_keys");
    if !auth_path.exists() {
        return;
    }
    if let Ok(content) = std::fs::read_to_string(&auth_path) {
        let new_content: String = content
            .lines()
            .filter(|l| l.trim() != pub_line)
            .collect::<Vec<_>>()
            .join("\n");
        let new_content = new_content.trim().to_string() + "\n";
        std::fs::write(&auth_path, new_content).ok();
    }
}
