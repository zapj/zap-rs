//! ZAP 统一加密库（AES-256-GCM）与「服务凭据」文件存储。
//!
//! 被 `zapd`（业务进程）、`zapexec`（root 特权进程）、`zapctl`（运维 CLI）共用，
//! 保证三者看到的是**同一套密钥与密文格式**——`zapctl` 存的凭据，`zapexec` 能解开。
//!
//! ## 密钥
//! - 路径：`/etc/zap/secret.key`（生产）→ `conf/secret.key`（开发回退）
//! - 32 字节随机数，首次访问自动生成，权限 `0600`
//!
//! ## 密文格式
//! `v1:<base64(nonce)>:<base64(ciphertext)>`；
//! `decrypt` 对非 `v1:` 前缀的历史明文原样返回（迁移期兼容）。
//!
//! ## 凭据文件
//! - 目录：`/etc/zap/credentials`（`0700`，root 所有）
//! - 文件名：`{service}_{user}.cred`，如 `mysql_root.cred`
//! - 权限：`0400`；内容为单行密文

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

const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// 凭据存储目录（root 专用）。
pub const CRED_DIR: &str = "/etc/zap/credentials";
/// 凭据文件后缀。
pub const CRED_SUFFIX: &str = ".cred";

// ── 密钥 ───────────────────────────────────────────────────

fn key_paths() -> [PathBuf; 2] {
    [
        PathBuf::from("/etc/zap/secret.key"),
        PathBuf::from("conf/secret.key"),
    ]
}

/// 当前生效的密钥文件路径（按生产 → 开发优先级探测），不存在返回 `None`。
/// 仅供备份等场景探测使用：只读取不生成，避免备份动作意外创建密钥。
pub fn active_key_path() -> Option<PathBuf> {
    key_paths().into_iter().find(|p| p.is_file())
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

fn load_or_create_key() -> Result<[u8; KEY_LEN], String> {
    for path in key_paths() {
        if path.exists() {
            let data = fs::read(&path).map_err(|e| format!("读取密钥 {}: {e}", path.display()))?;
            if data.len() == KEY_LEN {
                let mut key = [0u8; KEY_LEN];
                key.copy_from_slice(&data);
                return Ok(key);
            }
            return Err(format!(
                "密钥文件 {} 长度非法（{} 字节）",
                path.display(),
                data.len()
            ));
        }
    }

    let mut key = [0u8; KEY_LEN];
    getrandom::getrandom(&mut key).map_err(|e| format!("生成密钥失败: {e}"))?;

    let candidates = key_paths();
    candidates
        .iter()
        .find(|p| {
            let parent = p.parent().unwrap_or(Path::new("."));
            fs::create_dir_all(parent).is_ok()
                && fs::write(p, key).is_ok()
                && set_key_permissions(p)
        })
        .ok_or_else(|| "无法创建密钥文件（/etc/zap 与 conf 均不可写）".to_string())?;

    Ok(key)
}

/// 进程内缓存的主密钥（仅加载/生成一次）。
pub static SECRET_KEY: Lazy<Result<[u8; KEY_LEN], String>> = Lazy::new(load_or_create_key);

// ── 加解密 ─────────────────────────────────────────────────

/// 加密明文，返回 `v1:nonce:ciphertext`。
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }
    let key = SECRET_KEY.as_ref().map_err(|e| e.clone())?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).map_err(|e| format!("生成 nonce 失败: {e}"))?;
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
        .map_err(|e| format!("加密失败: {e}"))?;
    Ok(format!("{PREFIX}{}:{}", B64.encode(nonce), B64.encode(ct)))
}

/// 解密；非 `v1:` 前缀视为历史明文原样返回。
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

// ── 密码生成 ───────────────────────────────────────────────

const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SYMBOLS: &[u8] = b"!@#%^&*()-_=+";

/// 生成随机密码。默认只含字母数字（避免 shell / SQL / 配置文件转义问题）。
pub fn generate_password(len: usize, symbols: bool) -> Result<String, String> {
    let len = len.clamp(8, 128);
    let mut alphabet: Vec<u8> = ALNUM.to_vec();
    if symbols {
        alphabet.extend_from_slice(SYMBOLS);
    }
    let mut out = String::with_capacity(len);
    let mut buf = [0u8; 1];
    for _ in 0..len {
        getrandom::getrandom(&mut buf).map_err(|e| format!("随机数生成失败: {e}"))?;
        out.push(alphabet[buf[0] as usize % alphabet.len()] as char);
    }
    Ok(out)
}

// ── 凭据文件 ───────────────────────────────────────────────

/// 校验 service / user 片段：只允许 `[A-Za-z0-9._-]`，禁止空、`..`、超长。
pub fn sanitize_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if n.len() > 64 {
        return Err("名称过长（上限 64 字符）".to_string());
    }
    if !n
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(format!("名称只能包含字母、数字、点、下划线、短横线: '{n}'"));
    }
    if n == "." || n == ".." {
        return Err(format!("非法名称: '{n}'"));
    }
    Ok(n.to_string())
}

/// 凭据文件路径：`/etc/zap/credentials/{service}_{user}.cred`
pub fn cred_path(service: &str, user: &str) -> Result<PathBuf, String> {
    let s = sanitize_name(service)?;
    let u = sanitize_name(user)?;
    Ok(PathBuf::from(CRED_DIR).join(format!("{s}_{u}{CRED_SUFFIX}")))
}

#[cfg(unix)]
fn write_secure(path: &Path, content: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建凭据目录失败: {e}"))?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("设置目录权限失败: {e}"))?;
    }
    fs::write(path, content).map_err(|e| format!("写入凭据失败: {e}"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o400))
        .map_err(|e| format!("设置凭据权限失败: {e}"))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secure(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建凭据目录失败: {e}"))?;
    }
    fs::write(path, content).map_err(|e| format!("写入凭据失败: {e}"))
}

/// 加密保存凭据，返回文件路径。
pub fn save_cred(service: &str, user: &str, password: &str) -> Result<PathBuf, String> {
    let path = cred_path(service, user)?;
    let enc = encrypt(password)?;
    write_secure(&path, &enc)?;
    Ok(path)
}

/// 读取并解密凭据。
pub fn read_cred(service: &str, user: &str) -> Result<String, String> {
    let path = cred_path(service, user)?;
    if !path.is_file() {
        return Err(format!("凭据不存在: {}", path.display()));
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取凭据失败: {e}"))?;
    decrypt(raw.trim())
}

/// 凭据是否存在。
pub fn cred_exists(service: &str, user: &str) -> bool {
    cred_path(service, user).is_ok_and(|p| p.is_file())
}

/// 列出已保存凭据：`(显示名, 文件路径)`。
pub fn list_creds() -> Result<Vec<(String, PathBuf)>, String> {
    let dir = PathBuf::from(CRED_DIR);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("读取凭据目录失败: {e}"))? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(CRED_SUFFIX) else {
            continue;
        };
        out.push((stem.to_string(), path));
    }
    out.sort();
    Ok(out)
}

/// 删除凭据（文件不存在也算成功）。
pub fn remove_cred(service: &str, user: &str) -> Result<(), String> {
    let path = cred_path(service, user)?;
    if path.is_file() {
        fs::remove_file(&path).map_err(|e| format!("删除凭据失败: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let enc = encrypt("s3cr3t-p@ssw0rd").unwrap();
        assert!(enc.starts_with("v1:"));
        assert_ne!(enc, "s3cr3t-p@ssw0rd");
        assert_eq!(decrypt(&enc).unwrap(), "s3cr3t-p@ssw0rd");
    }

    #[test]
    fn ciphertext_is_randomized() {
        assert_ne!(encrypt("same").unwrap(), encrypt("same").unwrap());
    }

    #[test]
    fn decrypt_legacy_plaintext() {
        assert_eq!(decrypt("old-plain").unwrap(), "old-plain");
    }

    #[test]
    fn decrypt_tampered_fails() {
        let enc = encrypt("hello").unwrap();
        assert!(decrypt(&format!("{enc}X")).is_err());
    }

    #[test]
    fn password_length_and_alphabet() {
        let p = generate_password(20, false).unwrap();
        assert_eq!(p.len(), 20);
        assert!(p.chars().all(|c| c.is_ascii_alphanumeric()));
        // 长度下限兜底
        assert_eq!(generate_password(1, false).unwrap().len(), 8);
    }

    #[test]
    fn two_passwords_differ() {
        assert_ne!(
            generate_password(20, false).unwrap(),
            generate_password(20, false).unwrap()
        );
    }

    #[test]
    fn name_sanitize_rejects_traversal() {
        assert!(sanitize_name("../etc").is_err());
        assert!(sanitize_name("a/b").is_err());
        assert!(sanitize_name("").is_err());
        assert!(sanitize_name("..").is_err());
        assert_eq!(sanitize_name("mysql_8.0").unwrap(), "mysql_8.0");
    }

    #[test]
    fn cred_file_naming() {
        let p = cred_path("mysql", "root").unwrap();
        assert_eq!(p, PathBuf::from("/etc/zap/credentials/mysql_root.cred"));
        assert!(cred_path("mysql", "../root").is_err());
    }
}
