//! `zapd`（非特权）与 `zapexec`（root）之间的共享 wire 协议。
//!
//! - 传输：Unix domain socket，长度前缀 + JSON 帧
//! - 认证：连接建立后做挑战/响应 HMAC-SHA256（共享密钥），
//!   服务端另外做 SO_PEERCRED 校验（仅允许 `zapadm` 连接）
//!
//! 注意：`Request` 只包含白名单动词，**没有**任意 shell 执行入口。

pub mod auth;
pub mod frame;
pub mod types;

pub use types::{Message, Request, Response};
