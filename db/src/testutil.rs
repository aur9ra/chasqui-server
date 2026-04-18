use crate::SqliteRepository;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn create_test_repository() -> SqliteRepository {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    SqliteRepository::new(pool)
}