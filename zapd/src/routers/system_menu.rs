use axum::{Json, extract::Extension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Sqlite;
use std::net::SocketAddr;
use tracing::info;

use crate::{
    db,
    zap::{ZapError, ZapJsonResult, audit, jwt::ValidatedClaims},
};

// ── types ──────────────────────────────────────────────────

#[derive(sqlx::FromRow, Debug, Serialize)]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: i64,
    pub name: String,
    pub path: String,
    pub component: String,
    pub redirect: String,
    #[sqlx(rename = "type")]
    pub menu_type: String,
    pub title: String,
    pub icon: String,
    pub hidden: i64,
    pub keep_alive: i64,
    pub affix: i64,
    pub roles: String,
    pub sort_order: i64,
    pub status: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateMenuPayload {
    pub parent_id: Option<i64>,
    pub name: String,
    pub path: String,
    pub component: Option<String>,
    pub redirect: Option<String>,
    #[serde(rename = "type")]
    pub menu_type: Option<String>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub hidden: Option<i64>,
    pub keep_alive: Option<i64>,
    pub affix: Option<i64>,
    pub roles: Option<String>,
    pub sort_order: Option<i64>,
    pub status: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenuPayload {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub component: Option<String>,
    pub redirect: Option<String>,
    #[serde(rename = "type")]
    pub menu_type: Option<String>,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub hidden: Option<i64>,
    pub keep_alive: Option<i64>,
    pub affix: Option<i64>,
    pub roles: Option<String>,
    pub sort_order: Option<i64>,
    pub status: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteMenuPayload {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct MenuStatusPayload {
    pub id: i64,
    pub status: i64,
}

// ── tree builder ───────────────────────────────────────────

fn menu_to_tree_value(m: &MenuRow, children: Vec<Value>) -> Value {
    let mut meta = json!({
        "title": m.title,
        "icon": m.icon,
        "affix": m.affix == 1,
    });
    if m.hidden == 1 {
        meta["hidden"] = json!(true);
    }
    if m.keep_alive == 1 {
        meta["keepAlive"] = json!(true);
    }
    if !m.roles.is_empty() {
        meta["roles"] = json!(m.roles.split(',').map(|s| s.trim()).collect::<Vec<_>>());
    }

    let mut obj = json!({
        "id": m.id,
        "name": m.name,
        "path": m.path,
        "component": m.component,
        "type": m.menu_type,
        "meta": meta,
        "order": m.sort_order,
        "status": m.status,
    });
    if !m.redirect.is_empty() {
        obj["redirect"] = json!(m.redirect);
    }
    if !children.is_empty() {
        obj["children"] = json!(children);
    }
    obj
}

fn build_menu_tree(rows: &[MenuRow], parent_id: i64) -> Vec<Value> {
    rows.iter()
        .filter(|r| r.parent_id == parent_id)
        .map(|r| {
            let children = build_menu_tree(rows, r.id);
            menu_to_tree_value(r, children)
        })
        .collect()
}

// ── handlers ───────────────────────────────────────────────

/// Get full menu tree (for rendering sidebar)
pub async fn get_menus_tree() -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let rows: Vec<MenuRow> =
        sqlx::query_as("SELECT * FROM menus WHERE status = 1 ORDER BY sort_order, id")
            .fetch_all(pool)
            .await?;

    let tree = build_menu_tree(&rows, 0);
    Ok(Json(json!({ "code": 0, "message": "ok", "data": tree })))
}

/// Get flat menu list (for admin management)
pub async fn menu_list(_claims: ValidatedClaims) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let rows: Vec<MenuRow> = sqlx::query_as("SELECT * FROM menus ORDER BY sort_order, id")
        .fetch_all(pool)
        .await?;

    let tree = build_menu_tree(&rows, 0);
    Ok(Json(json!({ "code": 0, "message": "ok", "data": tree })))
}

/// Create menu
pub async fn menu_add(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<CreateMenuPayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();

    let result = sqlx::query(
        "INSERT INTO menus (parent_id, name, path, component, redirect, type, title, icon, hidden, keep_alive, affix, roles, sort_order, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(payload.parent_id.unwrap_or(0))
    .bind(&payload.name)
    .bind(&payload.path)
    .bind(payload.component.unwrap_or_default())
    .bind(payload.redirect.unwrap_or_default())
    .bind(payload.menu_type.unwrap_or_else(|| "menu".into()))
    .bind(payload.title.unwrap_or_default())
    .bind(payload.icon.unwrap_or_default())
    .bind(payload.hidden.unwrap_or(0))
    .bind(payload.keep_alive.unwrap_or(0))
    .bind(payload.affix.unwrap_or(0))
    .bind(payload.roles.unwrap_or_default())
    .bind(payload.sort_order.unwrap_or(0))
    .bind(payload.status.unwrap_or(1))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "menu_create",
        &format!("id={}", result.last_insert_rowid()),
        &payload.name,
    )
    .await;
    info!(
        "Menu created: {} (id: {})",
        payload.name,
        result.last_insert_rowid()
    );
    Ok(Json(
        json!({ "code": 0, "message": "创建成功", "data": { "id": result.last_insert_rowid() } }),
    ))
}

/// Update menu
pub async fn menu_update(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<UpdateMenuPayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let now = chrono::Local::now().timestamp();

    let mut qb: sqlx::QueryBuilder<'_, Sqlite> = sqlx::QueryBuilder::new("UPDATE menus SET ");
    let mut sep = qb.separated(", ");

    if let Some(v) = payload.parent_id {
        sep.push("parent_id = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.name {
        sep.push("name = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.path {
        sep.push("path = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.component {
        sep.push("component = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.redirect {
        sep.push("redirect = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.menu_type {
        sep.push("type = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.title {
        sep.push("title = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.icon {
        sep.push("icon = ").push_bind_unseparated(v);
    }
    if let Some(v) = payload.hidden {
        sep.push("hidden = ").push_bind_unseparated(v);
    }
    if let Some(v) = payload.keep_alive {
        sep.push("keep_alive = ").push_bind_unseparated(v);
    }
    if let Some(v) = payload.affix {
        sep.push("affix = ").push_bind_unseparated(v);
    }
    if let Some(ref v) = payload.roles {
        sep.push("roles = ").push_bind_unseparated(v);
    }
    if let Some(v) = payload.sort_order {
        sep.push("sort_order = ").push_bind_unseparated(v);
    }
    if let Some(v) = payload.status {
        sep.push("status = ").push_bind_unseparated(v);
    }
    sep.push("updated_at = ").push_bind_unseparated(now);

    qb.push(" WHERE id = ").push_bind(payload.id);
    let result = qb.build().execute(pool).await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "菜单不存在".to_string()));
    }
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "menu_update",
        &format!("id={}", payload.id),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "更新成功" })))
}

/// Delete menu (and children)
pub async fn menu_delete(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<DeleteMenuPayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    // Delete children first
    let _ = sqlx::query("DELETE FROM menus WHERE parent_id = ?")
        .bind(payload.id)
        .execute(pool)
        .await;
    let result = sqlx::query("DELETE FROM menus WHERE id = ?")
        .bind(payload.id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "菜单不存在".to_string()));
    }

    // Clean orphaned role_menus
    let _ = sqlx::query("DELETE FROM role_menus WHERE menu_id = ?")
        .bind(payload.id)
        .execute(pool)
        .await;

    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "menu_delete",
        &format!("id={}", payload.id),
        "",
    )
    .await;

    Ok(Json(json!({ "code": 0, "message": "删除成功" })))
}

/// Toggle menu status
pub async fn menu_status(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<MenuStatusPayload>,
) -> ZapJsonResult {
    let pool = db::get_db_pool().await;
    let result = sqlx::query("UPDATE menus SET status = ? WHERE id = ?")
        .bind(payload.status)
        .bind(payload.id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(ZapError::New(-1, "菜单不存在".to_string()));
    }
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "menu_status",
        &format!("id={}, status={}", payload.id, payload.status),
        "",
    )
    .await;
    Ok(Json(json!({ "code": 0, "message": "OK" })))
}
