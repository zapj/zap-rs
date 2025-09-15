use axum::{Extension, Json};
use serde_json::{json, Value};
use sqlx::{Pool, Sqlite};

use crate::zap::{self, jwt};

pub async fn system_info(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> Json<Value> {
    let system_info = zap::system_info::get_os_info().await;
    return Json(json!({
            "code":0,
            "message":"OK",
            "data":system_info,
        }));
}