use std::path::PathBuf;

use clap::{Args, Subcommand};
use tokio::net::UnixStream;

use zap_proto::{auth, frame, types::Message, Request};

#[derive(Args)]
pub struct ClientArgs {
    #[clap(long, default_value = "/run/zap/exec.sock")]
    socket: PathBuf,

    #[clap(long, default_value = "/etc/zap/exec.key")]
    secret: PathBuf,

    #[command(subcommand)]
    verb: ClientVerb,
}

#[derive(Subcommand)]
enum ClientVerb {
    TimeGet,
    TimeSync,
    TimeListTimezones,
    TimeSetTimezone { timezone: String },
    SshStatus,
    SshRestart,
    SshKeyList,
    SshKeyGenerate { name: String },
    SshKeyAuthorizedList,
    FileList { path: String },
    FileRead { path: String },
    FileWrite { path: String, content: String },
    FileDelete { path: String },
    FileInfo { path: String },
}

pub async fn run(args: ClientArgs) {
    let secret = auth::load_secret(args.secret.to_str().unwrap_or_default()).expect("无法读取密钥");
    let stream = UnixStream::connect(&args.socket).await.expect("无法连接 socket");
    let (mut rd, mut wr) = stream.into_split();

    // 握手
    match frame::recv(&mut rd).await.expect("读取挑战失败") {
        Message::Challenge { challenge } => {
            let mac = auth::hmac_hex(&secret, challenge.as_bytes());
            frame::send(&mut wr, &Message::Auth { mac })
                .await
                .expect("发送认证失败");
        }
        _ => panic!("预期收到挑战消息"),
    }
    match frame::recv(&mut rd).await.expect("读取欢迎消息失败") {
        Message::Welcome => {}
        _ => panic!("握手被拒绝"),
    }

    let req = match args.verb {
        ClientVerb::TimeGet => Request::TimeGet,
        ClientVerb::TimeSync => Request::TimeSync,
        ClientVerb::TimeListTimezones => Request::TimeListTimezones,
        ClientVerb::TimeSetTimezone { timezone } => Request::TimeSetTimezone { timezone },
        ClientVerb::SshStatus => Request::SshStatus,
        ClientVerb::SshRestart => Request::SshRestart,
        ClientVerb::SshKeyList => Request::SshKeyList,
        ClientVerb::SshKeyGenerate { name } => Request::SshKeyGenerate {
            name,
            key_type: None,
            bits: None,
            comment: None,
        },
        ClientVerb::SshKeyAuthorizedList => Request::SshKeyAuthorizedList,
        ClientVerb::FileList { path } => Request::FileList { path },
        ClientVerb::FileRead { path } => Request::FileRead { path },
        ClientVerb::FileWrite { path, content } => Request::FileWrite { path, content },
        ClientVerb::FileDelete { path } => Request::FileDelete { path },
        ClientVerb::FileInfo { path } => Request::FileInfo { path },
    };

    frame::send(&mut wr, &Message::Request(req))
        .await
        .expect("发送请求失败");

    match frame::recv(&mut rd).await.expect("读取响应失败") {
        Message::Response(resp) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&resp).expect("序列化响应失败")
            );
        }
        _ => panic!("预期收到响应"),
    }
}
