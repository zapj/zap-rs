pub mod models;
pub mod init_db;


use tokio::sync::OnceCell;
use sqlx::{pool::Pool, sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Sqlite, SqlitePool};

use crate::zap::ZapError;

pub static DB_POOL: OnceCell<Pool<Sqlite>> = OnceCell::const_new();

pub async fn get_db_pool() -> &'static SqlitePool {
    DB_POOL.get_or_init(|| async {
        let filename = "data/zap.db";
        let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);
        SqlitePoolOptions::new().max_connections(50).connect_with(options).await.unwrap()
    }).await
}

pub async fn open_db() -> Result<Pool<Sqlite>,ZapError> {
    let filename = "data/zap.db";
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);
    
    let pool = SqlitePoolOptions::new().max_connections(50).connect_with(options).await?;

    Ok(pool)
}


