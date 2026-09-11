//! 「我的 SSH 密钥」：面板用户自建 SSH 密钥（普通用户到 admin 均可）。
//!
//! 密钥私钥文件只保存在用户自己家目录 `~/.ssh/zap_<name>`（见 zapexec `ssh_user_key`
//! verb），本模块只做归属校验 + 元数据（公钥/指纹/注释）入库 + 转发文件操作。
//!
//! 可见性规则与连接一致（严格隔离）：任何角色只管理自己名下的密钥。
//! admin 额外可见/可选旧的系统级密钥（`/etc/zap/ssh`，服务器侧），保证历史连接可用。

use axum::Json;
use axum::extract::Query;
use serde::Deserialize;
use serde_json::{Value, json};

use zap_proto::Request;

use crate::db;
use crate::zap::jwt::{ValidatedClaims, is_admin};
use crate::zap::{ZapError, ZapJsonResult};

/// 校验密钥名：字母数字开头，仅允许字母数字 `-` `_`，最长 64（与 zapexec 一致）。
fn valid_key_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    let mut chars = name.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphanumeric());
    first_ok
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// 当前登录用户绑定的系统用户（linux_user）。无绑定则拒绝密钥管理。
async fn require_linux_user(claims: &ValidatedClaims) -> Result<String, ZapError> {
    let pool = db::get_db_pool().await;
    let lu: Option<String> = sqlx::query_scalar("SELECT linux_user FROM user WHERE id = ?")
        .bind(claims.id as i64)
        .fetch_optional(pool)
        .await?;
    match lu {
        Some(u) if !u.is_empty() => Ok(u),
        _ => Err(ZapError::New(
            -1,
            "当前账号未绑定系统用户，无法管理 SSH 密钥".to_string(),
        )),
    }
}

/// 确认密钥属于当前用户，返回 (public_key, comment, fingerprint)。
async fn own_key_row(
    claims: &ValidatedClaims,
    name: &str,
) -> Result<(String, String, String), ZapError> {
    let pool = db::get_db_pool().await;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT public_key, comment, fingerprint FROM user_ssh_keys WHERE user_id = ? AND name = ?",
    )
    .bind(claims.id as i64)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    row.ok_or_else(|| ZapError::New(-1, format!("SSH 密钥 '{name}' 不存在")))
}

// ── GET /terminal/keys ─────────────────────────────────────

/// 我的密钥列表：本人 + (admin) 系统级密钥。
/// data.items：密钥列表；另返回 vhost_mode / user_keys_enabled 兼容字段
/// （运行模式固定为独立系统用户，两字段恒为 "system" / true）。
pub async fn list_keys(claims: ValidatedClaims) -> ZapJsonResult {
    let mut items: Vec<Value> = Vec::new();

    // 本人「我的密钥」：密钥文件落在该 Linux 账号家目录的 ~/.ssh 下
    {
        let pool = db::get_db_pool().await;
        let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
            "SELECT name, comment, fingerprint, created_at FROM user_ssh_keys
             WHERE user_id = ? ORDER BY id DESC",
        )
        .bind(claims.id as i64)
        .fetch_all(pool)
        .await?;
        items.extend(
            rows.into_iter()
                .map(|(name, comment, fingerprint, created_at)| {
                    json!({
                        "name": name,
                        "scope": "user",
                        "comment": comment,
                        "fingerprint": fingerprint,
                        "created_at": created_at,
                    })
                }),
        );
    }

    // admin 额外展示系统级密钥（/etc/zap/ssh，服务器侧），仅名称/指纹等元数据；
    // 与运行模式无关：本机回环授权 / 历史连接 / 推送公钥仍可引用
    if is_admin(&claims)
        && let Ok(resp) = crate::zapexec::call(Request::SshKeyList).await
        && resp.code == 0
        && let Some(arr) = resp.data.as_ref().and_then(|d| d.as_array())
    {
        for v in arr {
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let comment = v
                .get("comment")
                .and_then(|x| x.as_str())
                .unwrap_or("系统级密钥")
                .to_string();
            let fingerprint = v
                .get("fingerprint")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            items.push(json!({
                "name": name,
                "scope": "system",
                "comment": comment,
                "fingerprint": fingerprint,
                "created_at": 0,
            }));
        }
    }

    Ok(Json(json!({
        "code": 0,
        "data": {
            "items": items,
            "vhost_mode": "system",
            "user_keys_enabled": true,
        }
    })))
}

// ── POST /terminal/keys/generate ───────────────────────────

#[derive(Debug, Deserialize)]
pub struct GeneratePayload {
    pub name: String,
    #[serde(default)]
    pub key_type: Option<String>,
    #[serde(default)]
    pub bits: Option<u32>,
    #[serde(default)]
    pub comment: Option<String>,
}

pub async fn generate_key(
    claims: ValidatedClaims,
    Json(payload): Json<GeneratePayload>,
) -> ZapJsonResult {
    let name = payload.name.trim().to_string();
    if !valid_key_name(&name) {
        return Err(ZapError::New(
            -1,
            "无效的密钥名称（仅允许字母/数字/-/_，最长 64 字符）".to_string(),
        ));
    }
    let linux_user = require_linux_user(&claims).await?;
    let pool = db::get_db_pool().await;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM user_ssh_keys WHERE user_id = ? AND name = ?")
            .bind(claims.id as i64)
            .bind(&name)
            .fetch_optional(pool)
            .await?;
    if exists.is_some() {
        return Err(ZapError::New(-1, format!("同名密钥 '{name}' 已存在")));
    }

    let resp = crate::zapexec::call(Request::SshUserKeyGenerate {
        linux_user: linux_user.clone(),
        name: name.clone(),
        key_type: payload.key_type,
        bits: payload.bits,
        comment: payload.comment.clone(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    let data = resp.data.unwrap_or(Value::Null);
    let public_key = data
        .get("public_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let fingerprint = data
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let comment = payload
        .comment
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| name.clone());

    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO user_ssh_keys (user_id, name, public_key, comment, fingerprint, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(claims.id as i64)
    .bind(&name)
    .bind(&public_key)
    .bind(&comment)
    .bind(&fingerprint)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    audit_log(
        &claims,
        "ssh_user_key_generate",
        &format!("生成密钥 {name}（{linux_user}）"),
    )
    .await;
    Ok(Json(
        json!({ "code": 0, "message": "密钥已生成并保存到我的家目录 ~/.ssh" }),
    ))
}

// ── POST /terminal/keys/import ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ImportPayload {
    pub name: String,
    pub private_key: String,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

pub async fn import_key(
    claims: ValidatedClaims,
    Json(payload): Json<ImportPayload>,
) -> ZapJsonResult {
    let name = payload.name.trim().to_string();
    if !valid_key_name(&name) {
        return Err(ZapError::New(
            -1,
            "无效的密钥名称（仅允许字母/数字/-/_，最长 64 字符）".to_string(),
        ));
    }
    let linux_user = require_linux_user(&claims).await?;
    let pool = db::get_db_pool().await;
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM user_ssh_keys WHERE user_id = ? AND name = ?")
            .bind(claims.id as i64)
            .bind(&name)
            .fetch_optional(pool)
            .await?;
    if exists.is_some() {
        return Err(ZapError::New(-1, format!("同名密钥 '{name}' 已存在")));
    }

    let resp = crate::zapexec::call(Request::SshUserKeyImport {
        linux_user: linux_user.clone(),
        name: name.clone(),
        private_key: payload.private_key,
        public_key: payload.public_key.clone(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    let data = resp.data.unwrap_or(Value::Null);
    let public_key = data
        .get("public_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let fingerprint = data
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let comment = payload
        .comment
        .filter(|c| !c.trim().is_empty())
        .unwrap_or_else(|| name.clone());

    let now = chrono::Utc::now().timestamp();
    sqlx::query(
        "INSERT INTO user_ssh_keys (user_id, name, public_key, comment, fingerprint, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(claims.id as i64)
    .bind(&name)
    .bind(&public_key)
    .bind(&comment)
    .bind(&fingerprint)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    audit_log(
        &claims,
        "ssh_user_key_import",
        &format!("导入密钥 {name}（{linux_user}）"),
    )
    .await;
    Ok(Json(
        json!({ "code": 0, "message": "密钥已导入并保存到我的家目录 ~/.ssh" }),
    ))
}

// ── POST /terminal/keys/delete ─────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DeletePayload {
    pub name: String,
}

pub async fn delete_key(
    claims: ValidatedClaims,
    Json(payload): Json<DeletePayload>,
) -> ZapJsonResult {
    let name = payload.name.trim().to_string();
    own_key_row(&claims, &name).await?; // 归属校验 + 存在性
    let linux_user = require_linux_user(&claims).await?;
    let resp = crate::zapexec::call(Request::SshUserKeyDelete {
        linux_user,
        name: name.clone(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    let pool = db::get_db_pool().await;
    sqlx::query("DELETE FROM user_ssh_keys WHERE user_id = ? AND name = ?")
        .bind(claims.id as i64)
        .bind(&name)
        .execute(pool)
        .await?;

    audit_log(&claims, "ssh_user_key_delete", &format!("删除密钥 {name}")).await;
    Ok(Json(json!({ "code": 0, "message": "密钥已删除" })))
}

// ── GET /terminal/keys/public?name= ────────────────────────

#[derive(Debug, Deserialize)]
pub struct KeyNameQuery {
    pub name: String,
}

pub async fn public_key(claims: ValidatedClaims, Query(q): Query<KeyNameQuery>) -> ZapJsonResult {
    let (public_key, comment, fingerprint) = own_key_row(&claims, q.name.trim()).await?;
    Ok(Json(
        json!({ "code": 0, "data": { "public_key": public_key, "comment": comment, "fingerprint": fingerprint } }),
    ))
}

// ── GET /terminal/keys/private?name= ───────────────────────

/// 下载自己的私钥（经 zapexec root 读取家目录文件，仅本人可操作）。
pub async fn private_key(claims: ValidatedClaims, Query(q): Query<KeyNameQuery>) -> ZapJsonResult {
    let name = q.name.trim().to_string();
    own_key_row(&claims, &name).await?;
    let linux_user = require_linux_user(&claims).await?;
    let resp = crate::zapexec::call(Request::SshUserKeyPrivateGet {
        linux_user,
        name: name.clone(),
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    let data = resp.data.unwrap_or(Value::Null);
    let private_key = data
        .get("private_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    Ok(Json(
        json!({ "code": 0, "data": { "name": name, "private_key": private_key } }),
    ))
}

async fn audit_log(claims: &ValidatedClaims, action: &str, detail: &str) {
    crate::zap::audit::log(Some(claims), None, action, detail, "用户 SSH 密钥管理").await;
}
