//! 敏感数据加密（AES-256-GCM）。
//!
//! - 密钥文件：优先 `/etc/zap/secret.key`（生产），回退 `conf/secret.key`（开发）。
//! - 密钥为 32 字节随机数，首次访问时自动生成，权限 0600。
//! - 密文格式：`v1:<base64(nonce)>:<base64(ciphertext)>`
//! - `decrypt` 对非 `v1:` 前缀的历史明文做兼容返回，用于启动时迁移。

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use once_cell::sync::Lazy;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

fn key_paths() -> [PathBuf; 2] {
    [
        PathBuf::from("/etc/zap/secret.key"),
        PathBuf::from("conf/secret.key"),
    ]
}

fn load_or_create_key() -> Result<[u8; KEY_LEN], String> {
    // 1. 使用已存在的密钥文件
    for path in key_paths() {
        if path.exists() {
            let data = fs::read(&path).map_err(|e| format!("读取密钥 {}: {e}", path.display()))?;
            if data.len() == KEY_LEN {
                let mut key = [0u8; KEY_LEN];
                key.copy_from_slice(&data);
                return Ok(key);
            }
            return Err(format!(
                "密钥文件 {} 长度非法（{}）",
                path.display(),
                data.len()
            ));
        }
    }
    // 2. 生成新密钥：优先写入 /etc/zap，失败（无权限）回退 conf/
    let mut key = [0u8; KEY_LEN];
    getrandom::getrandom(&mut key).map_err(|e| format!("生成密钥失败: {e}"))?;

    let candidates = key_paths();
    let target = candidates
        .iter()
        .find(|p| {
            let parent = p.parent().unwrap_or(Path::new("."));
            let ok = fs::create_dir_all(parent).is_ok()
                && fs::write(p, &key).is_ok()
                && set_key_permissions(p);
            if ok {
                info!("已生成新的加密密钥: {}", p.display());
            }
            ok
        })
        .ok_or_else(|| "无法创建密钥文件（/etc/zap 与 conf 均不可写）".to_string())?;

    if target.to_string_lossy().starts_with("conf/") {
        warn!("密钥写入 conf/ 仅为开发用途，生产环境应部署到 /etc/zap/secret.key");
    }
    Ok(key)
}

#[cfg(unix)]
fn set_key_permissions(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).is_ok()
}

#[cfg(not(unix))]
fn set_key_permissions(_path: &Path) -> bool {
    true
}

/// 全局缓存密钥（只加载/生成一次）。
pub static SECRET_KEY: Lazy<Result<[u8; KEY_LEN], String>> = Lazy::new(load_or_create_key);

/// 加密明文，返回 `v1:nonce:cipher` 格式。
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    let key = SECRET_KEY.as_ref().map_err(|e| e.clone())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|e| e.to_string())?;
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|e| format!("加密失败: {e}"))?;
    Ok(format!("{PREFIX}{}:{}", B64.encode(nonce), B64.encode(ct)))
}

/// 解密；非 `v1:` 前缀视为历史明文，原样返回。
pub fn decrypt(encrypted: &str) -> Result<String, String> {
    if encrypted.is_empty() {
        return Ok(String::new());
    }
    if !encrypted.starts_with(PREFIX) {
        return Ok(encrypted.to_string());
    }
    let key = SECRET_KEY.as_ref().map_err(|e| e.clone())?;
    let body = &encrypted[PREFIX.len()..];
    let (nonce_b64, ct_b64) = body.split_once(':').ok_or("密文格式错误")?;
    let nonce = B64.decode(nonce_b64).map_err(|e| e.to_string())?;
    let ct = B64.decode(ct_b64).map_err(|e| e.to_string())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
        .map_err(|e| format!("解密失败（密钥不匹配或数据损坏）: {e}"))?;
    String::from_utf8(pt).map_err(|e| e.to_string())
}

/// 密码加密入口（加密失败时返回原文并告警，避免服务不可用）。
pub fn encrypt_password(pwd: &str) -> String {
    match encrypt(pwd) {
        Ok(s) => s,
        Err(e) => {
            warn!("SSH 密码加密失败: {e}");
            pwd.to_string()
        }
    }
}

/// 密码解密入口。
pub fn decrypt_password(pwd: &str) -> String {
    match decrypt(pwd) {
        Ok(s) => s,
        Err(e) => {
            warn!("SSH 密码解密失败: {e}");
            String::new()
        }
    }
}

/// 启动迁移：把库中历史明文密码加密回写（幂等，仅处理非 `v1:` 前缀的记录）。
pub async fn migrate_legacy_passwords() {
    use sqlx::Row;

    let pool = match crate::db::get_db_pool_opt().await {
        Some(p) => p,
        None => return,
    };
    let rows = match sqlx::query(
        "SELECT id, password FROM ssh_connections WHERE password != '' AND password NOT LIKE 'v1:%'",
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => return, // 表不存在等情况静默跳过
    };
    let mut migrated = 0usize;
    for row in rows {
        let id: i64 = row.get("id");
        let pwd: String = row.get("password");
        match encrypt(&pwd) {
            Ok(enc) => {
                let _ = sqlx::query("UPDATE ssh_connections SET password = ? WHERE id = ?")
                    .bind(enc)
                    .bind(id)
                    .execute(pool)
                    .await;
                migrated += 1;
            }
            Err(e) => warn!("迁移 SSH 连接 id={id} 密码失败: {e}"),
        }
    }
    if migrated > 0 {
        info!("已迁移 {migrated} 条历史明文 SSH 密码为加密存储");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let plain = "s3cr3t-p@ssw0rd";
        let enc = encrypt(plain).unwrap();
        assert!(enc.starts_with("v1:"));
        assert_ne!(enc, plain);
        assert_eq!(decrypt(&enc).unwrap(), plain);
    }

    #[test]
    fn ciphertext_is_randomized() {
        // 同一明文两次加密结果不同（随机 nonce）
        let a = encrypt("same").unwrap();
        let b = encrypt("same").unwrap();
        assert_ne!(a, b);
        assert_eq!(decrypt(&a).unwrap(), decrypt(&b).unwrap());
    }

    #[test]
    fn decrypt_legacy_plaintext() {
        // 旧版本未加密的明文数据：原样返回，保证迁移期间可用
        assert_eq!(decrypt("old-plain-password").unwrap(), "old-plain-password");
    }

    #[test]
    fn decrypt_empty() {
        assert_eq!(decrypt("").unwrap(), "");
        assert_eq!(encrypt("").unwrap(), "");
    }

    #[test]
    fn decrypt_tampered_fails() {
        let enc = encrypt("hello").unwrap();
        let tampered = format!("{}X", enc);
        assert!(decrypt(&tampered).is_err());
    }
}
