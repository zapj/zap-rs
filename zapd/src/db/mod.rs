pub mod models;

use anyhow::{Context, Ok, Result};
use sqlx::{pool::Pool, sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Executor, Sqlite};
use tracing::error;

pub async fn prepare_database() -> Result<Pool<Sqlite>> {
    let filename = "data/zap.db";
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);
    
    let pool = SqlitePoolOptions::new().max_connections(50).connect_with(options).await
    .context("could not connect to database_url")?;

    Ok(pool)
}



pub async fn init_db() {
    let rst = prepare_database().await;
    if rst.is_err() {
        error!("init db error");
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
        created_at INTEGER,
        updated_at INTEGER
    
    );
    INSERT INTO "user" (username,password,email,nickname,last_login_time,last_login_ip,status,created_at,updated_at)
VALUES ("admin","admin","admin@demo.zap.cn","admin",1756650543,"127.0.0.1",1,1756650543,1756650543);
    "#;

    let conn = rst.unwrap();
    let _ = conn.execute(sql_script).await;

}

