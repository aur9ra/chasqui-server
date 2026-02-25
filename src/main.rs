use crate::config::ChasquiConfig;
use crate::database::sqlite::SqliteRepository;
use crate::features::watcher::start_directory_watcher;
use crate::services::sync::SyncService;
use axum::Router;
use dotenv;
use sqlx::Sqlite;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use tower_http::services::ServeDir;
use walkdir;

pub mod config;
pub mod database;
pub mod domain;
mod features;
pub mod services;
pub mod parser;

#[derive(Clone)]
pub struct AppState {
    pub sync_service: Arc<SyncService<SqliteRepository>>,
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

    // connect to db
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

    // initialize database access via desired database implementation
    let repository = SqliteRepository::new(pool.clone());

    // sync_service holds an in-memory hashmap of our database.
    // reading from this (rather, asking it for stuff) is much quicker than reading from sqlx.
    let sync_service = SyncService::new(repository)
        .await
        .expect("Failed to initialize SyncService");
    let shared_sync_service = Arc::new(sync_service);

    let app_state = AppState {
        sync_service: shared_sync_service.clone(),
        config: shared_config.clone(),
    };

    // run migrations
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run database migrations.");

    // Initial sync of content directory
    println!("Starting initial content sync...");
    let content_dir = &shared_config.content_dir;
    for entry in walkdir::WalkDir::new(content_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file()
            && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
        {
            let path = entry.into_path();
            if let Err(e) = shared_sync_service.handle_file_changed(&path).await {
                eprintln!("Error during initial sync of {}: {}", path.display(), e);
            }
        }
    }
    println!("Initial content sync complete.");

    // start background file watcher
    start_directory_watcher(shared_sync_service.clone(), shared_config.clone());

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
