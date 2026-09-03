use serde::{Deserialize, Serialize};

/// `zapd` -> `zapexec` 的请求。只有白名单动词，刻意不提供任意 shell 执行。
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum Request {
    /// 同步系统时钟（chrony / ntpdate）
    #[serde(rename = "time.sync")]
    TimeSync,
    /// 设置系统时区
    #[serde(rename = "time.set_timezone")]
    TimeSetTimezone { timezone: String },
    /// 列出可用时区
    #[serde(rename = "time.list_timezones")]
    TimeListTimezones,
    /// 读取当前时间/时区
    #[serde(rename = "time.get")]
    TimeGet,
    /// 读取 SSH 服务状态
    #[serde(rename = "ssh.status")]
    SshStatus,
    /// 重启 SSH 服务
    #[serde(rename = "ssh.restart")]
    SshRestart,
    /// 安装 SSH 服务端（openssh-server），日志写 run-{id}.log
    #[serde(rename = "ssh.install")]
    SshInstall { run_id: String },
    /// 列出系统服务（systemd）
    #[serde(rename = "service.list")]
    ServiceList,
    /// 对系统服务执行操作（start/stop/restart/reload/enable/disable）
    #[serde(rename = "service.action")]
    ServiceAction { name: String, action: String },
    /// 列出系统进程（ps）
    #[serde(rename = "process.list")]
    ProcessList,
    /// 终止进程（signal 缺省为 TERM，9 表示 KILL）
    #[serde(rename = "process.kill")]
    ProcessKill { pid: u32, signal: Option<String> },
    /// 列出 SSH 密钥
    #[serde(rename = "ssh_key.list")]
    SshKeyList,
    /// 读取公钥内容
    #[serde(rename = "ssh_key.get")]
    SshKeyGet { name: String },
    /// 生成 SSH 密钥
    #[serde(rename = "ssh_key.generate")]
    SshKeyGenerate {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        key_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bits: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        comment: Option<String>,
    },
    /// 导入 SSH 密钥
    #[serde(rename = "ssh_key.import")]
    SshKeyImport {
        name: String,
        private_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        public_key: Option<String>,
    },
    /// 删除 SSH 密钥
    #[serde(rename = "ssh_key.delete")]
    SshKeyDelete { name: String },
    /// 列出 authorized_keys 条目
    #[serde(rename = "ssh_key.authorized_list")]
    SshKeyAuthorizedList,
    /// 授权密钥（加入 authorized_keys）
    #[serde(rename = "ssh_key.authorize")]
    SshKeyAuthorize { name: String },
    /// 取消授权（按 authorized_keys 索引）
    #[serde(rename = "ssh_key.deauthorize")]
    SshKeyDeauthorize { index: usize },
    /// 把公钥写入本机系统用户的 ~/.ssh/authorized_keys（root 特权，仅本地回环连接用）
    #[serde(rename = "ssh_key.install_local")]
    SshKeyInstallLocal { username: String, key_name: String },
    /// 读取主机名与 DNS 解析器配置
    #[serde(rename = "network.get")]
    NetworkGet,
    /// 设置主机名
    #[serde(rename = "network.set_hostname")]
    NetworkSetHostname { hostname: String },
    /// 设置 DNS Resolver（nameserver / search 写入 /etc/resolv.conf）
    #[serde(rename = "network.set_resolver")]
    NetworkSetResolver {
        nameservers: Vec<String>,
        #[serde(default)]
        search: Vec<String>,
    },
    /// 列出目录
    #[serde(rename = "file.list")]
    FileList { path: String },
    /// 读文件（文本）
    #[serde(rename = "file.read")]
    FileRead { path: String },
    /// 写文件（文本）
    #[serde(rename = "file.write")]
    FileWrite { path: String, content: String },
    /// 删除文件/目录
    #[serde(rename = "file.delete")]
    FileDelete { path: String },
    /// 建目录
    #[serde(rename = "file.mkdir")]
    FileMkdir { path: String },
    /// 重命名
    #[serde(rename = "file.rename")]
    FileRename { path: String, new_path: String },
    /// 下载（base64 字节）
    #[serde(rename = "file.download")]
    FileDownload { path: String },
    /// 上传（base64 字节）
    #[serde(rename = "file.upload")]
    FileUpload {
        path: String,
        name: String,
        content: String,
    },
    /// 文件信息
    #[serde(rename = "file.info")]
    FileInfo { path: String },
    /// 添加 AppStore Git 源（clone 到 data/appstore/repos/<id>/）
    #[serde(rename = "appstore.repo_add")]
    AppstoreRepoAdd {
        name: String,
        url: String,
        run_id: String,
    },
    /// 删除 AppStore Git 源（内置源禁止删除）
    #[serde(rename = "appstore.repo_remove")]
    AppstoreRepoRemove { id: String },
    /// 更新单个 AppStore Git 源（clone/fetch + reset）
    #[serde(rename = "appstore.repo_update")]
    AppstoreRepoUpdate { id: String, run_id: String },
    /// 安装包：执行包的 bin.sh（或 app.yaml 指定脚本）
    #[serde(rename = "appstore.install")]
    AppstoreInstall {
        /// 形如 database/mariadb 的包相对路径
        pkg_path: String,
        /// custom | official
        source: String,
        /// 包来源的 git 源 id（source=official 时定位 repos/<repo_id>/）
        #[serde(skip_serializing_if = "Option::is_none")]
        repo_id: Option<String>,
        version: String,
        /// 用户点击的操作（app.yaml actions 键，如 bin/build），透传给安装脚本的 ACTION 环境变量
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        run_id: String,
    },
    /// 卸载包：执行 uninstall.sh 并删除已安装目录
    #[serde(rename = "appstore.uninstall")]
    AppstoreUninstall { pkg_path: String, run_id: String },
    /// 升级包：执行 upgrade.sh（缺省时先 uninstall.sh 再 bin.sh）
    #[serde(rename = "appstore.upgrade")]
    AppstoreUpgrade {
        pkg_path: String,
        source: String,
        /// 包来源的 git 源 id（source=official 时定位 repos/<repo_id>/）
        #[serde(skip_serializing_if = "Option::is_none")]
        repo_id: Option<String>,
        version: String,
        old_version: String,
        /// 用户点击的操作（app.yaml actions 键），透传给升级脚本的 ACTION 环境变量
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        run_id: String,
    },
    /// 运行自定义脚本（仅限 appstore/custom/ 内）
    #[serde(rename = "appstore.script_run")]
    AppstoreScriptRun { path: String, run_id: String },
    /// 停止运行中的任务（按 run_id 杀进程组）
    #[serde(rename = "appstore.script_stop")]
    AppstoreScriptStop { run_id: String },
    /// 读取 appstore 内脚本内容（编辑前读取）
    #[serde(rename = "appstore.script_read")]
    AppstoreScriptRead { path: String },
    /// 写自定义脚本（仅限 appstore/custom/ 内）
    #[serde(rename = "appstore.script_write")]
    AppstoreScriptWrite { path: String, content: String },
    /// 扫描已安装应用列表（data/apps/*/meta.yaml + info.yaml + 运行状态）
    #[serde(rename = "appstore.installed")]
    AppstoreInstalled,
    /// 对已安装应用的实例执行启停（start/stop/restart，走其登记的 systemd 服务）
    #[serde(rename = "appstore.instance_action")]
    AppstoreInstanceAction {
        /// 形如 application/php 的包路径
        pkg_path: String,
        /// start | stop | restart
        action: String,
    },
    /// 同步站点 Nginx vhost：按站点渲染 conf 文件并 reload（幂等）
    #[serde(rename = "site.vhost_sync")]
    SiteVhostSync {
        /// site 表主键（vhost 文件名 zap-site-{id}.conf）
        site_id: i64,
        /// 站点名称（sanitize 后用于文档根目录名）
        name: String,
        /// 站点域名列表（server_name，多个以空格分隔）
        domains: Vec<String>,
        /// true 生成 vhost；false 移除 vhost（站点停用）
        enabled: bool,
        /// PHP-FPM 通道（如 unix:/var/run/php-fpm-8.3.sock 或 127.0.0.1:9000）；
        /// None 表示纯静态站点，不生成 PHP location
        #[serde(skip_serializing_if = "Option::is_none")]
        php_socket: Option<String>,
        /// 站点文档根目录（面板按归属用户家目录规划并入库，如 /home/u/www/blog-1）；
        /// None 时回退 {ZAP_PATH}/data/www/{sanitize(name)}-{site_id}
        #[serde(default, skip_serializing_if = "Option::is_none")]
        web_root: Option<String>,
        /// 站点日志目录（access.log / error.log 所在）；
        /// None 时 vhost 不生成日志指令（沿用 nginx 全局日志）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        log_root: Option<String>,
        /// 站点文件属主（Linux 账号名）；Some 时 web_root/log_root 整树
        /// chown {owner}:www 并收紧权限（目录 750 / 文件 640），
        /// None（默认 www 运行模式）则归 www:www
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner_user: Option<String>,
    },
    /// 移除站点 Nginx vhost（站点删除时清理，幂等）
    #[serde(rename = "site.vhost_remove")]
    SiteVhostRemove { site_id: i64, name: String },
    /// 探测服务器运行环境快照（OS / Web 服务器 / PHP / 数据库 / 工具链）
    #[serde(rename = "env.detect")]
    EnvDetect,
    /// 初始化面板用户家目录骨架：mkdir -p {home_dir}/www {home_dir}/logs（root 特权）。
    /// owner 为 Some(linux 账号) 时按「独立系统用户」模式设置属主与权限：
    /// home 711 owner {u}:{u}，www / logs 750 owner {u}:www；
    /// owner None（默认 www 模式）时 www / logs 归 www:www。
    #[serde(rename = "user.home_init")]
    UserHomeInit {
        home_dir: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<String>,
    },
    /// 为面板用户创建 Linux 系统账号（useradd，nologin，home 指向 home_dir），幂等。
    /// 虚拟主机运行模式为「独立系统用户」时，面板用户在 user.add / 站点同步前调用。
    #[serde(rename = "user.system_init")]
    UserSystemInit {
        /// Linux 账号名（须通过 zap_proto::linux_username 派生，调用方已校验）
        linux_user: String,
        /// 账号 home 目录（面板记录的 home_dir）
        home_dir: String,
    },
    /// 移除 Linux 系统账号（删除面板用户 / 切换回 www 模式时调用）。
    /// 会先清掉该用户的 PHP-FPM pool 配置文件并 reload，再 userdel。
    #[serde(rename = "user.system_remove")]
    UserSystemRemove {
        /// Linux 账号名
        linux_user: String,
    },
    /// 按用户生成 PHP-FPM pool 配置并 reload（幂等）：
    /// 写入 {php 安装}/etc/php-fpm.d/{linux_user}.conf，
    /// listen unix:/var/run/php-fpm-{linux_user}-{php版本}.sock，worker 以 {linux_user} 运行，
    /// 规格来自 spec（JSON 字符串，缺省用面板默认值）。
    #[serde(rename = "php.pool_sync")]
    PhpPoolSync {
        /// PHP 实例标识（如 php8.3）
        php_instance: String,
        /// Linux 账号名（worker 运行身份 / pool 名 / socket 名）
        linux_user: String,
        /// 用户家目录（open_basedir / session / upload 隔离根）
        home_dir: String,
        /// fpm pool 规格 JSON 字符串；空 = 使用默认规格
        spec: String,
    },
}

/// `zapexec` -> `zapd` 的响应。
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    /// 0 = 成功，非 0 = 错误（沿用 `ZapError` 的 code 约定）
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Response {
    pub fn ok(message: impl Into<String>, data: Option<serde_json::Value>) -> Self {
        Self {
            code: 0,
            message: message.into(),
            data,
        }
    }

    pub fn err(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// 握手与数据阶段共用的消息封装。
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// server -> client：随机挑战（hex）
    Challenge { challenge: String },
    /// client -> server：HMAC-SHA256(secret, challenge) 的 hex
    Auth { mac: String },
    /// server -> client：握手成功
    Welcome,
    /// client -> server：请求
    Request(Request),
    /// server -> client：响应
    Response(Response),
}

/// 站点/目录名安全规范化（zapd 与 zapexec 共用）：
/// 仅保留 ASCII 字母数字与 `_`/`-`，其余（含 `.`、空格、路径分隔符等）替换为 `-`，
/// 再去除首尾 `-`；空结果回退 `site`，最长 48 字符。
/// 与 zapexec `verbs/site.rs` 的历史实现保持一致，避免「面板侧记录的目录名」与
/// 「执行端实际创建的目录名」分叉。
pub fn sanitize_site_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    out = out.trim_matches('-').to_string();
    if out.is_empty() {
        out = "site".to_string();
    }
    if out.chars().count() > 48 {
        out = out.chars().take(48).collect();
    }
    out
}

/// 面板用户名 → Linux 系统账号名派生（与家目录末段一致，home_dir 统一
/// `/home/{linux_username}`）：
/// 1. 经 `sanitize_site_name` 清洗；2. 转小写；
/// 3. 首位必须是 ascii 字母或 `_`（数字/`-` 开头补 `z` 前缀，保证 useradd 合法）；
/// 4. 最长 24 字符（Linux 用户名上限 32，留裕量给面板其它后缀）。
pub fn linux_username(username: &str) -> String {
    let clean = sanitize_site_name(username);
    let mut base: String = clean
        .chars()
        .take(23)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let first_ok = base
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok {
        // base 以数字或 `-` 开头（sanitize 已去首尾 `-`，这里多为数字开头/空）
        if base.is_empty() {
            base = "zapuser".to_string();
        } else {
            base.insert(0, 'z');
        }
    }
    if base.len() > 24 {
        base.truncate(24);
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_uses_whitelist_verbs() {
        assert_eq!(
            serde_json::to_string(&Request::TimeSync).unwrap(),
            r#"{"verb":"time.sync"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::SshKeyList).unwrap(),
            r#"{"verb":"ssh_key.list"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::SshKeyGenerate {
                name: "k".into(),
                key_type: Some("ed25519".into()),
                bits: None,
                comment: None,
            })
            .unwrap(),
            r#"{"verb":"ssh_key.generate","name":"k","key_type":"ed25519"}"#
        );
    }

    #[test]
    fn request_round_trip() {
        let req = Request::FileWrite {
            path: "/tmp/a.txt".into(),
            content: "hello".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{req:?}"), format!("{back:?}"));
    }

    #[test]
    fn appstore_verbs_tagging() {
        assert_eq!(
            serde_json::to_string(&Request::AppstoreInstalled).unwrap(),
            r#"{"verb":"appstore.installed"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreInstall {
                pkg_path: "database/mariadb".into(),
                source: "official".into(),
                repo_id: Some("zap-appstore".into()),
                version: "11.4.4".into(),
                action: None,
                run_id: "r1".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.install","pkg_path":"database/mariadb","source":"official","repo_id":"zap-appstore","version":"11.4.4","run_id":"r1"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreInstall {
                pkg_path: "application/php".into(),
                source: "official".into(),
                repo_id: Some("zap-appstore".into()),
                version: "8.3.3".into(),
                action: Some("build".into()),
                run_id: "r2".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.install","pkg_path":"application/php","source":"official","repo_id":"zap-appstore","version":"8.3.3","action":"build","run_id":"r2"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreRepoAdd {
                name: "My Store".into(),
                url: "https://github.com/user/store.git".into(),
                run_id: "r9".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.repo_add","name":"My Store","url":"https://github.com/user/store.git","run_id":"r9"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreRepoRemove {
                id: "my-store".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.repo_remove","id":"my-store"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreRepoUpdate {
                id: "zap-appstore".into(),
                run_id: "r9".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.repo_update","id":"zap-appstore","run_id":"r9"}"#
        );
    }

    #[test]
    fn appstore_instance_action_tagging() {
        assert_eq!(
            serde_json::to_string(&Request::AppstoreInstanceAction {
                pkg_path: "application/php".into(),
                action: "stop".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.instance_action","pkg_path":"application/php","action":"stop"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::SiteVhostSync {
                site_id: 1,
                name: "blog".into(),
                domains: vec!["a.com".into(), "b.com".into()],
                enabled: true,
                php_socket: Some("unix:/var/run/php-fpm-8.3.sock".into()),
                web_root: None,
                log_root: None,
                owner_user: None,
            })
            .unwrap(),
            r#"{"verb":"site.vhost_sync","site_id":1,"name":"blog","domains":["a.com","b.com"],"enabled":true,"php_socket":"unix:/var/run/php-fpm-8.3.sock"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::SiteVhostRemove {
                site_id: 2,
                name: "x".into(),
            })
            .unwrap(),
            r#"{"verb":"site.vhost_remove","site_id":2,"name":"x"}"#
        );
    }

    #[test]
    fn env_verb_tagging() {
        assert_eq!(
            serde_json::to_string(&Request::EnvDetect).unwrap(),
            r#"{"verb":"env.detect"}"#
        );
        // env.detect 需能经 serde 反序列化回来（白名单内部路由用）
        let back: Request = serde_json::from_str(r#"{"verb":"env.detect"}"#).unwrap();
        assert!(matches!(back, Request::EnvDetect));
    }

    #[test]
    fn site_vhost_sync_with_dirs() {
        let req = Request::SiteVhostSync {
            site_id: 1,
            name: "blog".into(),
            domains: vec!["a.com".into()],
            enabled: true,
            php_socket: None,
            web_root: Some("/home/zap/www/blog-1".into()),
            log_root: Some("/home/zap/logs/blog-1".into()),
            owner_user: Some("zap".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(
            json,
            r#"{"verb":"site.vhost_sync","site_id":1,"name":"blog","domains":["a.com"],"enabled":true,"web_root":"/home/zap/www/blog-1","log_root":"/home/zap/logs/blog-1","owner_user":"zap"}"#
        );
        // 老版本 JSON（无 web_root/log_root）也能反序列化成功 → None
        let old: Request =
            serde_json::from_str(r#"{"verb":"site.vhost_sync","site_id":1,"name":"blog","domains":["a.com"],"enabled":true,"php_socket":"unix:/var/run/php-fpm-8.3.sock"}"#)
                .unwrap();
        match old {
            Request::SiteVhostSync {
                web_root, log_root, ..
            } => {
                assert!(web_root.is_none());
                assert!(log_root.is_none());
            }
            _ => panic!("应解析为 SiteVhostSync"),
        }
    }

    #[test]
    fn user_home_init_tagging() {
        assert_eq!(
            serde_json::to_string(&Request::UserHomeInit {
                home_dir: "/home/zap".into(),
                owner: None,
            })
            .unwrap(),
            r#"{"verb":"user.home_init","home_dir":"/home/zap"}"#
        );
        let back: Request =
            serde_json::from_str(r#"{"verb":"user.home_init","home_dir":"/home/zap"}"#).unwrap();
        assert!(
            matches!(back, Request::UserHomeInit { home_dir, owner: None } if home_dir == "/home/zap")
        );
    }

    #[test]
    fn sanitize_site_name_helper() {
        assert_eq!(sanitize_site_name("我的 博客"), "site");
        assert_eq!(sanitize_site_name("my blog/x"), "my-blog-x");
        assert_eq!(sanitize_site_name(".."), "site");
        assert_eq!(sanitize_site_name("ABC_123"), "ABC_123");
        // 最长 48
        let long = "a".repeat(60);
        assert_eq!(sanitize_site_name(&long).chars().count(), 48);
    }

    #[test]
    fn linux_username_helper() {
        assert_eq!(linux_username("zap"), "zap");
        assert_eq!(linux_username("Zap_Admin"), "zap_admin");
        // 全非法 → sanitize 回退 site → 小写 site
        assert_eq!(linux_username("我的 博客"), "site");
        assert_eq!(linux_username("123abc"), "z123abc");
        // 前导 - 被 sanitize 修剪
        assert_eq!(linux_username("-x"), "x");
        let long = "A".repeat(40);
        let got = linux_username(&long);
        assert!(got.len() <= 24);
        assert!(got.chars().next().unwrap().is_ascii_alphabetic());
    }

    #[test]
    fn php_pool_sync_tagging() {
        let req = Request::PhpPoolSync {
            php_instance: "php8.3".into(),
            linux_user: "zap".into(),
            home_dir: "/home/zap".into(),
            spec: "{\"pm\":\"dynamic\",\"max_children\":8}".into(),
        };
        let back: Request = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(format!("{req:?}"), format!("{back:?}"));
    }

    #[test]
    fn appstore_verbs_round_trip() {
        let req = Request::AppstoreScriptWrite {
            path: "scripts/admin/backup.sh".into(),
            content: "#!/bin/bash\necho hi".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(format!("{req:?}"), format!("{back:?}"));
    }

    #[test]
    fn unknown_verb_rejected() {
        // 白名单之外（例如任意 shell 执行）必须被拒绝
        let err = serde_json::from_str::<Request>(r#"{"verb":"shell.exec","cmd":"id"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn missing_field_rejected() {
        let err = serde_json::from_str::<Request>(r#"{"verb":"file.read"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn response_helpers() {
        let ok = Response::ok("done", Some(serde_json::json!({ "a": 1 })));
        assert_eq!(ok.code, 0);
        assert_eq!(ok.message, "done");
        assert!(ok.data.is_some());

        let err = Response::err(7, "boom");
        assert_eq!(err.code, 7);
        assert_eq!(err.message, "boom");
        assert!(err.data.is_none());
    }

    #[test]
    fn response_omits_none_data() {
        let err = Response::err(1, "x");
        let json = serde_json::to_string(&err).unwrap();
        assert!(
            !json.contains("data"),
            "data=None 时不应序列化该字段: {json}"
        );
    }

    #[test]
    fn message_round_trip_all_variants() {
        let msgs = vec![
            Message::Challenge {
                challenge: "abc".into(),
            },
            Message::Auth { mac: "def".into() },
            Message::Welcome,
            Message::Request(Request::TimeSetTimezone {
                timezone: "Asia/Shanghai".into(),
            }),
            Message::Response(Response::ok("ok", None)),
        ];
        for m in msgs {
            let json = serde_json::to_string(&m).unwrap();
            let back: Message = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{m:?}"), format!("{back:?}"), "json: {json}");
        }
    }

    #[test]
    fn message_type_tagging() {
        assert_eq!(
            serde_json::to_string(&Message::Welcome).unwrap(),
            r#"{"type":"welcome"}"#
        );
        let m: Message = serde_json::from_str(r#"{"type":"challenge","challenge":"x"}"#).unwrap();
        assert!(matches!(m, Message::Challenge { challenge } if challenge == "x"));
    }
}
