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
        key_type: Option<String>,
        bits: Option<u32>,
        comment: Option<String>,
    },
    /// 导入 SSH 密钥
    #[serde(rename = "ssh_key.import")]
    SshKeyImport {
        name: String,
        private_key: String,
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
    FileUpload { path: String, name: String, content: String },
    /// 文件信息
    #[serde(rename = "file.info")]
    FileInfo { path: String },
    /// 更新 AppStore 软件库（git clone/pull 或 zip 下载解压）
    #[serde(rename = "appstore.repo_update")]
    AppstoreRepoUpdate {
        source_type: String,
        source_url: String,
        sha256: Option<String>,
        run_id: String,
    },
    /// 安装包：执行包的 bin.sh（或 app.yaml 指定脚本）
    #[serde(rename = "appstore.install")]
    AppstoreInstall {
        /// 形如 database/mariadb 的包相对路径
        pkg_path: String,
        /// custom | official
        source: String,
        version: String,
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
        version: String,
        old_version: String,
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
    /// 扫描已安装包列表（data/apps/*/meta.yaml）
    #[serde(rename = "appstore.installed")]
    AppstoreInstalled,
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
                version: "11.4.4".into(),
                run_id: "r1".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.install","pkg_path":"database/mariadb","source":"official","version":"11.4.4","run_id":"r1"}"#
        );
        assert_eq!(
            serde_json::to_string(&Request::AppstoreRepoUpdate {
                source_type: "zip".into(),
                source_url: "https://example.com/store.zip".into(),
                sha256: Some("abc".into()),
                run_id: "r9".into(),
            })
            .unwrap(),
            r#"{"verb":"appstore.repo_update","source_type":"zip","source_url":"https://example.com/store.zip","sha256":"abc","run_id":"r9"}"#
        );
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
        assert!(!json.contains("data"), "data=None 时不应序列化该字段: {json}");
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
        let m: Message =
            serde_json::from_str(r#"{"type":"challenge","challenge":"x"}"#).unwrap();
        assert!(matches!(m, Message::Challenge { challenge } if challenge == "x"));
    }
}
