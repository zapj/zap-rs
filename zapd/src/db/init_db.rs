use sqlx::Executor;

use super::get_db_pool;

pub async fn init_schema() {
    init_system_user_table_schema().await;
    init_system_monitor_table_schema().await;
    init_system_monitor_networks_table_schema().await;
    init_roles_table().await;
    init_menus_table().await;
    init_role_menus_table().await;
    init_audit_table().await;
    init_login_attempts_table().await;
    init_hourly_stats_tables().await;
    crate::routers::ssh_terminal::init_table().await;

    // Migration: fix empty path on existing child menus
    fix_empty_menu_paths().await;
    // Migration: ensure reseller role + base permissions exist on upgraded DBs
    ensure_reseller_role().await;
    // Migration: add owner_id column to user table (reseller customer ownership)
    ensure_user_owner_id().await;
    // Migration: rename 系统管理 -> 系统设置
    fix_system_menu_title().await;
    // Migration: add TOTP 2FA columns to user table
    ensure_user_totp_columns().await;
    // Migration: encrypt legacy plaintext SSH passwords
    crate::zap::crypto::migrate_legacy_passwords().await;
}

// ── user ───────────────────────────────────────────────────

async fn init_system_user_table_schema() {
    if table_exists("user").await {
        return;
    }
    // Create table
    let create_sql = r#"
    CREATE TABLE user (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        username VARCHAR(128) UNIQUE NOT NULL,
        password VARCHAR(256) NOT NULL,
        email VARCHAR(256) UNIQUE NOT NULL,
        phone VARCHAR(32) UNIQUE NOT NULL,
        nickname TEXT,
        last_login_time INTEGER,
        last_login_ip TEXT,
        status INTEGER DEFAULT 1,
        roles TEXT,
        permissions TEXT,
        owner_id INTEGER DEFAULT 0,
        created_at INTEGER,
        updated_at INTEGER
    )
    "#;
    get_db_pool().await.execute(create_sql).await.unwrap();

    // Insert admin with runtime-generated bcrypt hash (avoids $2y$ prefix issues)
    let default_password = "123456";
    let hashed = bcrypt::hash(default_password, bcrypt::DEFAULT_COST)
        .expect("failed to hash default password");
    let now = chrono::Utc::now().timestamp();

    sqlx::query(
        "INSERT INTO user (username, password, email, nickname, phone, last_login_time, last_login_ip, status, roles, permissions, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 1, 'admin', '', ?, ?)",
    )
    .bind("admin")
    .bind(&hashed)
    .bind("admin@demo.zap.cn")
    .bind("admin")
    .bind("18826002600")
    .bind(now)
    .bind("127.0.0.1")
    .bind(now)
    .bind(now)
    .execute(get_db_pool().await)
    .await
    .unwrap();
}

// ── monitor ────────────────────────────────────────────────

async fn init_system_monitor_table_schema() {
    if table_exists("system_stats").await {
        return;
    }
    let sql_script = r#"
    CREATE TABLE system_stats (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        loadavg_one REAL,
        loadavg_five REAL,
        loadavg_fifteen REAL,
        cpu_usage REAL,
        memory_usage REAL,
        swap_usage REAL,
        created_at BIGINT
    )
    "#;
    let _ = get_db_pool().await.execute(sql_script).await;
}

async fn init_system_monitor_networks_table_schema() {
    if table_exists("networks_stats").await {
        return;
    }
    let sql_script = r#"
    CREATE TABLE networks_stats (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        name TEXT,
        received BIGINT,
        transmitted BIGINT,
        errors_on_received BIGINT,
        errors_on_transmitted BIGINT,
        packets_received BIGINT,
        packets_transmitted BIGINT,
        total_received BIGINT,
        total_transmitted BIGINT,
        total_packets_received BIGINT,
        total_packets_transmitted BIGINT,
        total_errors_on_received BIGINT,
        total_errors_on_transmitted BIGINT,
        ipaddrs TEXT,
        created_at BIGINT
    )
    "#;
    let _ = get_db_pool().await.execute(sql_script).await;
}

// ── roles ──────────────────────────────────────────────────

async fn init_roles_table() {
    if table_exists("roles").await {
        return;
    }
    let sql = r#"
    CREATE TABLE roles (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        name VARCHAR(64) UNIQUE NOT NULL,
        role_key VARCHAR(64) UNIQUE NOT NULL,
        description TEXT DEFAULT '',
        status INTEGER DEFAULT 1,
        created_at INTEGER,
        updated_at INTEGER
    );
    INSERT INTO roles (name, role_key, description, status, created_at, updated_at)
    VALUES ('管理员', 'admin', '系统最高权限角色', 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO roles (name, role_key, description, status, created_at, updated_at)
    VALUES ('普通用户', 'user', '普通用户角色', 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO roles (name, role_key, description, status, created_at, updated_at)
    VALUES ('经销商', 'reseller', '经销商角色', 1, strftime('%s','now'), strftime('%s','now'));
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── menus ──────────────────────────────────────────────────

async fn init_menus_table() {
    if table_exists("menus").await {
        return;
    }
    let sql = r#"
    CREATE TABLE menus (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        parent_id INTEGER DEFAULT 0,
        name VARCHAR(64) NOT NULL,
        path VARCHAR(128) NOT NULL DEFAULT '',
        component VARCHAR(256) DEFAULT '',
        redirect VARCHAR(128) DEFAULT '',
        type VARCHAR(16) NOT NULL DEFAULT 'menu',
        title VARCHAR(64) NOT NULL DEFAULT '',
        icon VARCHAR(64) DEFAULT '',
        hidden INTEGER DEFAULT 0,
        keep_alive INTEGER DEFAULT 0,
        affix INTEGER DEFAULT 0,
        roles TEXT DEFAULT '',
        sort_order INTEGER DEFAULT 0,
        status INTEGER DEFAULT 1,
        created_at INTEGER,
        updated_at INTEGER
    );

    -- Dashboard
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (1, 0, 'dashboard', '/dashboard', 'dashboard/index', '', 'menu', '仪表盘', 'ep:house', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- System dir
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (2, 0, 'system', '/system', 'Layout', '/system/user', 'dir', '系统设置', 'ep:setting', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- System children
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (21, 2, 'user', 'user', 'system/users/index', 'menu', '用户管理', 'ep:user', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (22, 2, 'roles', 'roles', 'system/roles/index', 'menu', '角色管理', 'ep:view', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (23, 2, 'menus', 'menus', 'system/menus/index', 'menu', '菜单管理', 'ep:menu', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (24, 2, 'config', 'config', 'system/config/index', 'menu', '服务配置', 'ep:tools', 1, 'admin', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (25, 2, 'ssh-keys', 'ssh-keys', 'system/config/ssh-keys', 'menu', 'SSH 密钥', 'ep:key', 1, 'admin', 5, 1, strftime('%s','now'), strftime('%s','now'));

    -- Terminal
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (4, 0, 'terminal', '/terminal', 'Layout', '/terminal', 'menu', '终端', 'ep:monitor', 1, 'admin,user', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (41, 4, 'ssh-terminal', 'index', 'terminal/index', 'menu', 'SSH 终端', 'ep:connection', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- File manager
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (3, 0, 'files', '/files', 'Layout', '/files', 'menu', '文件管理', 'ep:folder', 1, 'admin,user', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (31, 3, 'file-manager', 'index', 'files/index', 'menu', '文件管理器', 'ep:folder-opened', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- Reseller customer management (Layout + child page)
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (5, 0, 'reseller-users', '/reseller/users', 'Layout', '/reseller/users/index', 'menu', '客户管理', 'ep:user-filled', 1, 'reseller', 5, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (51, 5, 'reseller-users-index', 'index', 'system/users/index', 'menu', '客户管理', '', 1, 'reseller', 1, 1, strftime('%s','now'), strftime('%s','now'));
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── role_menus ─────────────────────────────────────────────

async fn init_role_menus_table() {
    if table_exists("role_menus").await {
        return;
    }
    let sql = r#"
    CREATE TABLE role_menus (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        role_id INTEGER NOT NULL,
        menu_id INTEGER NOT NULL,
        UNIQUE(role_id, menu_id)
    );
    -- Admin gets all menu IDs
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 1);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 2);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 21);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 22);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 23);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 24);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 25);
    -- User gets dashboard only
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 1);
    -- File manager: admin gets all, user gets read access
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 31);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 31);
    -- Terminal: both admin and user
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 41);
    -- Reseller: same base permissions as user + customer management
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 1);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 31);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 5);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 51);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── migrations ─────────────────────────────────────────────

/// Fix child menus that have empty path (causes frontend menuToRoute to crash)
async fn fix_empty_menu_paths() {
    let pool = get_db_pool().await;

    let result = sqlx::query(
        "UPDATE menus SET path = 'index' WHERE parent_id > 0 AND (path = '' OR path IS NULL)"
    )
    .execute(pool)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Fixed {} menu(s) with empty path", r.rows_affected());
        }
        _ => {}
    }
}

/// Ensure reseller role and its base menu permissions exist (idempotent).
/// Needed for databases created before the reseller role was introduced.
async fn ensure_reseller_role() {
    let pool = get_db_pool().await;
    let now = chrono::Local::now().timestamp();

    // 0. Ensure the customer-management menu exists with Layout wrapper (upgraded DBs)
    let _ = sqlx::query(
        "INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
         VALUES (5, 0, 'reseller-users', '/reseller/users', 'Layout', '/reseller/users/index', 'menu', '客户管理', 'ep:user-filled', 1, 'reseller', 5, 1, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            parent_id = 0, name = 'reseller-users', path = '/reseller/users', component = 'Layout', redirect = '/reseller/users/index', type = 'menu', title = '客户管理', icon = 'ep:user-filled', affix = 1, roles = 'reseller', sort_order = 5, status = 1",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
         VALUES (51, 5, 'reseller-users-index', 'index', 'system/users/index', 'menu', '客户管理', '', 1, 'reseller', 1, 1, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            parent_id = 5, name = 'reseller-users-index', path = 'index', component = 'system/users/index', type = 'menu', title = '客户管理', icon = '', affix = 1, roles = 'reseller', sort_order = 1, status = 1",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    // 1. Ensure the reseller role exists
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO roles (name, role_key, description, status, created_at, updated_at)
         VALUES ('经销商', 'reseller', '经销商角色', 1, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await;

    // 2. Resolve the role id (roles uses AUTOINCREMENT)
    let role_id: i64 = match sqlx::query_as("SELECT id FROM roles WHERE role_key = 'reseller'")
        .fetch_optional(pool)
        .await
    {
        Ok(Some((id,))) => id,
        _ => return,
    };

    // 3. Ensure base menu permissions (same as user + customer management)
    for menu_id in [1i64, 3, 31, 4, 41, 5, 51] {
        let _ = sqlx::query("INSERT OR IGNORE INTO role_menus (role_id, menu_id) VALUES (?, ?)")
            .bind(role_id)
            .bind(menu_id)
            .execute(pool)
            .await;
    }
}

/// Add owner_id column to the user table for existing databases (idempotent).
async fn ensure_user_owner_id() {
    if column_exists("user", "owner_id").await {
        return;
    }
    let pool = get_db_pool().await;
    let _ = sqlx::query("ALTER TABLE user ADD COLUMN owner_id INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    tracing::info!("Added user.owner_id column");
}

/// Rename the "系统管理" top menu to "系统设置" for existing databases.
async fn fix_system_menu_title() {
    let pool = get_db_pool().await;
    let _ = sqlx::query(
        "UPDATE menus SET title = '系统设置' WHERE name = 'system' AND title = '系统管理'",
    )
    .execute(pool)
    .await;
}

// ── audit logs ─────────────────────────────────────────────

async fn init_audit_table() {
    if table_exists("audit_logs").await {
        return;
    }
    let sql = r#"
    CREATE TABLE audit_logs (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL DEFAULT 0,
        username VARCHAR(128) NOT NULL DEFAULT '',
        action VARCHAR(64) NOT NULL DEFAULT '',
        target TEXT NOT NULL DEFAULT '',
        detail TEXT NOT NULL DEFAULT '',
        ip VARCHAR(64) NOT NULL DEFAULT '',
        created_at INTEGER
    );
    CREATE INDEX idx_audit_logs_action ON audit_logs(action);
    CREATE INDEX idx_audit_logs_user ON audit_logs(username);
    CREATE INDEX idx_audit_logs_created ON audit_logs(created_at);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── login attempt lockout ──────────────────────────────────

async fn init_login_attempts_table() {
    if table_exists("login_attempts").await {
        return;
    }
    let sql = r#"
    CREATE TABLE login_attempts (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        username VARCHAR(128) NOT NULL DEFAULT '',
        ip VARCHAR(64) NOT NULL DEFAULT '',
        failed_count INTEGER NOT NULL DEFAULT 0,
        locked_until INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER,
        UNIQUE(username, ip)
    );
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── hourly aggregated monitoring stats ─────────────────────

async fn init_hourly_stats_tables() {
    if !table_exists("system_stats_hourly").await {
        let sql = r#"
        CREATE TABLE system_stats_hourly (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            hour_start INTEGER NOT NULL,
            avg_loadavg_one REAL,
            avg_cpu_usage REAL,
            max_cpu_usage REAL,
            avg_memory_usage REAL,
            max_memory_usage REAL,
            avg_swap_usage REAL,
            UNIQUE(hour_start)
        );
        "#;
        let _ = get_db_pool().await.execute(sql).await;
    }
    if !table_exists("networks_stats_hourly").await {
        let sql = r#"
        CREATE TABLE networks_stats_hourly (
            id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            hour_start INTEGER NOT NULL,
            avg_received REAL,
            avg_transmitted REAL,
            max_received REAL,
            max_transmitted REAL,
            UNIQUE(name, hour_start)
        );
        "#;
        let _ = get_db_pool().await.execute(sql).await;
    }
}

// ── migrations ─────────────────────────────────────────────

/// Add TOTP 2FA columns to the user table (idempotent).
async fn ensure_user_totp_columns() {
    let pool = get_db_pool().await;
    if !column_exists("user", "totp_secret").await {
        let _ = sqlx::query("ALTER TABLE user ADD COLUMN totp_secret TEXT NOT NULL DEFAULT ''")
            .execute(pool)
            .await;
        tracing::info!("Added user.totp_secret column");
    }
    if !column_exists("user", "totp_enabled").await {
        let _ = sqlx::query("ALTER TABLE user ADD COLUMN totp_enabled INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        tracing::info!("Added user.totp_enabled column");
    }
}

// ── helper ─────────────────────────────────────────────────

async fn table_exists(table_name: &str) -> bool {
    let pool = get_db_pool().await;
    let result: Result<(String,), sqlx::Error> =
        sqlx::query_as("select name from sqlite_master where name = ?")
            .bind(table_name)
            .fetch_one(pool)
            .await;
    result.is_ok()
}

async fn column_exists(table_name: &str, column_name: &str) -> bool {
    let pool = get_db_pool().await;
    let sql = format!(
        "SELECT name FROM pragma_table_info('{}') WHERE name = '{}'",
        table_name, column_name
    );
    let result: Result<Option<String>, sqlx::Error> =
        sqlx::query_scalar(&sql).fetch_optional(pool).await;
    matches!(result, Ok(Some(_)))
}