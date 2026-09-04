mod appstore;
mod env;
mod file;
mod network;
mod php;
mod process;
mod service;
mod site;
mod ssh;
mod ssh_key;
mod time;
mod user;

use std::path::PathBuf;

use zap_proto::{Request, Response};

/// 软件安装根目录：第三方软件本体安装到此处（与 zap 面板数据解耦）。
/// 默认 `/usr/local/apps`；可用环境变量 `ZAP_APPS_DIR` 覆盖
/// （rundev / systemd / 自定义脚本均可设置）。
/// 注意：安装元数据（meta.yaml / info.yaml）仍在 `{ZAP_PATH}/data/apps`，
/// 两者职责不同。
pub(super) fn install_root() -> PathBuf {
    std::env::var("ZAP_APPS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/usr/local/apps"))
}

/// 白名单动词分发：这里没有、也不会有任意 shell 执行入口。
pub async fn dispatch(req: Request, gid: u32) -> Response {
    match req {
        Request::TimeSync => time::sync().await,
        Request::TimeSetTimezone { timezone } => time::set_timezone(&timezone).await,
        Request::TimeListTimezones => time::list_timezones().await,
        Request::TimeGet => time::get().await,
        Request::NetworkGet => network::get().await,
        Request::NetworkSetHostname { hostname } => network::set_hostname(&hostname).await,
        Request::NetworkSetResolver {
            nameservers,
            search,
        } => network::set_resolver(&nameservers, &search).await,
        Request::SshStatus => ssh::status().await,
        Request::SshRestart => ssh::restart().await,
        Request::SshInstall { run_id } => ssh::install(run_id).await,
        Request::ServiceList => service::list().await,
        Request::ServiceAction { name, action } => service::action(&name, &action).await,
        Request::ProcessList => process::list().await,
        Request::ProcessKill { pid, signal } => process::kill(pid, signal).await,
        Request::SshKeyList => ssh_key::list(gid).await,
        Request::SshKeyGet { name } => ssh_key::get(name, gid).await,
        Request::SshKeyGenerate {
            name,
            key_type,
            bits,
            comment,
        } => ssh_key::generate(name, key_type, bits, comment, gid).await,
        Request::SshKeyImport {
            name,
            private_key,
            public_key,
        } => ssh_key::import(name, private_key, public_key, gid).await,
        Request::SshKeyDelete { name } => ssh_key::delete(name).await,
        Request::SshKeyAuthorizedList => ssh_key::authorized_list().await,
        Request::SshKeyAuthorize { name } => ssh_key::authorize(name, gid).await,
        Request::SshKeyDeauthorize { index } => ssh_key::deauthorize(index).await,
        Request::SshKeyInstallLocal { username, key_name } => {
            ssh_key::install_local(username, key_name).await
        }
        Request::FileList { path } => file::list(path).await,
        Request::FileRead { path } => file::read(path).await,
        Request::FileWrite { path, content } => file::write(path, content).await,
        Request::FileDelete { path } => file::delete(path).await,
        Request::FileMkdir { path } => file::mkdir(path).await,
        Request::FileRename { path, new_path } => file::rename(path, new_path).await,
        Request::FileDownload { path } => file::download(path).await,
        Request::FileUpload {
            path,
            name,
            content,
        } => file::upload(path, name, content).await,
        Request::FileInfo { path } => file::info(path).await,
        Request::AppstoreRepoAdd { name, url, run_id } => {
            appstore::repo_add(name, url, run_id).await
        }
        Request::AppstoreRepoRemove { id } => appstore::repo_remove(id).await,
        Request::AppstoreRepoUpdate { id, run_id } => appstore::repo_update(id, run_id).await,
        Request::AppstoreInstall {
            pkg_path,
            source,
            repo_id,
            version,
            action,
            options,
            run_id,
        } => appstore::install(pkg_path, source, repo_id, version, action, options, run_id).await,
        Request::AppstoreUninstall { pkg_path, run_id } => {
            appstore::uninstall(pkg_path, run_id).await
        }
        Request::AppstoreUpgrade {
            pkg_path,
            source,
            repo_id,
            version,
            old_version,
            action,
            options,
            run_id,
        } => {
            appstore::upgrade(
                pkg_path,
                source,
                repo_id,
                version,
                old_version,
                action,
                options,
                run_id,
            )
            .await
        }
        Request::AppstoreScriptRun { path, run_id } => appstore::script_run(path, run_id).await,
        Request::AppstoreScriptStop { run_id } => appstore::script_stop(run_id).await,
        Request::AppstoreScriptRead { path } => appstore::script_read(path).await,
        Request::AppstoreScriptWrite { path, content } => {
            appstore::script_write(path, content).await
        }
        Request::AppstoreRunFiles { run_id } => appstore::run_files(run_id).await,
        Request::AppstoreRunFileRead { run_id, path } => {
            appstore::run_file_read(run_id, path).await
        }
        Request::AppstoreRunFileWrite {
            run_id,
            path,
            content,
        } => appstore::run_file_write(run_id, path, content).await,
        Request::AppstoreRunRetry { run_id, new_run_id } => {
            appstore::run_retry(run_id, new_run_id).await
        }
        Request::AppstoreInstalled => appstore::installed().await,
        Request::AppstoreInstanceAction { pkg_path, action } => {
            appstore::instance_action(pkg_path, action).await
        }
        Request::SiteVhostSync {
            site_id,
            name,
            domains,
            enabled,
            php_socket,
            web_root,
            log_root,
            owner_user,
        } => {
            site::vhost_sync(
                site_id, name, domains, enabled, php_socket, web_root, log_root, owner_user,
            )
            .await
        }
        Request::SiteVhostRemove { site_id, name } => site::vhost_remove(site_id, name).await,
        Request::EnvDetect => env::detect().await,
        Request::UserHomeInit { home_dir, owner } => {
            user::home_init(&home_dir, owner.as_deref()).await
        }
        Request::UserHomeMigrate {
            src_home,
            dest_home,
            owner,
        } => user::migrate_home(&src_home, &dest_home, owner.as_deref()).await,
        Request::UserSystemInit {
            linux_user,
            home_dir,
        } => user::system_init(&linux_user, &home_dir).await,
        Request::UserSystemRemove { linux_user } => user::system_remove(&linux_user).await,
        Request::PhpPoolSync {
            php_instance,
            linux_user,
            home_dir,
            spec,
        } => php::pool_sync(php_instance, linux_user, home_dir, spec).await,
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
