use anyhow::{Context, Ok, Result};
use sqlx::{pool::Pool, sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Sqlite};


pub async fn prepare_database() -> Result<Pool<Sqlite>> {
    let filename = "zap.db";
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);
    
    let pool = SqlitePoolOptions::new().max_connections(50).connect_with(options).await
    .context("could not connect to database_url")?;

    Ok(pool)
}

