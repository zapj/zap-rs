//! 数据迁移（服务器配置 → 数据迁移，仅 admin）。
//!
//! /home 磁盘不足时，把用户家目录数据整体迁移到新挂载点（如 /home2）：
//! 1. 物理搬移由 zapexec 以 root 执行（`user.home_migrate`，支持跨文件系统）；
//! 2. 更新 `user.home_dir` 与站点 `web_root/log_root`（数据库路径前缀跟随）；
//! 3. 对涉及站点重新同步 Nginx vhost / PHP-FPM pool。
//!
//! 存量用户迁移不改变其登录凭据与站点配置；新用户仍按「运行环境默认设置」
//! 里的默认挂载点（user_home_root）创建。

use std::net::SocketAddr;

use axum::{
    Json,
    extract::{Extension, Query},
};
use serde::Deserialize;
use serde_json::json;

use crate::db;
use crate::zap::ZapError;
use crate::zap::ZapJsonResult;
use crate::zap::audit;
use crate::zap::jwt::ValidatedClaims;
use crate::zap::jwt::is_admin;
use zap_proto::Request;

/// 挂载点/家目录路径合法（绝对路径、无 `..`、无空白、非根）。
fn mount_ok(m: &str) -> bool {
    !m.is_empty() && m.starts_with('/') && !m.contains("..") && !m.contains(' ') && m.len() > 1
}

fn norm(m: &str) -> String {
    m.trim().trim_end_matches('/').to_string()
}

#[derive(Debug, Deserialize)]
pub struct MigratePreviewQuery {
    /// 源挂载点（默认 /home）
    src: Option<String>,
}

/// GET /system/migrate/users?src=/home：列出位于源挂载点下、可迁移的面板用户。
pub async fn migrate_users_preview(
    claims: ValidatedClaims,
    Query(q): Query<MigratePreviewQuery>,
) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可查看数据迁移".to_string()));
    }
    let src = norm(q.src.as_deref().unwrap_or("/home"));
    if !mount_ok(&src) {
        return Err(ZapError::New(-1, "源挂载点非法".to_string()));
    }

    let pool = db::get_db_pool().await;
    let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
        "SELECT u.id, u.username, u.linux_user, u.home_dir,
                (SELECT COUNT(*) FROM site s WHERE s.user_id = u.id)
         FROM user u WHERE u.home_dir LIKE ? AND u.home_dir != '' ORDER BY u.id",
    )
    .bind(format!("{src}/%"))
    .fetch_all(pool)
    .await?;

    let candidates = rows
        .into_iter()
        .map(|(id, username, linux_user, home_dir, site_count)| {
            json!({ "id": id, "username": username, "linux_user": linux_user, "home_dir": home_dir, "site_count": site_count })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "code": 0,
        "message": "ok",
        "data": { "src": src, "count": candidates.len(), "candidates": candidates }
    })))
}

#[derive(Debug, Deserialize)]
pub struct MigratePayload {
    /// 目标挂载点（如 /home2，须已挂载好且目录存在权限可写）
    pub dest: String,
    /// 源挂载点（默认 /home）
    #[serde(default)]
    pub src: Option<String>,
    /// 指定迁移的用户；空 = 迁移源挂载点下的全部用户
    #[serde(default)]
    pub user_ids: Option<Vec<i64>>,
}

/// POST /system/migrate/home：把用户从源挂载点迁移到目标挂载点。
pub async fn migrate_home_mv(
    claims: ValidatedClaims,
    Extension(client_addr): Extension<SocketAddr>,
    Json(payload): Json<MigratePayload>,
) -> ZapJsonResult {
    if !is_admin(&claims) {
        return Err(ZapError::New(-1, "仅管理员可执行数据迁移".to_string()));
    }
    let src = norm(payload.src.as_deref().unwrap_or("/home"));
    let dest = norm(&payload.dest);
    if !mount_ok(&src) || !mount_ok(&dest) {
        return Err(ZapError::New(-1, "源/目标挂载点非法".to_string()));
    }
    if src == dest {
        return Err(ZapError::New(
            -1,
            "源与目标挂载点相同，无需迁移".to_string(),
        ));
    }
    if dest == "/home" {
        return Err(ZapError::New(-1, "目标挂载点不能是默认 /home".to_string()));
    }
    let mode = crate::routers::system_env::vhost_mode().await;

    let pool = db::get_db_pool().await;
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, username, linux_user, home_dir FROM user
         WHERE home_dir LIKE ? AND home_dir != '' ORDER BY id",
    )
    .bind(format!("{src}/%"))
    .fetch_all(pool)
    .await?;

    // 按 user_ids 收敛（空 = 全部）
    let ids: Option<std::collections::HashSet<i64>> =
        payload.user_ids.map(|v| v.into_iter().collect());
    let mut ok_items: Vec<serde_json::Value> = Vec::new();
    let mut fail_items: Vec<serde_json::Value> = Vec::new();

    for (id, username, linux_user, old_home) in rows {
        if let Some(set) = &ids
            && !set.contains(&id)
        {
            continue;
        }
        let name = old_home.rsplit('/').next().unwrap_or("");
        if name.is_empty() {
            fail_items.push(json!({ "id": id, "username": username, "error": "家目录路径非法" }));
            continue;
        }
        let new_home = format!("{dest}/{name}");
        // 数据迁移中站点 vhost/FPM 会重建，先记录涉及站点
        let site_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM site WHERE user_id = ?")
            .bind(id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        // system 模式：迁移时同步更新 Linux 账号家目录指针
        let owner = if mode == "system" && !linux_user.is_empty() {
            Some(linux_user.clone())
        } else {
            None
        };
        let resp = match crate::zapexec::call(Request::UserHomeMigrate {
            src_home: old_home.clone(),
            dest_home: new_home.clone(),
            owner,
        })
        .await
        {
            Ok(r) => r,
            Err(e) => {
                fail_items.push(json!({
                    "id": id, "username": username,
                    "home_dir": old_home,
                    "error": format!("执行端通信失败：{e}"),
                }));
                continue;
            }
        };
        if resp.code != 0 {
            fail_items.push(json!({
                "id": id, "username": username,
                "home_dir": old_home,
                "error": format!("搬移失败：{}", resp.message),
            }));
            continue;
        }

        // 1) 更新用户家目录
        let _ = sqlx::query("UPDATE user SET home_dir = ? WHERE id = ?")
            .bind(&new_home)
            .bind(id)
            .execute(pool)
            .await;
        // 2) 站点路径前缀跟随（web_root / log_root）
        let _ = sqlx::query(
            "UPDATE site SET web_root = REPLACE(web_root, ?, ?), log_root = REPLACE(log_root, ?, ?)
             WHERE user_id = ?",
        )
        .bind(&old_home)
        .bind(&new_home)
        .bind(&old_home)
        .bind(&new_home)
        .bind(id)
        .execute(pool)
        .await;

        // 3) 涉及站点重新同步（nginx vhost / FPM pool 使用新路径）
        let mut site_errors: Vec<String> = Vec::new();
        let mut synced = 0;
        for sid in &site_ids {
            match crate::routers::site::sync_one_site(*sid).await {
                Ok(_) => synced += 1,
                Err(e) => site_errors.push(format!("site#{sid}: {e}")),
            }
        }

        ok_items.push(json!({
            "id": id,
            "username": username,
            "linux_user": linux_user,
            "old_home": old_home,
            "new_home": new_home,
            "sites": site_ids.len(),
            "sites_synced": synced,
            "site_errors": site_errors,
        }));
    }

    let ok_n = ok_items.len();
    let fail_n = fail_items.len();
    audit::log(
        Some(&claims),
        Some(client_addr.ip().to_string().as_str()),
        "user_home_migrate",
        &format!("{src} → {dest}"),
        &format!("ok={ok_n} fail={fail_n}"),
    )
    .await;

    Ok(Json(json!({
        "code": 0,
        "message": format!("迁移完成：成功 {ok_n}，失败 {fail_n}"),
        "data": { "src": src, "dest": dest, "mode": mode, "ok": ok_items, "fail": fail_items }
    })))
}
