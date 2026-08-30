use std::io;

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub const SECRET_LEN: usize = 32;

/// 读取共享密钥文件（64 个 hex 字符 = 32 字节）。
pub fn load_secret(path: &str) -> io::Result<Vec<u8>> {
    let s = std::fs::read_to_string(path)
        .map_err(|e| io::Error::new(e.kind(), format!("读取密钥 {path}: {e}")))?;
    let bytes = hex::decode(s.trim())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("密钥不是合法 hex: {e}")))?;
    if bytes.len() != SECRET_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "密钥必须为 32 字节（64 hex）",
        ));
    }
    Ok(bytes)
}

/// 生成全新随机密钥（32 字节 -> 64 hex）。
pub fn generate_secret_hex() -> io::Result<String> {
    let mut buf = [0u8; SECRET_LEN];
    getrandom::getrandom(&mut buf).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(hex::encode(buf))
}

/// 生成随机挑战（16 字节 -> 32 hex）。
pub fn challenge_hex() -> io::Result<String> {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(hex::encode(buf))
}

/// 计算 HMAC-SHA256 并输出 hex。
pub fn hmac_hex(secret: &[u8], data: &[u8]) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).expect("hmac 接受任意长度密钥");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

/// 常量时间校验 hex 形式的 HMAC。
pub fn verify_hex(secret: &[u8], data: &[u8], mac_hex: &str) -> bool {
    let Ok(expected) = hex::decode(mac_hex) else {
        return false;
    };
    let mut mac = <HmacSha256 as Mac>::new_from_slice(secret).expect("hmac 接受任意长度密钥");
    mac.update(data);
    mac.verify_slice(&expected).is_ok()
}
