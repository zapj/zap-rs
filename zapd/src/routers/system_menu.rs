use axum::Json;
use serde_json::json;

use crate::zap::ZapJsonResult;



pub async fn get_menus_tree() -> ZapJsonResult {
    
    Ok(Json(json!({})))
}