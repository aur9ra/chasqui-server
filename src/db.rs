use sqlx::sqlite::{Sqlite, SqlitePoolOptions};

enum Env {
    Dev,
    Production,
}

const DB_ENV: Env = Env::Dev;

async fn init_db_connection() -> Result<(), sqlx::Error> {
    let pool_options = SqlitePoolOptions::new();
    let db_url = match DB_ENV {
        Env::Dev => "db/dev.db",
        Env::Production => "db/prod.db",
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(db_url)
        .await?;

    Ok(())
}
