use sqlx::Executor;


use super::get_db_pool;


pub async fn init_schema() {
    init_system_user_table_schema().await;
    init_system_monitor_table_schema().await;
    init_system_monitor_networks_table_schema().await;

}
async fn init_system_user_table_schema(){    
    if table_exists("user").await {
        return;
    }

    let sql_script = r#"
    CREATE TABLE user (  
        id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
        username VARCHAR(128) UNIQUE NOT NULL,
        password VARCHAR(256) NOT NULL,
        email VARCHAR(256) UNIQUE NOT NULL,
        nickname TEXT,
        last_login_time INTEGER,
        last_login_ip TEXT,
        status INTEGER DEFAULT 1,
        roles TEXT,
        permissions TEXT,
        created_at INTEGER,
        updated_at INTEGER
    
    );
    INSERT INTO "user" (username,password,email,nickname,last_login_time,last_login_ip,status,created_at,updated_at)
VALUES ("admin","$2y$10$LiiwCTjRHewO1FY/B8Y7yuLYvOBuL/7gFKIZAP/JWDwliWWPTiE4a","admin@demo.zap.cn","admin",1756650543,"127.0.0.1",1,1756650543,1756650543);
    "#;

    let _ = get_db_pool().await.execute(sql_script).await;
}

async fn init_system_monitor_table_schema(){    
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

async fn init_system_monitor_networks_table_schema(){    
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

async fn table_exists(table_name : &str) -> bool {
    let pool = get_db_pool().await;
    let table_is_exist:Result<(String,),sqlx::Error> = sqlx::query_as("select name from sqlite_master where name= ? ").bind(table_name)
    .fetch_one(pool).await;
    if table_is_exist.is_ok() {
        return true;
    }
    false
}