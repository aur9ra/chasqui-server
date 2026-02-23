use crate::config::ChasquiConfig;
use crate::features::pages::model::DbPage;
use crate::features::pages::repo::{get_pages_from_db, process_md_dir, process_page_operations};
use crate::features::watcher::start_directory_watcher;
use axum::Router;
use dotenv;
use sqlx::Sqlite;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::ServeDir;

pub mod config;
mod features;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::Pool<Sqlite>,
    pub config: Arc<ChasquiConfig>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // determine environment variables
    dotenv::dotenv().ok();

    // load centralized config
    let config = ChasquiConfig::from_env();
    let shared_config = Arc::new(config.clone());

    // verify db exists
    if !Sqlite::database_exists(&config.database_url)
        .await
        .unwrap_or(false)
    {
        println!(
            "Unable to connect to database at {}, creating...",
            config.database_url
        );
        match Sqlite::create_database(&config.database_url).await {
            Ok(_) => println!("Successfully created database at {}.", &config.database_url),
            Err(e) => panic!(
                "Unable to create database at {}. Error details: {}",
                &config.database_url, e
            ),
        };
    }

    // connect to our db
    let pool = match SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.database_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            panic!("Failed to create pool on {}: {}", config.database_url, e);
        }
    };

    // run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations.");

    let app_state = AppState {
        pool: pool.clone(),
        config: shared_config.clone(),
    };

    // init pages, sync with db
    let md_path = Path::new("./content/md");
    let db_pages = get_pages_from_db(&pool).await.unwrap();
    let borrowable_db_pages: Vec<&DbPage> = db_pages.iter().collect();
    let page_operations = process_md_dir(md_path, borrowable_db_pages, &config).unwrap();
    process_page_operations(&pool, page_operations)
        .await
        .unwrap();

    println!("Sync complete.");

    // start background file watcher
    start_directory_watcher(pool.clone(), shared_config.clone());

    println!("Starting server...");

    // start router setup

    // api router, where features are composed
    let api_router = Router::new().nest("/pages", features::pages::pages_router());

    let app = Router::new()
        .nest("/api", api_router)
        .fallback_service(ServeDir::new(config.frontend_path))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
