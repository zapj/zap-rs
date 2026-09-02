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
    // AppStore: run records table + menu
    init_appstore_runs_table().await;
    // IP 池管理表
    init_ip_pool_table().await;
    // 用户站点管理表
    init_site_table().await;
    // API Token 管理表（幂等建表，新旧库均生效）
    init_api_token_table().await;
    // 「开发」菜单（API Tokens / API 文档，幂等补插）
    ensure_dev_menus().await;
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
        phone VARCHAR(32) UNIQUE,
        nickname TEXT,
        last_login_time INTEGER,
        last_login_ip TEXT,
        status INTEGER DEFAULT 1,
        roles TEXT,
        permissions TEXT,
        owner_id INTEGER DEFAULT 0,
        totp_secret TEXT NOT NULL DEFAULT '',
        totp_enabled INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER,
        updated_at INTEGER
    )
    "#;
    get_db_pool().await.execute(create_sql).await.unwrap();

    // Insert admin with runtime-generated bcrypt hash (avoids $2y$ prefix issues).
    // Password: use $ZAP_ADMIN_PASSWORD if set (fresh DBs only), otherwise default "123456".
    let default_password = std::env::var("ZAP_ADMIN_PASSWORD")
        .map(|p| p.trim().to_string())
        .unwrap_or_default();
    let default_password = if default_password.is_empty() {
        "123456".to_string()
    } else {
        default_password
    };
    let hashed = bcrypt::hash(&default_password, bcrypt::DEFAULT_COST)
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
    INSERT INTO roles (name, role_key, description, status, created_at, updated_at)
    VALUES ('演示', 'demo', '演示角色', 1, strftime('%s','now'), strftime('%s','now'));
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

    -- 站点管理（Layout 包裹 + 一级直链：单个子菜单，位于仪表盘之下）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (9, 0, 'site', '/site', 'Layout', '/site/index', 'menu', '站点', 'ep:aim', 1, 'admin,user,reseller', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (91, 9, 'site-index', 'index', 'site/index', 'menu', '站点', 'ep:aim', 1, 'admin,user,reseller', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- System dir
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (2, 0, 'system', '/system', 'Layout', '/system/user', 'dir', '系统设置', 'ep:setting', 1, 'admin', 8, 1, strftime('%s','now'), strftime('%s','now'));

    -- System children
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (21, 2, 'user', 'user', 'system/users/index', 'menu', '用户管理', 'ep:user', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (22, 2, 'roles', 'roles', 'system/roles/index', 'menu', '角色管理', 'ep:view', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (23, 2, 'menus', 'menus', 'system/menus/index', 'menu', '菜单管理', 'ep:menu', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (25, 2, 'ssh-keys', 'ssh-keys', 'system/config/ssh-keys', 'menu', 'SSH 密钥', 'ep:key', 1, 'admin', 4, 1, strftime('%s','now'), strftime('%s','now'));

    -- Server config dir
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (7, 0, 'server', '/server', 'Layout', '/server/time', 'dir', '服务器配置', 'ep:set-up', 1, 'admin', 9, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (71, 7, 'server-time', 'time', 'server/time/index', 'menu', '服务器时间', 'ep:clock', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (72, 7, 'server-services', 'services', 'server/services/index', 'menu', '系统服务', 'ep:tools', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (73, 7, 'server-ssh', 'ssh', 'server/ssh/index', 'menu', 'SSH 服务', 'ep:connection', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (74, 7, 'server-process', 'process', 'server/process/index', 'menu', '进程管理', 'ep:cpu', 1, 'admin', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (75, 7, 'server-network', 'network', 'server/network/index', 'menu', '网络设置', 'ep:link', 1, 'admin', 5, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (76, 7, 'server-ip', 'ip', 'server/ip/index', 'menu', 'IP 设置', 'ep:postcard', 1, 'admin', 6, 1, strftime('%s','now'), strftime('%s','now'));

    -- Terminal（Layout 包裹 + 一级直链：单个子菜单）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (4, 0, 'terminal', '/terminal', 'Layout', '/terminal/index', 'menu', '终端', 'ep:monitor', 1, 'admin,user', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (41, 4, 'terminal-index', 'index', 'terminal/index', 'menu', '终端', 'ep:monitor', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- File manager（Layout 包裹 + 一级直链：单个子菜单）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (3, 0, 'files', '/files', 'Layout', '/files/index', 'menu', '文件管理', 'ep:folder', 1, 'admin,user', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (31, 3, 'files-index', 'index', 'files/index', 'menu', '文件管理', 'ep:folder', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- Reseller customer management (Layout + child page)
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (5, 0, 'reseller-users', '/reseller/users', 'Layout', '/reseller/users/index', 'menu', '客户管理', 'ep:user-filled', 1, 'reseller', 5, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (51, 5, 'reseller-users-index', 'index', 'system/users/index', 'menu', '客户管理', 'ep:user-filled', 1, 'reseller', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- AppStore (Layout + children)
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (6, 0, 'appstore', '/appstore', 'Layout', '/appstore/index', 'menu', '应用商店', 'ep:goods', 1, 'admin,user', 6, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (61, 6, 'appstore-index', 'index', 'appstore/index', 'menu', '应用商店', 'ep:goods', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (62, 6, 'appstore-scripts', 'scripts', 'appstore/scripts', 'menu', '脚本管理', 'ep:document', 1, 'admin,user', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- Server status dir（应用商店之后）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (8, 0, 'server-status', '/server-status', 'Layout', '/server-status/info', 'dir', '服务器状态', 'ep:data-line', 1, 'admin', 7, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (81, 8, 'server-status-info', 'info', 'server-status/info/index', 'menu', '服务器信息', 'ep:info-filled', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (82, 8, 'server-status-load', 'load', 'server-status/load/index', 'menu', '系统负载', 'ep:odometer', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (83, 8, 'server-status-network', 'network', 'server-status/network/index', 'menu', '网络', 'ep:share', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (84, 8, 'server-status-memory', 'memory', 'server-status/memory/index', 'menu', '内存', 'ep:coin', 1, 'admin', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (85, 8, 'server-status-cpu', 'cpu', 'server-status/cpu/index', 'menu', 'CPU', 'ep:cpu', 1, 'admin', 5, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (86, 8, 'server-status-disk', 'disk', 'server-status/disk/index', 'menu', '硬盘', 'ep:box', 1, 'admin', 6, 1, strftime('%s','now'), strftime('%s','now'));
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
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 25);
    -- User gets dashboard only
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 1);
    -- File manager: admin gets all, user gets read access
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 3);
    -- Terminal: both admin and user
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 4);
    -- Terminal / File manager 子菜单授权（admin/user/reseller/demo）
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 41);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 31);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 31);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 31);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 31);
    -- Reseller: same base permissions as user + customer management
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 1);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 5);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 51);
    -- AppStore: admin / user / reseller 均可访问
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 62);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 62);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 62);
    -- Demo: dashboard, files, terminal, appstore（与普通用户一致）
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 1);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 62);
    -- Server status: admin 专属
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 8);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 81);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 82);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 83);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 84);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 85);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 86);
    -- Server config: admin 专属
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 7);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 71);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 72);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 73);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 74);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 75);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 76);
    -- 站点管理：admin 全部 / user 自己的站点 / reseller 所属客户的站点
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 91);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 91);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 91);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
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

// ── appstore ────────────────────────────────────────────────

async fn init_appstore_runs_table() {
    if table_exists("appstore_runs").await {
        return;
    }
    let sql = r#"
    CREATE TABLE appstore_runs (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        run_id TEXT NOT NULL UNIQUE,
        action TEXT NOT NULL DEFAULT '',
        pkg TEXT NOT NULL DEFAULT '',
        username TEXT NOT NULL DEFAULT '',
        status TEXT NOT NULL DEFAULT 'running',
        exit_code INTEGER NOT NULL DEFAULT -1,
        log_path TEXT NOT NULL DEFAULT '',
        started_at INTEGER NOT NULL DEFAULT 0,
        finished_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX idx_appstore_runs_started ON appstore_runs(started_at);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── ip_pool（IP 池管理）─────────────────────────────────────

async fn init_ip_pool_table() {
    if table_exists("ip_pool").await {
        return;
    }
    let sql = r#"
    CREATE TABLE ip_pool (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        address TEXT NOT NULL UNIQUE,
        version INTEGER NOT NULL DEFAULT 4,
        ip_type TEXT NOT NULL DEFAULT 'shared',
        reserved INTEGER NOT NULL DEFAULT 0,
        remark TEXT NOT NULL DEFAULT '',
        created_at INTEGER,
        updated_at INTEGER
    );
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── site（用户站点管理）─────────────────────────────────────

async fn init_site_table() {
    if table_exists("site").await {
        return;
    }
    let sql = r#"
    -- 站点主表：一个站点可绑定多个域名 / 多个 IP（见 site_domain / site_ip）
    CREATE TABLE site (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL DEFAULT 0,
        name TEXT NOT NULL DEFAULT '',
        status INTEGER NOT NULL DEFAULT 1,
        remark TEXT NOT NULL DEFAULT '',
        created_at INTEGER,
        updated_at INTEGER
    );
    CREATE INDEX idx_site_user_id ON site(user_id);

    CREATE TABLE site_domain (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        site_id INTEGER NOT NULL DEFAULT 0,
        domain TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX idx_site_domain_site_id ON site_domain(site_id);
    CREATE UNIQUE INDEX idx_site_domain_domain ON site_domain(domain);

    CREATE TABLE site_ip (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        site_id INTEGER NOT NULL DEFAULT 0,
        ip TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX idx_site_ip_site_id ON site_ip(site_id);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── api_token（API Token 管理）──────────────────────────────

async fn init_api_token_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS api_token (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL DEFAULT 0,
        name TEXT NOT NULL DEFAULT '',
        token_hash TEXT NOT NULL DEFAULT '',
        prefix TEXT NOT NULL DEFAULT '',
        last_used_at INTEGER NOT NULL DEFAULT 0,
        expires_at INTEGER NOT NULL DEFAULT 0,
        status INTEGER NOT NULL DEFAULT 1,
        created_at INTEGER,
        updated_at INTEGER
    );
    CREATE UNIQUE INDEX IF NOT EXISTS idx_api_token_hash ON api_token(token_hash);
    CREATE INDEX IF NOT EXISTS idx_api_token_user ON api_token(user_id);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

/// 幂等补插「开发」顶级菜单及子菜单（API Tokens / API 文档），并授权 admin/user/reseller。
/// 可安全重复执行：已存在则跳过（新库/旧库均适用，无需重置数据库）。
async fn ensure_dev_menus() {
    let pool = get_db_pool().await;
    // 顶级目录：开发（sort_order=10，位于最下方）
    let _ = pool
        .execute(
            r#"
        INSERT INTO menus (parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
        SELECT 0, 'dev', '/dev', 'Layout', '/dev/api-tokens', 'dir', '开发', 'ep:tools', 1, 'admin,user,reseller', 10, 1, strftime('%s','now'), strftime('%s','now')
        WHERE NOT EXISTS (SELECT 1 FROM menus WHERE parent_id = 0 AND name = 'dev')
        "#,
        )
        .await;
    // 子菜单：API Tokens
    let _ = pool
        .execute(
            r#"
        INSERT INTO menus (parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
        SELECT id, 'api-tokens', 'api-tokens', 'dev/api-tokens/index', 'menu', 'API Tokens', 'ep:key', 1, 'admin,user,reseller', 1, 1, strftime('%s','now'), strftime('%s','now')
        FROM menus
        WHERE parent_id = 0 AND name = 'dev'
          AND NOT EXISTS (
              SELECT 1 FROM menus m2
              WHERE m2.parent_id = (SELECT id FROM menus WHERE parent_id = 0 AND name = 'dev')
                AND m2.name = 'api-tokens'
          )
        "#,
        )
        .await;
    // 子菜单：API 文档
    let _ = pool
        .execute(
            r#"
        INSERT INTO menus (parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
        SELECT id, 'api-docs', 'api-docs', 'dev/api-docs/index', 'menu', 'API 文档', 'ep:document', 1, 'admin,user,reseller', 2, 1, strftime('%s','now'), strftime('%s','now')
        FROM menus
        WHERE parent_id = 0 AND name = 'dev'
          AND NOT EXISTS (
              SELECT 1 FROM menus m3
              WHERE m3.parent_id = (SELECT id FROM menus WHERE parent_id = 0 AND name = 'dev')
                AND m3.name = 'api-docs'
          )
        "#,
        )
        .await;
    // role_menus 授权：admin / user / reseller ×（dev、api-tokens、api-docs）
    let _ = pool
        .execute(
            r#"
        INSERT INTO role_menus (role_id, menu_id)
        SELECT r.id, m.id
        FROM roles r
        JOIN menus m
          ON m.name IN ('dev', 'api-tokens', 'api-docs')
         AND (m.parent_id = 0 OR m.parent_id IN (SELECT id FROM menus WHERE parent_id = 0 AND name = 'dev'))
        WHERE r.role_key IN ('admin', 'user', 'reseller')
          AND NOT EXISTS (
              SELECT 1 FROM role_menus x WHERE x.role_id = r.id AND x.menu_id = m.id
          )
        "#,
        )
        .await;
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
