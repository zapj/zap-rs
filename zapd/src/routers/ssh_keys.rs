use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::zap::{jwt::ValidatedClaims, ZapError, ZapJsonResult};
use zap_proto::Request;

// ── handlers ───────────────────────────────────────────────
// 所有对 SSH 密钥（/etc/zap/ssh）与 authorized_keys 的读写都转发给
// zapexec（root），zapd 不再直接操作 root 拥有的文件。

/// List all SSH keys in /etc/zap/ssh/
pub async fn list_keys(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyList).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

/// Get the public key content for a specific key
pub async fn get_key_content(
    _claims: ValidatedClaims,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ZapJsonResult {
    let name = params.get("name").cloned().unwrap_or_default();
    if name.is_empty() {
        return Err(ZapError::New(-1, "缺少密钥名称".to_string()));
    }

    let resp = crate::zapexec::call(Request::SshKeyGet { name }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

// ── generate ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GenerateKeyPayload {
    pub name: String,
    pub key_type: Option<String>, // rsa, ed25519, ecdsa
    pub bits: Option<u32>,        // for rsa
    pub comment: Option<String>,
}

pub async fn generate_key(
    _claims: ValidatedClaims,
    Json(payload): Json<GenerateKeyPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyGenerate {
        name: payload.name,
        key_type: payload.key_type,
        bits: payload.bits,
        comment: payload.comment,
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
}

// ── import ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ImportKeyPayload {
    pub name: String,
    pub private_key: String,
    pub public_key: Option<String>,
}

pub async fn import_key(
    _claims: ValidatedClaims,
    Json(payload): Json<ImportKeyPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyImport {
        name: payload.name,
        private_key: payload.private_key,
        public_key: payload.public_key,
    })
    .await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message, "data": resp.data })))
}

// ── delete ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DeleteKeyPayload {
    pub name: String,
}

pub async fn delete_key(
    _claims: ValidatedClaims,
    Json(payload): Json<DeleteKeyPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyDelete { name: payload.name }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

// ── authorized_keys ────────────────────────────────────────

/// List all entries in authorized_keys
pub async fn list_authorized_keys(_claims: ValidatedClaims) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyAuthorizedList).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "data": resp.data })))
}

/// Authorize a key (add to authorized_keys)
#[derive(Debug, Deserialize)]
pub struct AuthorizeKeyPayload {
    pub name: String, // key name from /etc/zap/ssh/
}

pub async fn authorize_key(
    _claims: ValidatedClaims,
    Json(payload): Json<AuthorizeKeyPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyAuthorize { name: payload.name }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}

/// Deauthorize a key (remove from authorized_keys by index)
#[derive(Debug, Deserialize)]
pub struct DeauthorizeKeyPayload {
    pub index: usize,
}

pub async fn deauthorize_key(
    _claims: ValidatedClaims,
    Json(payload): Json<DeauthorizeKeyPayload>,
) -> ZapJsonResult {
    let resp = crate::zapexec::call(Request::SshKeyDeauthorize { index: payload.index }).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": resp.message })))
}
