mod file;
mod ssh;
mod ssh_key;
mod time;

use zap_proto::{Request, Response};

/// 白名单动词分发：这里没有、也不会有任意 shell 执行入口。
pub async fn dispatch(req: Request, gid: u32) -> Response {
    match req {
        Request::TimeSync => time::sync().await,
        Request::TimeSetTimezone { timezone } => time::set_timezone(&timezone).await,
        Request::TimeListTimezones => time::list_timezones().await,
        Request::TimeGet => time::get().await,
        Request::SshStatus => ssh::status().await,
        Request::SshRestart => ssh::restart().await,
        Request::SshKeyList => ssh_key::list(gid).await,
        Request::SshKeyGet { name } => ssh_key::get(name, gid).await,
        Request::SshKeyGenerate { name, key_type, bits, comment } => {
            ssh_key::generate(name, key_type, bits, comment, gid).await
        }
        Request::SshKeyImport { name, private_key, public_key } => {
            ssh_key::import(name, private_key, public_key, gid).await
        }
        Request::SshKeyDelete { name } => ssh_key::delete(name).await,
        Request::SshKeyAuthorizedList => ssh_key::authorized_list().await,
        Request::SshKeyAuthorize { name } => ssh_key::authorize(name, gid).await,
        Request::SshKeyDeauthorize { index } => ssh_key::deauthorize(index).await,
        Request::FileList { path } => file::list(path).await,
        Request::FileRead { path } => file::read(path).await,
        Request::FileWrite { path, content } => file::write(path, content).await,
        Request::FileDelete { path } => file::delete(path).await,
        Request::FileMkdir { path } => file::mkdir(path).await,
        Request::FileRename { path, new_path } => file::rename(path, new_path).await,
        Request::FileDownload { path } => file::download(path).await,
        Request::FileUpload { path, name, content } => file::upload(path, name, content).await,
        Request::FileInfo { path } => file::info(path).await,
    }
}

/// 构造一个清空环境、仅带安全 PATH 的 root 子进程命令。
pub(crate) fn root_cmd(program: &str) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    c.env_clear();
    c.env(
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    );
    c
}
