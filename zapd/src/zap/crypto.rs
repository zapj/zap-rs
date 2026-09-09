//! 敏感数据加密（AES-256-GCM）。
//!
//! 底层实现统一由 `zap-crypto` crate 提供：密钥文件 `/etc/zap/secret.key`
//! （开发回退 `conf/secret.key`）、密文格式 `v1:<base64(nonce)>:<base64(ciphertext)>`
//! 与 `zapctl` / `zapexec` 完全一致（同一把密钥、同一种编码，彼此可互通解密）。
//! 本模块只保留 zapd 侧的易用封装。

use tracing::warn;

pub use zap_crypto::{decrypt, encrypt};

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

/// 密码解密入口（解密失败返回空串并告警，调用侧应拒绝使用空密码继续）。
pub fn decrypt_password(pwd: &str) -> String {
    match decrypt(pwd) {
        Ok(s) => s,
        Err(e) => {
            warn!("SSH 密码解密失败: {e}");
            String::new()
        }
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

    #[test]
    fn password_helpers_degrade_gracefully() {
        assert_eq!(decrypt_password(""), ""); // 明文空串原样返回
        // 不可解密的密文 → 空串（调用侧拒用）
        let bad = format!("v1:{}{}", "A".repeat(30), ":YQ==");
        assert_eq!(decrypt_password(&bad), "");
    }
}
