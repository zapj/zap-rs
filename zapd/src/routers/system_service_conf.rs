//! 通用服务配置端点（「服务配置」大类：php / mysql / mariadb / docker …）。
//!
//! 端点（均需管理员，均透传 zapexec 返回）：
//! - GET  /system/service-conf/status           状态探测（Query: service）
//! - GET  /system/service-conf/list             列出可编辑配置（Query: service）
//! - GET  /system/service-conf/read             读取配置内容（Query: service, path）
//! - POST /system/service-conf/save             保存配置（body: service, path, content）
//! - GET  /system/service-conf/keys             关键项表单（Query: service）
//! - POST /system/service-conf/keys/save        保存关键项（body: service, keys）
//! - POST /system/service-conf/control          服务控制（body: service, action）

use std::collections::BTreeMap;
use std::net::SocketAddr;

use axum::Json;
use axum::extract::{Extension, Query};
use serde::Deserialize;
use serde_json::json;

use crate::zap::ZapError;
use crate::zap::ZapJsonResult;
use crate::zap::audit;
use crate::zap::jwt::ValidatedClaims;
use crate::zap::jwt::is_admin;
use zap_proto::Request;

fn require_admin(claims: &ValidatedClaims) -> Result<(), ZapError> {
    if is_admin(claims) {
        Ok(())
    } else {
        Err(ZapError::New(-1, "仅管理员可访问".to_string()))
    }
}

/// 执行一次 zapexec 请求，透传其 code/message/data。
async fn exec(req: Request) -> Result<Json<serde_json::Value>, ZapError> {
    let resp = crate::zapexec::call(req).await?;
    if resp.code != 0 {
        return Err(ZapError::New(resp.code, resp.message));
    }
    Ok(Json(json!({ "code": 0, "message": "ok", "data": resp.data })))
}

fn validate_service(service: &str) -> Result<String, ZapError> {
    if !matches!(service, "php" | "mysql" | "mariadb" | "docker") {
        return Err(ZapError::New(-1, "不支持的服务类型".to_string()));
    }
    Ok(service.to_string())
}

#[derive(Debug, Deserialize)]
pub struct ServiceQuery {
    pub service: String,
}

/// GET /system/service-conf/status
pub async fn status(
    claims: ValidatedClaims,
    Query(q): Query<ServiceQuery>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&q.service)?;
    exec(Request::ServiceConfStatus { service }).await
}

/// GET /system/service-conf/list
pub async fn conf_list(
    claims: ValidatedClaims,
    Query(q): Query<ServiceQuery>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&q.service)?;
    exec(Request::ServiceConfList { service }).await
}

#[derive(Debug, Deserialize)]
pub struct ServiceReadQuery {
    pub service: String,
    pub path: String,
}

/// GET /system/service-conf/read?service=..&path=..
pub async fn conf_read(
    claims: ValidatedClaims,
    Query(q): Query<ServiceReadQuery>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&q.service)?;
    exec(Request::ServiceConfRead {
        service,
        path: q.path,
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct ServiceSaveBody {
    pub service: String,
    pub path: String,
    pub content: String,
}

/// POST /system/service-conf/save
pub async fn conf_save(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
    Json(body): Json<ServiceSaveBody>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&body.service)?;
    let result = exec(Request::ServiceConfSave {
        service,
        path: body.path.clone(),
        content: body.content.clone(),
    })
    .await;
    if result.is_ok() {
        audit::log(
            Some(&claims),
            Some(client_addr.ip().to_string().as_str()),
            "service_conf_save",
            "service-conf",
            &format!("保存 {} 配置文件 {}", body.service, body.path),
        )
        .await;
    }
    result
}

/// GET /system/service-conf/keys
pub async fn keys_get(
    claims: ValidatedClaims,
    Query(q): Query<ServiceQuery>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&q.service)?;
    exec(Request::ServiceConfKeys { service }).await
}

#[derive(Debug, Deserialize)]
pub struct ServiceKeysSaveBody {
    pub service: String,
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

/// POST /system/service-conf/keys/save
pub async fn keys_save(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
    Json(body): Json<ServiceKeysSaveBody>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&body.service)?;
    let result = exec(Request::ServiceConfKeysSave {
        service,
        keys: body.keys.clone(),
    })
    .await;
    if result.is_ok() {
        audit::log(
            Some(&claims),
            Some(client_addr.ip().to_string().as_str()),
            "service_conf_keys_save",
            "service-conf",
            &format!("保存 {} 关键配置", body.service),
        )
        .await;
    }
    result
}

#[derive(Debug, Deserialize)]
pub struct ServiceControlBody {
    pub service: String,
    pub action: String,
}

/// POST /system/service-conf/control
pub async fn control(
    claims: ValidatedClaims,
    client_addr: Extension<SocketAddr>,
    Json(body): Json<ServiceControlBody>,
) -> ZapJsonResult {
    require_admin(&claims)?;
    let service = validate_service(&body.service)?;
    if !matches!(body.action.as_str(), "start" | "stop" | "restart" | "reload") {
        return Err(ZapError::New(
            -1,
            "仅支持 start / stop / restart / reload".to_string(),
        ));
    }
    let result = exec(Request::ServiceConfControl {
        service,
        action: body.action.clone(),
    })
    .await;
    if result.is_ok() {
        audit::log(
            Some(&claims),
            Some(client_addr.ip().to_string().as_str()),
            "service_conf_control",
            "service-conf",
            &format!("{} 服务操作 {}", body.service, body.action),
        )
        .await;
    }
    result
}
