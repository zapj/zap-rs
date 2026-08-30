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
