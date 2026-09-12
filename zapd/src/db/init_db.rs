use sqlx::Executor;

use super::get_db_pool;

/// 建表与种子数据入口。
///
/// **约定（开发阶段）**：数据库通过 `--reset-db` 重建，因此这里**不做任何老库兼容迁移**
/// —— 没有 `ALTER TABLE ... ADD COLUMN` 兜底、没有数据搬运、没有版本水位表。
/// 改字段请直接改对应的 `CREATE TABLE`，然后重置数据库。
pub async fn init_schema() {
    init_system_user_table_schema().await;
    init_system_monitor_table_schema().await;
    init_system_monitor_networks_table_schema().await;
    init_roles_table().await;
    init_menus_table().await;
    init_role_menus_table().await;
    // 动作级权限点（请求级鉴权依据；role_menus 仅用于菜单渲染）
    init_role_permissions_table().await;
    init_audit_table().await;
    init_login_attempts_table().await;
    init_hourly_stats_tables().await;
    crate::routers::ssh_terminal::init_table().await;
    // AppStore: run records table + menu
    init_appstore_runs_table().await;
    // 脚本/自动化：计划任务表
    init_cron_jobs_table().await;
    // IP 池管理表
    init_ip_pool_table().await;
    // 用户站点管理表
    init_site_table().await;
    // 站点扩展档案（类型/伪静态/upstream/location/自定义目录 + TLS 高级设置）
    init_site_profile_table().await;
    // PHP-FPM 规格模板表（user.fpm_spec_ref 已在 user 表中定义）
    init_fpm_spec_table().await;
    // 套餐（Packages）表：创建客户时可选择的资源套餐
    init_packages_table().await;
    // 站内信（通知中心）表
    init_notice_message_table().await;
    // 全局运行环境状态表（scope=auto 自动探测快照 / scope=conf 面板默认配置）
    init_server_env_table().await;
    // API Token 管理表
    init_api_token_table().await;
    // SSL/TLS 证书管理表
    init_ssl_cert_table().await;
    // 系统更新：自动更新配置表
    init_update_config_table().await;
    // 菜单（menus/role_menus）与角色权限（role_permissions）为静态基础数据：
    // SSL/TLS、应用商店（含已安装应用）、服务器状态、脚本/自动化（自定义脚本+计划任务）、
    // 系统设置（含审计日志、Zap 设置、系统更新）、服务器配置、开发 —— 一次 seed 到位，
    // 不做运行时补插；重置数据库后回到初始状态。
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
        home_dir TEXT NOT NULL DEFAULT '',
        linux_user TEXT NOT NULL DEFAULT '',
        fpm_pool TEXT NOT NULL DEFAULT '',
        -- fpm_spec_ref：''=面板全局默认 / 'inherit'=继承 owner(reseller) 名下默认 / 模板名
        fpm_spec_ref TEXT NOT NULL DEFAULT '',
        -- prefs：个人中心 → 偏好设置（JSON 字符串）
        prefs TEXT NOT NULL DEFAULT '',
        last_login_time INTEGER,
        last_login_ip TEXT,
        status INTEGER DEFAULT 1,
        roles TEXT,
        permissions TEXT,
        owner_id INTEGER DEFAULT 0,
        package_id INTEGER NOT NULL DEFAULT 0,
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
        "INSERT INTO user (username, home_dir, linux_user, password, email, nickname, phone, last_login_time, last_login_ip, status, roles, permissions, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 'admin', '', ?, ?)",
    )
    .bind("admin")
    .bind("/home/admin")
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

// ── packages（套餐）────────────────────────────────────────

/// 套餐（Packages）：创建客户时选用的资源套餐（对齐 cPanel/WHM 的 Packages）。
/// - owner_id = 0：全局套餐（admin 维护，所有人可用）
/// - owner_id != 0：reseller 自建套餐，仅创建者自己可用
/// - 限制项：磁盘配额 / 最大站点数 / 单站点域名数 / 月流量（仅记录）/ MySQL 与 MariaDB 库数 /
///   PostgreSQL 库数 / FTP 用户数 / FPM 规格模板 / SSH 终端开关
/// - 能力项：allow_proxy（普通用户可用反向代理）；「自定义目录」不再作为套餐能力，
///   已对全部用户开放（home 目录内任意目录可选）
/// - 数值 0 表示「不限」
async fn init_packages_table() {
    if table_exists("packages").await {
        return;
    }
    let sql = r#"
    CREATE TABLE packages (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        name VARCHAR(64) UNIQUE NOT NULL,
        remark TEXT NOT NULL DEFAULT '',
        disk_quota_mb INTEGER NOT NULL DEFAULT 0,
        max_sites INTEGER NOT NULL DEFAULT 0,
        max_domains INTEGER NOT NULL DEFAULT 0,
        max_bandwidth_mb INTEGER NOT NULL DEFAULT 0,
        -- 用户可创建的 MySQL / MariaDB 数据库数量（0 = 不限，建库时硬拦截）
        max_mysql_dbs INTEGER NOT NULL DEFAULT 0,
        -- 用户可创建的 PostgreSQL 数据库数量（0 = 不限，仅记录与展示）
        max_pgsql_dbs INTEGER NOT NULL DEFAULT 0,
        -- 用户可创建的 FTP 账号数量（0 = 不限，仅记录与展示）
        max_ftp_users INTEGER NOT NULL DEFAULT 0,
        fpm_spec_ref TEXT NOT NULL DEFAULT '',
        allow_ssh INTEGER NOT NULL DEFAULT 0,
        allow_proxy INTEGER NOT NULL DEFAULT 0,
        owner_id INTEGER NOT NULL DEFAULT 0,
        status INTEGER NOT NULL DEFAULT 1,
        created_at INTEGER,
        updated_at INTEGER
    );
    INSERT INTO packages (name, remark, disk_quota_mb, max_sites, max_domains, max_bandwidth_mb, max_mysql_dbs, max_pgsql_dbs, max_ftp_users, fpm_spec_ref, allow_ssh, allow_proxy, owner_id, status, created_at, updated_at)
    VALUES ('默认套餐', '不限磁盘、不限站点、不限域名、不限数据库与 FTP 账号数，允许 SSH 终端（反向代理默认关闭，可在「编辑套餐」中开启；自定义目录已全量开放）', 0, 0, 0, 0, 0, 0, 0, '', 1, 0, 0, 1, strftime('%s','now'), strftime('%s','now'));
    "#;
    let _ = get_db_pool().await.execute(sql).await;
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

    -- 数据库管理（Layout 包裹 + 一级直链：紧随站点之后；user 仅能管自己前缀的库）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (14, 0, 'database', '/database', 'Layout', '/database/index', 'menu', '数据库', 'ep:coin', 1, 'admin,user', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (141, 14, 'database-index', 'index', 'database/index', 'menu', '数据库', 'ep:coin', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- System dir
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (2, 0, 'system', '/system', 'Layout', '/system/user', 'dir', '系统设置', 'ep:setting', 1, 'admin', 12, 1, strftime('%s','now'), strftime('%s','now'));

    -- System children
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (26, 2, 'basic-config', 'basic-config', 'system/config/basic', 'menu', '基础设置', 'ep:set-up', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (28, 2, 'zap-config', 'zap-config', 'system/config/zap', 'menu', 'Zap 设置', 'ep:operation', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (21, 2, 'user', 'user', 'system/users/index', 'menu', '用户管理', 'ep:user', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (22, 2, 'roles', 'roles', 'system/roles/index', 'menu', '角色管理', 'ep:view', 1, 'admin', 4, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (23, 2, 'menus', 'menus', 'system/menus/index', 'menu', '菜单管理', 'ep:menu', 1, 'admin', 5, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (25, 2, 'ssh-keys', 'ssh-keys', 'system/config/ssh-keys', 'menu', 'SSH 密钥', 'ep:key', 1, 'admin', 6, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (24, 2, 'audit', 'audit', 'system/audit/index', 'menu', '审计日志', 'ep:tickets', 1, 'admin', 7, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (27, 2, 'system-update', 'update', 'system/update/index', 'menu', '系统更新', 'ep:refresh', 1, 'admin', 8, 1, strftime('%s','now'), strftime('%s','now'));

    -- Server config dir（服务器配置，紧随服务配置之后；运维项）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (7, 0, 'server', '/server', 'Layout', '/server/time', 'dir', '服务器配置', 'ep:set-up', 1, 'admin', 10, 1, strftime('%s','now'), strftime('%s','now'));

    -- Service config dir（服务配置：运行服务（应用商店安装）的配置页，位于服务器配置上方）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (13, 0, 'services', '/services', 'Layout', '/services/nginx', 'dir', '服务配置', 'ep:service', 1, 'admin', 9, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (131, 13, 'service-nginx', 'nginx', 'services/nginx/index', 'menu', 'Nginx 配置', 'ep:document', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (132, 13, 'service-php', 'php', 'services/php/index', 'menu', 'PHP 配置', 'ep:coin', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (133, 13, 'service-mysql', 'mysql', 'services/mysql/index', 'menu', 'MySQL / MariaDB', 'ep:data-analysis', 1, 'admin', 3, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (135, 13, 'service-docker', 'docker', 'services/docker/index', 'menu', 'Docker 服务', 'ep:box', 1, 'admin', 5, 1, strftime('%s','now'), strftime('%s','now'));

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
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (80, 7, 'server-firewall', 'firewall', 'server/firewall/index', 'menu', '防火墙', 'ep:lock', 1, 'admin', 7, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (77, 7, 'server-env', 'env', 'server/env/index', 'menu', '运行环境', 'ep:magic-stick', 1, 'admin', 8, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (78, 7, 'server-entities', 'entities', 'server/entities/index', 'menu', '同步运行环境', 'ep:user-filled', 1, 'admin', 9, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (79, 7, 'server-migrate', 'migrate', 'server/migrate/index', 'menu', '数据迁移', 'ep:sort', 1, 'admin', 10, 1, strftime('%s','now'), strftime('%s','now'));

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
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (52, 5, 'reseller-packages', 'packages', 'system/packages/index', 'menu', '套餐', 'ep:goods', 0, 'admin,reseller', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- SSL/TLS（Layout + 子菜单，位于应用商店之前，admin/user）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (11, 0, 'ssl-tls', '/ssl-tls', 'Layout', '/ssl-tls/certs', 'dir', 'SSL/TLS', 'ep:lock', 1, 'admin,user', 6, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (111, 11, 'ssl-certs', 'certs', 'ssl-tls/certs/index', 'menu', 'SSL证书', 'ep:lock', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));

    -- AppStore (Layout + children)
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (6, 0, 'appstore', '/appstore', 'Layout', '/appstore/index', 'menu', '应用商店', 'ep:goods', 1, 'admin,user', 7, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (61, 6, 'appstore-index', 'index', 'appstore/index', 'menu', '应用商店', 'ep:goods', 1, 'admin,user', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (62, 6, 'installed', 'installed', 'appstore/installed', 'menu', '已安装应用', 'ep:box', 1, 'admin,user,reseller', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- Server status（子菜单：服务器信息 tabs + Nginx Server，应用商店之后，admin）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (8, 0, 'server-status', '/server-status', 'Layout', '/server-status/index', 'dir', '服务器状态', 'ep:data-line', 1, 'admin', 8, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (81, 8, 'server-status-index', 'index', 'server-status/index', 'menu', '服务器信息', 'ep:info-filled', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (87, 8, 'server-status-nginx', 'nginx-server', 'server-status/nginx-server/index', 'menu', 'Nginx Server', 'ep:monitor', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- 脚本/自动化（Layout + 子菜单，仅 admin，位于服务器配置之后）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (10, 0, 'automation', '/automation', 'Layout', '/automation/scripts', 'dir', '脚本/自动化', 'ep:timer', 1, 'admin', 11, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (101, 10, 'appstore-scripts', 'scripts', 'automation/scripts/index', 'menu', '自定义脚本', 'ep:document', 1, 'admin', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (102, 10, 'script-cron', 'cron', 'automation/cron/index', 'menu', '计划任务', 'ep:alarm-clock', 1, 'admin', 2, 1, strftime('%s','now'), strftime('%s','now'));

    -- Dev（Layout + 子菜单，位于最下方，admin/user/reseller）
    INSERT INTO menus (id, parent_id, name, path, component, redirect, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (12, 0, 'dev', '/dev', 'Layout', '/dev/api-tokens', 'dir', '开发', 'ep:tools', 1, 'admin,user,reseller', 13, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (121, 12, 'api-tokens', 'api-tokens', 'dev/api-tokens/index', 'menu', 'API Tokens', 'ep:key', 1, 'admin,user,reseller', 1, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (122, 12, 'api-docs', 'api-docs', 'dev/api-docs/index', 'menu', 'API 文档', 'ep:document', 1, 'admin,user,reseller', 2, 1, strftime('%s','now'), strftime('%s','now'));
    INSERT INTO menus (id, parent_id, name, path, component, type, title, icon, affix, roles, sort_order, status, created_at, updated_at)
    VALUES (123, 12, 'app-script-guide', 'app-script-guide', 'dev/app-script-guide/index', 'menu', '应用脚本编写', 'ep:notebook', 1, 'admin,user,reseller', 3, 1, strftime('%s','now'), strftime('%s','now'));
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
    -- 套餐（Packages）：admin 与 reseller 均可使用
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 52);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 52);
    -- AppStore: admin / user / reseller 均可访问
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 61);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 61);
    -- Demo: dashboard, files, terminal, appstore（与普通用户一致）
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 1);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 3);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 4);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 6);
    INSERT INTO role_menus (role_id, menu_id) VALUES (4, 61);
    -- Server status: admin 专属（服务器信息 tabs 81 + Nginx Server 87）
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 8);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 81);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 87);
    -- Server config: admin 专属（Nginx 配置已移入服务配置）
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 7);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 71);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 72);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 73);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 74);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 75);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 76);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 80);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 77);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 78);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 79);
    -- 站点管理：admin 全部 / user 自己的站点 / reseller 所属客户的站点
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 9);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 91);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 91);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 91);
    -- 数据库管理：admin 全部 / user 仅自己前缀的库
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 14);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 14);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 141);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 141);
    -- SSL/TLS：admin / user
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 11);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 111);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 11);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 111);
    -- 服务配置（Nginx/PHP/MySQL 与 MariaDB 合一/Docker）：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 13);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 131);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 132);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 133);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 135);
    -- 已安装应用：admin / user / reseller
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 62);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 62);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 62);
    -- 基础设置：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 26);
    -- Zap 设置：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 28);
    -- 审计日志：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 24);
    -- 系统更新：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 27);
    -- 脚本/自动化（自定义脚本 + 计划任务）：仅 admin
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 10);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 101);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 102);
    -- 开发：admin / user / reseller
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 12);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 121);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 122);
    INSERT INTO role_menus (role_id, menu_id) VALUES (1, 123);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 12);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 121);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 122);
    INSERT INTO role_menus (role_id, menu_id) VALUES (2, 123);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 12);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 121);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 122);
    INSERT INTO role_menus (role_id, menu_id) VALUES (3, 123);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── role_permissions（动作级权限点）────────────────────────

/// 角色 → 权限点（`{ns}:view` / `{ns}:edit`）。
///
/// 与 `role_menus` 的区别：`role_menus` 只决定**前端菜单渲染**，
/// 这里才是**请求级鉴权**的依据（见 `routers::access`）。
///
/// 种子数据直接由 `access` 的权限矩阵推导：内置角色升级前后的可达范围完全一致，
/// 不会出现「加了权限校验后普通用户被锁死」的情况。新增自定义角色默认无权限，
/// 需管理员在「角色权限」中显式勾选（fail-closed）。
async fn init_role_permissions_table() {
    if !table_exists("role_permissions").await {
        let sql = r#"
    CREATE TABLE role_permissions (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        role_id INTEGER NOT NULL,
        perm_key VARCHAR(64) NOT NULL,
        UNIQUE(role_id, perm_key)
    );
    CREATE INDEX idx_role_permissions_role ON role_permissions(role_id);
    "#;
        let _ = get_db_pool().await.execute(sql).await;
    }

    // 建表与补齐分开：权限矩阵新增模块（如 database）后，老安装的内置角色
    // 也要拿到对应权限点，否则新功能对老用户直接 403。
    sync_builtin_role_permissions().await;
}

/// 把内置角色的权限点补齐到「权限矩阵推导出的默认值」。
///
/// 内置角色（admin/reseller/user/demo）属于系统定义，权限随代码升级而扩展；
/// 只做 INSERT OR IGNORE —— 只补缺失的权限点，不会删除管理员额外授予的权限点。
/// 自定义角色不受影响（保持 fail-closed，需管理员在「角色权限」中显式勾选）。
async fn sync_builtin_role_permissions() {
    let pool = get_db_pool().await;
    let roles: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, role_key FROM roles WHERE role_key IN ('admin','reseller','user','demo')",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for (role_id, role_key) in roles {
        for perm in crate::routers::access::default_permissions_for(&role_key) {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO role_permissions (role_id, perm_key) VALUES (?, ?)",
            )
            .bind(role_id)
            .bind(perm)
            .execute(pool)
            .await;
        }
    }
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

// ── cron_jobs（脚本/自动化：计划任务）────────────────────────

async fn init_cron_jobs_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS cron_jobs (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL DEFAULT '',
        script_path TEXT NOT NULL DEFAULT '',
        schedule TEXT NOT NULL DEFAULT '',
        remark TEXT NOT NULL DEFAULT '',
        enabled INTEGER NOT NULL DEFAULT 1,
        last_run_at INTEGER NOT NULL DEFAULT 0,
        last_run_id TEXT NOT NULL DEFAULT '',
        next_run_at INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL DEFAULT 0
    );
    "#;
    let _ = get_db_pool().await.execute(sql).await;
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
        php_instance TEXT NOT NULL DEFAULT '',
        vhost_state TEXT NOT NULL DEFAULT 'pending',
        vhost_error TEXT NOT NULL DEFAULT '',
        vhost_synced_at INTEGER NOT NULL DEFAULT 0,
        run_state TEXT NOT NULL DEFAULT 'running',
        web_root TEXT NOT NULL DEFAULT '',
        log_root TEXT NOT NULL DEFAULT '',
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

// ── site_profile（站点扩展档案，1:1 site.id）────────────────────

async fn init_site_profile_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS site_profile (
        site_id INTEGER NOT NULL PRIMARY KEY,
        site_type TEXT NOT NULL DEFAULT 'php',
        pseudo_static TEXT NOT NULL DEFAULT 'none',
        pseudo_custom TEXT NOT NULL DEFAULT '',
        web_root_custom INTEGER NOT NULL DEFAULT 0,
        upstreams TEXT NOT NULL DEFAULT '[]',
        locations TEXT NOT NULL DEFAULT '[]',
        ssl_cert_id INTEGER NOT NULL DEFAULT 0,
        force_https INTEGER NOT NULL DEFAULT 0,
        -- TLS 高级设置（空串/缺省 = 面板默认：协议回退 TLSv1.2+TLSv1.3，密码套件不输出）
        ssl_http2 INTEGER NOT NULL DEFAULT 1,
        ssl_prefer_server_ciphers INTEGER NOT NULL DEFAULT 1,
        ssl_protocols TEXT NOT NULL DEFAULT '',
        ssl_ciphers TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL DEFAULT 0
    );
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

// ── ssl_cert（SSL/TLS 证书管理）────────────────────────────

async fn init_ssl_cert_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS ssl_cert (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        -- user_id：证书归属用户（0 = 历史系统证书，仅管理员可见，可在编辑中转为归属某用户）
        user_id INTEGER NOT NULL DEFAULT 0,
        name TEXT NOT NULL DEFAULT '',
        domains TEXT NOT NULL DEFAULT '',
        cert_type TEXT NOT NULL DEFAULT 'upload',
        cert_content TEXT NOT NULL DEFAULT '',
        key_content TEXT NOT NULL DEFAULT '',
        ca_bundle TEXT NOT NULL DEFAULT '',
        csr TEXT NOT NULL DEFAULT '',
        not_before INTEGER NOT NULL DEFAULT 0,
        not_after INTEGER NOT NULL DEFAULT 0,
        status INTEGER NOT NULL DEFAULT 1,
        remark TEXT NOT NULL DEFAULT '',
        created_at INTEGER,
        updated_at INTEGER
    );
    CREATE INDEX IF NOT EXISTS idx_ssl_cert_user ON ssl_cert(user_id);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── fpm_spec（PHP-FPM 规格模板库，仅 admin 维护）────────────────

async fn init_fpm_spec_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS fpm_spec (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL DEFAULT '',
        spec TEXT NOT NULL DEFAULT '',
        remark TEXT NOT NULL DEFAULT '',
        created_at INTEGER,
        updated_at INTEGER
    );
    CREATE UNIQUE INDEX IF NOT EXISTS idx_fpm_spec_name ON fpm_spec(name);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── notice_message（站内信 / 通知中心）────────────────────────

async fn init_notice_message_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS notice_message (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL DEFAULT 0,
        type TEXT NOT NULL DEFAULT '',
        title TEXT NOT NULL DEFAULT '',
        body TEXT NOT NULL DEFAULT '',
        is_read INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS idx_notice_user ON notice_message(user_id, id);
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── server_env（全局运行环境状态表）───────────────────────────

async fn init_server_env_table() {
    let sql = r#"
    CREATE TABLE IF NOT EXISTS server_env (
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        scope TEXT NOT NULL DEFAULT 'auto',
        k TEXT NOT NULL DEFAULT '',
        v TEXT NOT NULL DEFAULT '',
        remark TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL DEFAULT 0,
        UNIQUE(scope, k)
    );
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

// ── helper ─────────────────────────────────────────────────

/// 系统更新配置（单行表）：自动更新开关 / cron / 渠道 / 最近检查信息。
async fn init_update_config_table() {
    if table_exists("update_config").await {
        return;
    }
    let sql = r#"
    CREATE TABLE update_config (
        id INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
        auto INTEGER NOT NULL DEFAULT 0,
        cron TEXT NOT NULL DEFAULT '0 3 * * *',
        channel TEXT NOT NULL DEFAULT 'https://mirrors.zap.cn/zap/releases',
        last_check_at INTEGER NOT NULL DEFAULT 0,
        last_check_version TEXT NOT NULL DEFAULT '',
        last_check_has_update INTEGER NOT NULL DEFAULT 0,
        last_error TEXT NOT NULL DEFAULT '',
        updated_at INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO update_config (id, auto, cron, channel, updated_at)
    VALUES (1, 0, '0 3 * * *', 'https://mirrors.zap.cn/zap/releases', strftime('%s','now'));
    "#;
    let _ = get_db_pool().await.execute(sql).await;
}

async fn table_exists(table_name: &str) -> bool {
    let pool = get_db_pool().await;
    let result: Result<(String,), sqlx::Error> =
        sqlx::query_as("select name from sqlite_master where name = ?")
            .bind(table_name)
            .fetch_one(pool)
            .await;
    result.is_ok()
}
