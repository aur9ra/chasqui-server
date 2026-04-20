pub mod repo;
pub mod sqlite;
pub mod testutil;

pub use sqlite::SqliteRepository;

use anyhow::Result;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn create_pool(database_url: &str, max_connections: u32) -> Result<sqlx::SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await
        .map_err(Into::into)
}

pub async fn run_migrations(pool: &sqlx::SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}