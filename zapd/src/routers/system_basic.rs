//! 面板基础设置（系统设置 → 基础设置，仅 admin）。
//!
//! 三个 Tab 的配置统一存储于 server_env(scope='conf')，键名带 `basic.` 前缀，
//! 与运行环境的默认配置（webserver / php_default 等）互不干扰。
//!
//! 端点：
//! - GET  /system/config/basic   读取基础 / Mail / 联系信息
//! - POST /system/config/basic   保存（支持按 Tab 部分提交，未传字段保持不变；
//!   Mail 密码留空表示不改动原密码）

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::{Json, extract::Extension};
use serde::Deserialize;
use serde_json::json;

use crate::db;
use crate::zap::ZapError;
use crate::zap::ZapJsonResult;
use crate::zap::audit;
use crate::zap::jwt::ValidatedClaims;
use crate::zap::jwt::is_admin;

// ── 键定义 ──────────────────────────────────────────────────

const CONF_SCOPE: &str = "conf";

/// 基础设置（建站默认网络）：默认 IPv4 / 默认 IPv6 / 网络设备。
/// 站点创建时未指定 IP 则使用这里的默认值。
const K_IPV4: &str = "basic_default_ipv4";
const K_IPV6: &str = "basic_default_ipv6";
const K_IFACE: &str = "basic_network_iface";

/// Mail（发信参数，供后续系统通知 / 工单邮件发送使用）。
const K_MAIL_HOST: &str = "basic_mail_host";
const K_MAIL_PORT: &str = "basic_mail_port";
const K_MAIL_ENCRYPTION: &str = "basic_mail_encryption";
const K_MAIL_FROM: &str = "basic_mail_from";
const K_MAIL_USERNAME: &str = "basic_mail_username";
const K_MAIL_PASSWORD: &str = "basic_mail_password";

/// 联系信息（面板对外展示的客服 / 联系方式）。
const K_CONTACT_NAME: &str = "basic_contact_name";
const K_CONTACT_EMAIL: &str = "basic_contact_email";
const K_CONTACT_QQ: &str = "basic_contact_qq";
const K_CONTACT_WECHAT: &str = "basic_contact_wechat";
const K_CONTACT_PHONE: &str = "basic_contact_phone";
const K_CONTACT_REMARK: &str = "basic_contact_remark";

/// 读取 scope='conf' 全部键值。
async fn load_conf() -> HashMap<String, String> {
    let pool = db::get_db_pool().await;
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT k, v FROM server_env WHERE scope = ?")
        .bind(CONF_SCOPE)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.into_iter().collect()
}

fn get(conf: &HashMap<String, String>, key: &str) -> String {
    conf.get(key).cloned().unwrap_or_default()
}

// ── handlers ────────────────────────────────────────────────

/// GET /system/config/basic
pub async fn basic_get(claims: ValidatedClaims) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可查看基础设置".to_string()));
    }
    let conf = load_conf().await;
    Ok(Json(json!({
        "code": 0,
        "message": "OK",
        "data": {
            "basic": {
                "ipv4": get(&conf, K_IPV4),
                "ipv6": get(&conf, K_IPV6),
                "iface": get(&conf, K_IFACE),
            },
            "mail": {
                "host": get(&conf, K_MAIL_HOST),
                "port": get(&conf, K_MAIL_PORT),
                "encryption": get(&conf, K_MAIL_ENCRYPTION),
                "from": get(&conf, K_MAIL_FROM),
                "username": get(&conf, K_MAIL_USERNAME),
                // 密码仅写入不回显，避免泄露
                "password": "",
            },
            "contact": {
                "name": get(&conf, K_CONTACT_NAME),
                "email": get(&conf, K_CONTACT_EMAIL),
                "qq": get(&conf, K_CONTACT_QQ),
                "wechat": get(&conf, K_CONTACT_WECHAT),
                "phone": get(&conf, K_CONTACT_PHONE),
                "remark": get(&conf, K_CONTACT_REMARK),
            },
        }
    })))
}

#[derive(Debug, Default, Deserialize)]
pub struct BasicPane {
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub iface: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct MailPane {
    pub host: Option<String>,
    pub port: Option<String>,
    /// ssl / tls(starttls) / none
    pub encryption: Option<String>,
    pub from: Option<String>,
    pub username: Option<String>,
    /// 留空 = 不修改原密码
    pub password: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ContactPane {
    pub name: Option<String>,
    pub email: Option<String>,
    pub qq: Option<String>,
    pub wechat: Option<String>,
    pub phone: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct BasicSavePayload {
    pub basic: Option<BasicPane>,
    pub mail: Option<MailPane>,
    pub contact: Option<ContactPane>,
}

/// POST /system/config/basic
pub async fn basic_save(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
    Json(payload): Json<BasicSavePayload>,
) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可修改基础设置".to_string()));
    }

    let mut upserts: Vec<(String, String)> = Vec::new();

    // 收集某可选项：Some(v) 即写入（v 可为空串 = 清空该键）
    fn push_opt(
        upserts: &mut Vec<(String, String)>,
        field: &Option<String>,
        key: &'static str,
        limit: usize,
    ) -> Result<(), ZapError> {
        if let Some(v) = field {
            let v = v.trim().to_string();
            if v.len() > limit {
                return Err(ZapError::New(
                    -1,
                    format!("「{key}」长度超限（最大 {limit} 字符）"),
                ));
            }
            upserts.push((key.to_string(), v));
        }
        Ok(())
    }

    if let Some(b) = &payload.basic {
        push_opt(&mut upserts, &b.ipv4, K_IPV4, 64)?;
        push_opt(&mut upserts, &b.ipv6, K_IPV6, 64)?;
        push_opt(&mut upserts, &b.iface, K_IFACE, 32)?;
    }
    if let Some(m) = &payload.mail {
        push_opt(&mut upserts, &m.host, K_MAIL_HOST, 128)?;
        push_opt(&mut upserts, &m.port, K_MAIL_PORT, 8)?;
        if let Some(e) = &m.encryption {
            let e = e.trim().to_string();
            if !["ssl", "tls", "none"].contains(&e.as_str()) {
                return Err(ZapError::New(
                    -1,
                    "加密方式仅支持 ssl / tls / none".to_string(),
                ));
            }
            upserts.push((K_MAIL_ENCRYPTION.to_string(), e));
        }
        push_opt(&mut upserts, &m.from, K_MAIL_FROM, 128)?;
        push_opt(&mut upserts, &m.username, K_MAIL_USERNAME, 128)?;
        if let Some(p) = &m.password {
            let p = p.trim().to_string();
            if !p.is_empty() {
                upserts.push((K_MAIL_PASSWORD.to_string(), p));
            }
        }
    }
    if let Some(c) = &payload.contact {
        push_opt(&mut upserts, &c.name, K_CONTACT_NAME, 64)?;
        push_opt(&mut upserts, &c.email, K_CONTACT_EMAIL, 128)?;
        push_opt(&mut upserts, &c.qq, K_CONTACT_QQ, 64)?;
        push_opt(&mut upserts, &c.wechat, K_CONTACT_WECHAT, 64)?;
        push_opt(&mut upserts, &c.phone, K_CONTACT_PHONE, 32)?;
        push_opt(&mut upserts, &c.remark, K_CONTACT_REMARK, 512)?;
    }

    if !upserts.is_empty() {
        let pool = db::get_db_pool().await;
        let now = chrono::Local::now().timestamp();
        for (k, v) in &upserts {
            let _ = sqlx::query(
                "INSERT INTO server_env (scope, k, v, remark, updated_at) VALUES ('conf', ?, ?, '面板基础设置', ?)
                 ON CONFLICT(scope, k) DO UPDATE SET v = excluded.v, updated_at = excluded.updated_at",
            )
            .bind(k)
            .bind(v)
            .bind(now)
            .execute(pool)
            .await;
        }
    }

    let detail = upserts
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let detail_str: &str = if detail.is_empty() {
        "未提交任何配置"
    } else {
        detail.as_str()
    };
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "basic_config_save",
        "system",
        detail_str,
    )
    .await;

    Ok(Json(json!({ "code": 0, "message": "基础设置已保存" })))
}
