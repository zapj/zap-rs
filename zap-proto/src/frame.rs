use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::types::Message;

/// 单帧上限，防止恶意超大帧占用内存。
const MAX_FRAME: u32 = 16 * 1024 * 1024;

/// 发送一帧：4 字节大端长度前缀 + JSON 载荷。
pub async fn send<W: AsyncWrite + Unpin>(w: &mut W, msg: &Message) -> io::Result<()> {
    let bytes = serde_json::to_vec(msg).map_err(serde_err)?;
    if bytes.len() > MAX_FRAME as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    w.write_u32(bytes.len() as u32).await?;
    w.write_all(&bytes).await?;
    w.flush().await?;
    Ok(())
}

/// 读取一帧。
pub async fn recv<R: AsyncRead + Unpin>(r: &mut R) -> io::Result<Message> {
    let len = r.read_u32().await?;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await?;
    serde_json::from_slice(&buf).map_err(serde_err)
}

fn serde_err(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}
