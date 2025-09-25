use axum::Extension;
use sqlx::{Pool, Sqlite};

use crate::zap::{self, jwt, ZapJsonResult};

pub async fn system_info(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> ZapJsonResult {
    zap::system_info::get_system_info().await
}

pub async fn system_status(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> ZapJsonResult {
    zap::system_info::get_system_status().await
}

// show all system info
// pub async fn system_info_full(_:jwt::Claims,Extension(_):Extension<Pool<Sqlite>>) -> Json<Value> {
//     let system_info = zap::system_info::get_os_info().await;
//     return Json(json!({
//             "code":0,
//             "message":"OK",
//             "data":system_info,
//         }));
// }