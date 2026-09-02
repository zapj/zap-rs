use std::os::unix::io::AsRawFd;
use std::path::Path;

use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, info, warn};

use zap_proto::{auth, frame, types::Message};

use crate::verbs;

#[derive(Clone, Copy, Debug)]
pub struct ClientIdentity {
    pub uid: u32,
    pub gid: u32,
}

pub async fn serve(socket: &Path, secret: &[u8], identity: ClientIdentity) {
    if let Some(dir) = socket.parent() {
        let _ = std::fs::create_dir_all(dir);
        set_owner_mode(dir, identity, 0o750);
    }
    if socket.exists() {
        let _ = std::fs::remove_file(socket);
    }

    let listener = match UnixListener::bind(socket) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("绑定 {} 失败: {e}", socket.display());
            return;
        }
    };
    set_owner_mode(socket, identity, 0o660);

    info!("监听 {}", socket.display());

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let secret = secret.to_vec();
                tokio::spawn(async move {
                    if let Err(e) = handle_conn(stream, &secret, identity.uid, identity.gid).await {
                        debug!("连接结束: {e}");
                    }
                });
            }
            Err(e) => warn!("accept 错误: {e}"),
        }
    }
}

async fn handle_conn(
    stream: UnixStream,
    secret: &[u8],
    expected_uid: u32,
    gid: u32,
) -> std::io::Result<()> {
    // 1) SO_PEERCRED：只允许 zapadm 用户连接
    let uid = match peer_uid(stream.as_raw_fd()) {
        Some(u) => u,
        None => {
            warn!("无法获取对端凭据，拒绝连接");
            return Ok(());
        }
    };
    if uid != expected_uid {
        warn!("拒绝来自 uid {uid} 的连接（期望 {expected_uid}）");
        return Ok(());
    }

    let (mut rd, mut wr) = stream.into_split();

    // 2) 挑战/响应 HMAC 握手
    let challenge = auth::challenge_hex()?;
    frame::send(
        &mut wr,
        &Message::Challenge {
            challenge: challenge.clone(),
        },
    )
    .await?;

    match frame::recv(&mut rd).await? {
        Message::Auth { mac } => {
            if !auth::verify_hex(secret, challenge.as_bytes(), &mac) {
                warn!("uid {uid} 认证失败");
                return Ok(());
            }
        }
        _ => {
            warn!("握手阶段收到非法消息");
            return Ok(());
        }
    }
    frame::send(&mut wr, &Message::Welcome).await?;

    // 3) 请求/响应循环
    loop {
        match frame::recv(&mut rd).await {
            Ok(Message::Request(req)) => {
                let resp = verbs::dispatch(req, gid).await;
                if frame::send(&mut wr, &Message::Response(resp))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(_) => break,
            Err(e) => {
                debug!("读取错误: {e}");
                break;
            }
        }
    }
    Ok(())
}

fn peer_uid(fd: std::os::unix::io::RawFd) -> Option<u32> {
    unsafe {
        let mut cred: libc::ucred = std::mem::zeroed();
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let rc = libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut libc::c_void,
            &mut len,
        );
        if rc == 0 { Some(cred.uid) } else { None }
    }
}

pub(crate) fn set_owner_mode(path: &Path, identity: ClientIdentity, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    let Ok(cpath) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return;
    };
    unsafe {
        // best-effort：root 下将属组设为 zapadm；非 root（开发）下静默失败
        libc::chown(cpath.as_ptr(), 0, identity.gid);
    }
}
