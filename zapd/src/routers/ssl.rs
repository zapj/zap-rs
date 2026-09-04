//! 「SSL/TLS」菜单后端：SSL 证书管理。
//!
//! 支持三种来源并统一按 PEM 保存到 `ssl_cert` 表（四段材料）：
//!   - cert_content：证书（crt；Let's Encrypt 时为叶子证书）
//!   - key_content ：私钥（key）
//!   - ca_bundle   ：中间链（ca-bundle；自签名 / 单段证书为空）
//!   - csr         ：证书签名请求（手动导入或自签名生成时产生）
//!
//! 功能：手动添加 / 修改 / 删除 / 查看；rcgen 生成自签名证书；
//! acme-lib 走 ACME HTTP-01 向 Let's Encrypt 申请（需 80 端口可达）。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::body::Body as AxBody;
use axum::extract::{Extension, Query};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::{
    db,
    zap::{ZapError, ZapJsonResult, audit, jwt::ValidatedClaims},
};

// ── 行结构 ───────────────────────────────────────────────────

#[derive(sqlx::FromRow, Debug, serde::Serialize)]
struct CertListRow {
    id: i64,
    name: String,
    domains: String,
    cert_type: String,
    not_before: i64,
    not_after: i64,
    status: i64,
    remark: String,
    created_at: i64,
    updated_at: i64,
}

#[derive(sqlx::FromRow, Debug, serde::Serialize)]
struct CertDetailRow {
    id: i64,
    name: String,
    domains: String,
    cert_type: String,
    cert_content: String,
    key_content: String,
    ca_bundle: String,
    csr: String,
    not_before: i64,
    not_after: i64,
    status: i64,
    remark: String,
    created_at: i64,
    updated_at: i64,
}

// ── 列表 / 详情 ──────────────────────────────────────────────

pub async fn cert_list(_claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let rows: Vec<CertListRow> = sqlx::query_as(
        "SELECT id, name, domains, cert_type, not_before, not_after, status, remark,
                created_at, updated_at
         FROM ssl_cert
         ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(Json(json!({ "code": 0, "message": "OK", "data": rows })))
}

#[derive(Debug, Deserialize)]
pub struct CertDetailQuery {
    pub id: i64,
}

pub async fn cert_detail(
    Query(q): Query<CertDetailQuery>,
    _claims: ValidatedClaims,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let row: Option<CertDetailRow> = sqlx::query_as("SELECT * FROM ssl_cert WHERE id = ?")
        .bind(q.id)
        .fetch_optional(pool)
        .await?;
    match row {
        Some(r) => Ok(Json(json!({ "code": 0, "message": "OK", "data": r }))),
        None => Err(ZapError::New(-1, "证书不存在".to_string())),
    }
}

// ── 手动添加 / 修改 / 删除 ───────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CertAddPayload {
    pub name: String,
    #[serde(default)]
    pub domains: String,
    #[serde(default)]
    pub cert_content: String,
    #[serde(default)]
    pub key_content: String,
    #[serde(default)]
    pub ca_bundle: String,
    #[serde(default)]
    pub csr: String,
    #[serde(default)]
    pub remark: String,
}

pub async fn cert_add(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CertAddPayload>,
) -> ZapJsonResult {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ZapError::New(-1, "请填写证书名称".to_string()));
    }
    let has_material = !payload.cert_content.trim().is_empty()
        || !payload.key_content.trim().is_empty()
        || !payload.csr.trim().is_empty();
    if !has_material {
        return Err(ZapError::New(
            -1,
            "证书内容为空：至少提供 cert / key / csr 中的一种".to_string(),
        ));
    }
    // 域名 / 有效期：未提供域名时自动从证书（或 CSR）解析
    let parsed = parse_pem_info(&payload.cert_content).or_else(|| parse_pem_info(&payload.csr));
    let domains = if payload.domains.trim().is_empty() {
        parsed
            .as_ref()
            .map(|p| p.domains_str.clone())
            .unwrap_or_default()
    } else {
        payload.domains.trim().to_string()
    };
    let (not_before, not_after) = parsed
        .as_ref()
        .map(|p| (p.not_before, p.not_after))
        .unwrap_or((0, 0));

    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let r = sqlx::query(
        "INSERT INTO ssl_cert
            (name, domains, cert_type, cert_content, key_content, ca_bundle, csr,
             not_before, not_after, status, remark, created_at, updated_at)
         VALUES (?, ?, 'upload', ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
    )
    .bind(&name)
    .bind(&domains)
    .bind(payload.cert_content.trim())
    .bind(payload.key_content.trim())
    .bind(payload.ca_bundle.trim())
    .bind(payload.csr.trim())
    .bind(not_before)
    .bind(not_after)
    .bind(payload.remark.trim())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let id = r.last_insert_rowid();
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssl_cert_add",
        &name,
        &format!("id={id}"),
    )
    .await;
    Ok(Json(
        json!({ "code": 0, "message": "OK", "data": { "id": id } }),
    ))
}

/// 修改证书。四段 PEM 为 `Option`：未传（如只切换启用状态）时保持原值不变，
/// 显式传空串才会清空对应内容。
#[derive(Debug, Deserialize)]
pub struct CertUpdatePayload {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub domains: String,
    #[serde(default)]
    pub cert_content: Option<String>,
    #[serde(default)]
    pub key_content: Option<String>,
    #[serde(default)]
    pub ca_bundle: Option<String>,
    #[serde(default)]
    pub csr: Option<String>,
    #[serde(default)]
    pub remark: String,
    #[serde(default)]
    pub status: Option<i32>,
}

pub async fn cert_update(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CertUpdatePayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let status = payload.status.unwrap_or(1);

    // 未提供域名时，若本次上传了证书（或 CSR）则自动解析填充
    let mut domains = payload.domains.trim().to_string();
    let mut validity: Option<(i64, i64)> = None;
    if domains.is_empty() {
        if let Some(p) = parse_pem_info(payload.cert_content.as_deref().unwrap_or("")) {
            domains = p.domains_str.clone();
            if p.not_after > 0 {
                validity = Some((p.not_before, p.not_after));
            }
        } else if let Some(p) = parse_pem_info(payload.csr.as_deref().unwrap_or("")) {
            domains = p.domains_str.clone();
        }
    } else if let Some(p) = parse_pem_info(payload.cert_content.as_deref().unwrap_or(""))
        && p.not_after > 0
    {
        validity = Some((p.not_before, p.not_after));
    }

    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("UPDATE ssl_cert SET ");
    {
        let mut sep = qb.separated(", ");
        sep.push("name = ").push_bind_unseparated(payload.name.trim());
        sep.push("domains = ").push_bind_unseparated(domains);
        sep.push("remark = ").push_bind_unseparated(payload.remark.trim());
        sep.push("status = ").push_bind_unseparated(status);
        sep.push("updated_at = ").push_bind_unseparated(now);
    }
    if let Some(v) = payload.cert_content.as_deref() {
        qb.push(", cert_content = ").push_bind(v.trim());
    }
    if let Some(v) = payload.key_content.as_deref() {
        qb.push(", key_content = ").push_bind(v.trim());
    }
    if let Some(v) = payload.ca_bundle.as_deref() {
        qb.push(", ca_bundle = ").push_bind(v.trim());
    }
    if let Some(v) = payload.csr.as_deref() {
        qb.push(", csr = ").push_bind(v.trim());
    }
    if let Some((nb, na)) = validity {
        qb.push(", not_before = ")
            .push_bind(nb)
            .push(", not_after = ")
            .push_bind(na);
    }
    qb.push(" WHERE id = ").push_bind(payload.id);
    let r = qb.build().execute(pool).await?;
    if r.rows_affected() == 0 {
        return Err(ZapError::New(-1, "证书不存在".to_string()));
    }
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssl_cert_update",
        &payload.name,
        &format!("id={}", payload.id),
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "OK" })))
}

#[derive(Debug, Deserialize)]
pub struct CertDeletePayload {
    pub id: i64,
}

pub async fn cert_delete(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CertDeletePayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let r = sqlx::query("DELETE FROM ssl_cert WHERE id = ?")
        .bind(payload.id)
        .execute(pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(ZapError::New(-1, "证书不存在".to_string()));
    }
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssl_cert_delete",
        &format!("id={}", payload.id),
        "删除证书",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "OK" })))
}

// ── 证书解析（自动读取域名 / 有效期等，供添加证书时自动填充）──

/// subjectAltName 的 OID（2.5.29.17 → 55 1D 11）。
const OID_SUBJECT_ALT_NAME: &[u8] = &[0x55, 0x1d, 0x11];

#[derive(Debug, serde::Serialize)]
struct ParsedCertInfo {
    /// cert：X.509 证书；csr：证书签名请求
    kind: String,
    domains: Vec<String>,
    domains_str: String,
    common_name: String,
    subject: String,
    issuer: String,
    not_before: i64,
    not_after: i64,
    serial: String,
    fingerprint: String,
    key_type: String,
    key_bits: u32,
    /// SAN 中解析出的域名数量（0 表示证书不含 SAN，域名取自 CN）
    sans_count: usize,
    /// PEM 中包含的证书数量（>1 说明粘贴的是含中间链的 fullchain）
    cert_count: usize,
}

/// 极简 DER 读取器：只解析 TLV（单字节 tag + 短/长型长度）。
struct Der<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Der<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn next_tlv(&mut self) -> Option<(u8, &'a [u8])> {
        let tag = *self.buf.get(self.pos)?;
        self.pos += 1;
        let first = *self.buf.get(self.pos)? as usize;
        self.pos += 1;
        let len = if first & 0x80 != 0 {
            let n = first & 0x7f;
            if n == 0 || n > 4 {
                return None;
            }
            let mut v = 0usize;
            for _ in 0..n {
                v = (v << 8) | (*self.buf.get(self.pos)? as usize);
                self.pos += 1;
            }
            v
        } else {
            first
        };
        let end = self.pos.checked_add(len)?;
        let data = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some((tag, data))
    }
}

fn push_uniq(v: &mut Vec<String>, s: String) {
    let s = s.trim().to_string();
    if s.is_empty() {
        return;
    }
    if !v.iter().any(|x| x.eq_ignore_ascii_case(&s)) {
        v.push(s);
    }
}

/// 拆开 X509Extension 的 DER：Extension ::= SEQUENCE { oid, [critical], OCTET STRING }。
fn split_extension_der(der: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut outer = Der::new(der);
    let (tag, seq) = outer.next_tlv()?;
    if tag != 0x30 {
        return None;
    }
    let mut inner = Der::new(seq);
    let (tag, oid) = inner.next_tlv()?;
    if tag != 0x06 {
        return None;
    }
    let (mut tag, mut val) = inner.next_tlv()?;
    if tag == 0x01 {
        // critical 标志，跳过
        (tag, val) = inner.next_tlv()?;
    }
    if tag != 0x04 {
        return None;
    }
    Some((oid, val))
}

/// 解析 SEQUENCE OF GeneralName，只取 dNSName(2) 与 iPAddress(7)。
fn general_name_domains(der: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut outer = Der::new(der);
    let Some((tag, seq)) = outer.next_tlv() else {
        return out;
    };
    if tag != 0x30 {
        return out;
    }
    let mut inner = Der::new(seq);
    while let Some((tag, val)) = inner.next_tlv() {
        match tag {
            0x82 => {
                if let Ok(s) = std::str::from_utf8(val) {
                    push_uniq(&mut out, s.to_string());
                }
            }
            0x87 => {
                if val.len() == 4 {
                    let mut a = [0u8; 4];
                    a.copy_from_slice(val);
                    push_uniq(&mut out, std::net::Ipv4Addr::from(a).to_string());
                } else if val.len() == 16 {
                    let mut a = [0u8; 16];
                    a.copy_from_slice(val);
                    push_uniq(&mut out, std::net::Ipv6Addr::from(a).to_string());
                }
            }
            _ => {}
        }
    }
    out
}

/// 把 X509Name 渲染成 `C=CN, O=xxx, CN=yyy` 形式。
fn name_to_string(name: &openssl::x509::X509NameRef) -> String {
    name.entries()
        .map(|e| {
            let key = e.object().nid().short_name().unwrap_or("?");
            let val = String::from_utf8_lossy(e.data().as_slice()).trim().to_string();
            format!("{key}={val}")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn common_name(name: &openssl::x509::X509NameRef) -> String {
    use openssl::nid::Nid;
    name.entries_by_nid(Nid::COMMONNAME)
        .next()
        .map(|e| String::from_utf8_lossy(e.data().as_slice()).trim().to_string())
        .unwrap_or_default()
}

/// ASN1 时间 → Unix 时间戳（秒）。
fn asn1_ts(t: &openssl::asn1::Asn1TimeRef) -> i64 {
    use openssl::asn1::Asn1Time;
    let Ok(epoch) = Asn1Time::from_unix(0) else {
        return 0;
    };
    match epoch.diff(t) {
        Ok(d) => d.days as i64 * 86400 + d.secs as i64,
        Err(_) => 0,
    }
}

/// 公钥类型与位数。
fn key_meta(pkey: &openssl::pkey::PKeyRef<openssl::pkey::Public>) -> (String, u32) {
    use openssl::pkey::Id;
    let name = match pkey.id() {
        Id::RSA => "RSA",
        Id::EC => "EC",
        Id::ED25519 => "Ed25519",
        Id::ED448 => "Ed448",
        Id::DSA => "DSA",
        _ => "未知",
    };
    (name.to_string(), pkey.bits())
}

fn parse_certificate(pem: &str) -> Option<ParsedCertInfo> {
    use openssl::hash::MessageDigest;
    use openssl::x509::X509;

    let cert = X509::from_pem(pem.as_bytes()).ok()?;
    let subject = name_to_string(cert.subject_name());
    let issuer = name_to_string(cert.issuer_name());
    let cn = common_name(cert.subject_name());

    let mut domains: Vec<String> = Vec::new();
    if let Some(sans) = cert.subject_alt_names() {
        for gn in sans.iter() {
            if let Some(dns) = gn.dnsname() {
                push_uniq(&mut domains, dns.to_string());
            } else if let Some(ip) = gn.ipaddress() {
                if ip.len() == 4 {
                    let mut a = [0u8; 4];
                    a.copy_from_slice(ip);
                    push_uniq(&mut domains, std::net::Ipv4Addr::from(a).to_string());
                } else if ip.len() == 16 {
                    let mut a = [0u8; 16];
                    a.copy_from_slice(ip);
                    push_uniq(&mut domains, std::net::Ipv6Addr::from(a).to_string());
                }
            }
        }
    }
    let sans_count = domains.len();
    // 无 SAN 时退回 CN（老式证书 / 自签名证书常见）
    if domains.is_empty() && !cn.is_empty() {
        domains.push(cn.clone());
    }

    let (key_type, key_bits) = match cert.public_key() {
        Ok(pk) => key_meta(&pk),
        Err(_) => (String::new(), 0),
    };
    let serial = cert
        .serial_number()
        .to_bn()
        .ok()
        .and_then(|b| b.to_hex_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let fingerprint = match cert.digest(MessageDigest::sha256()) {
        Ok(d) => hex::encode_upper(d)
            .as_bytes()
            .chunks(2)
            .map(|c| String::from_utf8_lossy(c).to_string())
            .collect::<Vec<_>>()
            .join(":"),
        Err(_) => String::new(),
    };
    let cert_count = pem.matches("-----BEGIN CERTIFICATE-----").count();

    Some(ParsedCertInfo {
        kind: "cert".to_string(),
        domains_str: domains.join(", "),
        domains,
        common_name: cn,
        subject,
        issuer,
        not_before: asn1_ts(cert.not_before()),
        not_after: asn1_ts(cert.not_after()),
        serial,
        fingerprint,
        key_type,
        key_bits,
        sans_count,
        cert_count,
    })
}

fn parse_csr_pem(pem: &str) -> Option<ParsedCertInfo> {
    use openssl::x509::X509Req;

    let req = X509Req::from_pem(pem.as_bytes()).ok()?;
    let subject = name_to_string(req.subject_name());
    let cn = common_name(req.subject_name());

    let mut domains: Vec<String> = Vec::new();
    if let Ok(exts) = req.extensions() {
        for ext in exts.iter() {
            let Ok(der) = ext.to_der() else { continue };
            let Some((oid, val)) = split_extension_der(&der) else {
                continue;
            };
            if oid != OID_SUBJECT_ALT_NAME {
                continue;
            }
            for d in general_name_domains(val) {
                push_uniq(&mut domains, d);
            }
        }
    }
    let sans_count = domains.len();
    if domains.is_empty() && !cn.is_empty() {
        domains.push(cn.clone());
    }
    let (key_type, key_bits) = match req.public_key() {
        Ok(pk) => key_meta(&pk),
        Err(_) => (String::new(), 0),
    };

    Some(ParsedCertInfo {
        kind: "csr".to_string(),
        domains_str: domains.join(", "),
        domains,
        common_name: cn,
        subject,
        issuer: String::new(),
        not_before: 0,
        not_after: 0,
        serial: String::new(),
        fingerprint: String::new(),
        key_type,
        key_bits,
        sans_count,
        cert_count: 0,
    })
}

/// 自动识别 PEM 类型并解析（证书优先取第一张，即叶子证书）。
fn parse_pem_info(raw: &str) -> Option<ParsedCertInfo> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains("BEGIN CERTIFICATE REQUEST") || t.contains("BEGIN NEW CERTIFICATE REQUEST") {
        return parse_csr_pem(t);
    }
    if t.contains("BEGIN CERTIFICATE")
        || t.contains("BEGIN X509 CERTIFICATE")
        || t.contains("BEGIN TRUSTED CERTIFICATE")
    {
        return parse_certificate(t);
    }
    None
}

#[derive(Debug, Deserialize)]
pub struct CertParsePayload {
    #[serde(default)]
    pub pem: String,
}

/// 解析证书 / CSR：返回域名、有效期、签发者、指纹等，用于添加证书时自动填充。
pub async fn cert_parse(_claims: ValidatedClaims, Json(payload): Json<CertParsePayload>) -> ZapJsonResult {
    match parse_pem_info(&payload.pem) {
        Some(info) => Ok(Json(json!({ "code": 0, "message": "OK", "data": info }))),
        None => Err(ZapError::New(
            -1,
            "无法识别证书内容：请粘贴 PEM 格式的证书（crt）或 CSR".to_string(),
        )),
    }
}

// ── 自签名生成 ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CertSelfSignPayload {
    pub name: String,
    pub domains: String,
    #[serde(default = "default_sign_days")]
    pub days: i64,
    #[serde(default)]
    pub remark: String,
}

fn default_sign_days() -> i64 {
    365
}

/// 拆分逗号 / 空格 / 换行分隔的域名列表。
fn split_domains(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\t', '\n', '\r', ';'])
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 使用 rcgen 生成自签名证书，返回 (crt, key, csr, not_before, not_after)。
fn gen_self_signed(
    domains: &[String],
    days: i64,
) -> Result<(String, String, String, i64, i64), ZapError> {
    use rcgen::{CertificateParams, DnType, KeyPair};
    use time::{Duration, OffsetDateTime};

    if domains.is_empty() {
        return Err(ZapError::New(
            -1,
            "请填写至少一个域名或 IP（多个用逗号分隔）".to_string(),
        ));
    }
    let mut params = CertificateParams::new(domains.to_vec())
        .map_err(|e| ZapError::New(-1, format!("域名不合法: {e}")))?;
    params
        .distinguished_name
        .push(DnType::CommonName, domains[0].as_str());
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(days);

    let key_pair =
        KeyPair::generate().map_err(|e| ZapError::New(-1, format!("密钥生成失败: {e}")))?;
    let csr = params
        .serialize_request(&key_pair)
        .map_err(|e| ZapError::New(-1, format!("CSR 生成失败: {e}")))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| ZapError::New(-1, format!("证书签名失败: {e}")))?;
    let csr_pem = csr
        .pem()
        .map_err(|e| ZapError::New(-1, format!("CSR 序列化失败: {e}")))?;

    Ok((
        cert.pem(),
        key_pair.serialize_pem(),
        csr_pem,
        (now - Duration::days(1)).unix_timestamp(),
        (now + Duration::days(days)).unix_timestamp(),
    ))
}

pub async fn cert_self_sign(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CertSelfSignPayload>,
) -> ZapJsonResult {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err(ZapError::New(-1, "请填写证书名称".to_string()));
    }
    let domains = split_domains(&payload.domains);
    let days = payload.days.clamp(1, 3650);
    let (cert_pem, key_pem, csr_pem, not_before, not_after) = gen_self_signed(&domains, days)?;

    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let domains_str = domains.join(", ");
    let r = sqlx::query(
        "INSERT INTO ssl_cert
            (name, domains, cert_type, cert_content, key_content, ca_bundle, csr,
             not_before, not_after, status, remark, created_at, updated_at)
         VALUES (?, ?, 'self-signed', ?, ?, '', ?, ?, ?, 1, ?, ?, ?)",
    )
    .bind(&name)
    .bind(&domains_str)
    .bind(&cert_pem)
    .bind(&key_pem)
    .bind(&csr_pem)
    .bind(not_before)
    .bind(not_after)
    .bind(payload.remark.trim())
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let id = r.last_insert_rowid();
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssl_cert_self_sign",
        &name,
        &format!("id={id}, domains={domains_str}, days={days}"),
    )
    .await;
    Ok(Json(
        json!({ "code": 0, "message": "OK", "data": { "id": id } }),
    ))
}

// ── Let's Encrypt 申请（ACME HTTP-01）────────────────────────

#[derive(Debug, Deserialize)]
pub struct CertLetsEncryptPayload {
    pub email: String,
    pub domains: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub staging: Option<bool>,
    #[serde(default)]
    pub remark: Option<String>,
}

/// 将 PEM 证书链拆为（叶子证书, 中间链），供 crt / ca-bundle 分列存储。
fn split_leaf_chain(pem: &str) -> (String, String) {
    const END: &str = "-----END CERTIFICATE-----";
    let mut blocks: Vec<String> = Vec::new();
    for part in pem.split(END) {
        if let Some(pos) = part.find("-----BEGIN CERTIFICATE-----") {
            let seg = part[pos..].trim();
            if !seg.is_empty() {
                blocks.push(format!("{seg}{END}"));
            }
        }
    }
    if blocks.is_empty() {
        return (pem.trim().to_string(), String::new());
    }
    let leaf = blocks.remove(0);
    let ca = blocks.join("\n");
    (leaf, ca)
}

/// 同步执行 ACME 订单流程（HTTP-01）。map 用于向临时验证服务器注入 token→keyAuth。
/// 成功返回 (fullchain_pem, private_key_pem, 剩余天数)。
fn run_acme_order(
    email: &str,
    primary: &str,
    alt: &[String],
    staging: bool,
    map: &Arc<Mutex<HashMap<String, String>>>,
) -> Result<(String, String, i64), String> {
    use acme_lib::persist::MemoryPersist;
    use acme_lib::{Directory, DirectoryUrl};

    let url = if staging {
        DirectoryUrl::LetsEncryptStaging
    } else {
        DirectoryUrl::LetsEncrypt
    };
    let persist = MemoryPersist::new();
    let dir = Directory::from_url(persist, url).map_err(|e| e.to_string())?;
    let acc = dir.account(email).map_err(|e| e.to_string())?;
    let alt_refs: Vec<&str> = alt.iter().map(|s| s.as_str()).collect();
    let mut ord_new = acc
        .new_order(primary, &alt_refs)
        .map_err(|e| e.to_string())?;

    let ord_csr = loop {
        if let Some(o) = ord_new.confirm_validations() {
            break o;
        }
        let auths = ord_new.authorizations().map_err(|e| e.to_string())?;
        for auth in auths.iter() {
            let chall = auth.http_challenge();
            let token = chall.http_token().to_string();
            let proof = chall.http_proof().to_string();
            if let Ok(mut m) = map.lock() {
                m.insert(token.clone(), proof);
            }
            info!(token = %token, "ACME HTTP-01 等待域名验证");
            chall.validate(8000).map_err(|e| e.to_string())?;
        }
        ord_new.refresh().map_err(|e| e.to_string())?;
    };

    let pkey = acme_lib::create_p384_key();
    let ord_cert = ord_csr
        .finalize_pkey(pkey, 8000)
        .map_err(|e| e.to_string())?;
    let cert = ord_cert
        .download_and_save_cert()
        .map_err(|e| e.to_string())?;
    Ok((
        cert.certificate().to_string(),
        cert.private_key().to_string(),
        cert.valid_days_left(),
    ))
}

pub async fn cert_letsencrypt(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CertLetsEncryptPayload>,
) -> ZapJsonResult {
    let email = payload.email.trim().to_string();
    if email.is_empty() {
        return Err(ZapError::New(-1, "请填写 ACME 账户邮箱".to_string()));
    }
    let domains = split_domains(&payload.domains);
    if domains.is_empty() {
        return Err(ZapError::New(-1, "请填写至少一个域名".to_string()));
    }
    // 域名不能是纯 IP（LE 仅支持域名）。
    for d in &domains {
        if d.parse::<std::net::IpAddr>().is_ok() {
            return Err(ZapError::New(
                -1,
                format!("Let's Encrypt 不支持 IP 地址申请：{d}"),
            ));
        }
    }
    let staging = payload.staging.unwrap_or(false);
    let primary = domains[0].clone();
    let alt: Vec<String> = domains[1..].to_vec();

    // 1) 启动临时 HTTP-01 验证服务器（Let's Encrypt 固定访问 80 端口）
    let map: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", 80))
        .await
        .map_err(|e| {
            ZapError::New(
                -1,
                format!("无法监听 80 端口以完成域名验证：{e}（需 root 权限且 80 端口未被占用）"),
            )
        })?;
    let map_srv = map.clone();
    let srv = tokio::spawn(async move {
        loop {
            let (stream, _peer) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => break,
            };
            let map_conn = map_srv.clone();
            tokio::spawn(async move {
                let io = hyper_util::rt::TokioIo::new(stream);
                let svc = service_fn(move |req: Request<Incoming>| {
                    let map_req = map_conn.clone();
                    async move {
                        let path = req.uri().path().to_string();
                        if let Some(token) = path.strip_prefix("/.well-known/acme-challenge/")
                            && let Some(proof) = map_req.lock().unwrap().get(token).cloned()
                        {
                            return Ok::<_, std::convert::Infallible>(Response::new(AxBody::from(
                                proof,
                            )));
                        }
                        Ok(Response::builder()
                            .status(StatusCode::NOT_FOUND)
                            .body(AxBody::from("not found"))
                            .unwrap())
                    }
                });
                let _ = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                )
                .serve_connection(io, svc)
                .await;
            });
        }
    });

    // 2) 在阻塞线程执行 ACME 流程（内部会轮询等待验证结果）
    let map_acme = map.clone();
    let email_acme = email.clone();
    let primary_acme = primary.clone();
    let alt_acme = alt.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_acme_order(&email_acme, &primary_acme, &alt_acme, staging, &map_acme)
    })
    .await
    .map_err(|e| ZapError::New(-1, format!("ACME 任务执行异常: {e}")))?;

    srv.abort();

    let (fullchain_pem, key_pem, days_left) = match result {
        Ok(v) => v,
        Err(e) => {
            return Err(ZapError::New(-1, format!("Let's Encrypt 申请失败：{e}")));
        }
    };

    // 3) 拆分叶子证书与中间链后入库
    let (leaf, ca) = split_leaf_chain(&fullchain_pem);
    let cert_type = if staging {
        "letsencrypt-staging"
    } else {
        "letsencrypt"
    };
    let remark = format!(
        "{}（{} 天有效）",
        payload
            .remark
            .as_deref()
            .unwrap_or("Let's Encrypt 自动申请"),
        days_left.max(0)
    );
    let name = payload
        .name
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| primary.clone());

    let pool = db::get_db_pool().await;
    let now = chrono::Utc::now().timestamp();
    let not_after = if days_left > 0 {
        now + days_left * 86400
    } else {
        0
    };
    let domains_str = domains.join(", ");
    let r = sqlx::query(
        "INSERT INTO ssl_cert
            (name, domains, cert_type, cert_content, key_content, ca_bundle, csr,
             not_after, status, remark, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, '', ?, ?, 1, ?, ?, ?)",
    )
    .bind(&name)
    .bind(&domains_str)
    .bind(cert_type)
    .bind(&leaf)
    .bind(&key_pem)
    .bind(&ca)
    .bind(not_after)
    .bind(&remark)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    let id = r.last_insert_rowid();
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "ssl_cert_letsencrypt",
        &name,
        &format!("id={id}, domains={domains_str}, staging={staging}"),
    )
    .await;
    Ok(Json(
        json!({ "code": 0, "message": "OK", "data": { "id": id, "cert_type": cert_type } }),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;

    /// 证书 / CSR 的域名、有效期应能被自动解析出来（添加证书时无需手工填写域名）。
    #[test]
    fn parse_self_signed_materials() {
        let (cert_pem, _key, csr_pem, not_before, not_after) =
            gen_self_signed(&["example.com".to_string(), "www.example.com".to_string()], 30)
                .expect("生成自签名证书失败");

        let cert = parse_pem_info(&cert_pem).expect("解析证书失败");
        assert_eq!(cert.kind, "cert");
        assert_eq!(cert.domains, vec!["example.com", "www.example.com"]);
        assert_eq!(cert.common_name, "example.com");
        assert_eq!(cert.cert_count, 1);
        assert_eq!((cert.not_before, cert.not_after), (not_before, not_after));
        assert!(!cert.fingerprint.is_empty());

        let csr = parse_pem_info(&csr_pem).expect("解析 CSR 失败");
        assert_eq!(csr.kind, "csr");
        assert_eq!(csr.domains, vec!["example.com", "www.example.com"]);
    }

    #[test]
    fn parse_invalid_input() {
        assert!(parse_pem_info("").is_none());
        assert!(parse_pem_info("   ").is_none());
        assert!(parse_pem_info("not a pem at all").is_none());
    }
}
