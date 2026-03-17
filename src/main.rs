use crate::config::ChasquiConfig;
use crate::database::sqlite::SqliteRepository;
use crate::services::sync::SyncService;
use crate::watcher::watcher::start_directory_watcher;
use axum::Router;
use dotenv;
use sqlx::Sqlite;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::PathBuf;
use std::sync::Arc;

pub mod config;
pub mod database;
mod features;
pub mod io;
pub mod parser;
pub mod services;
pub mod watcher;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct AppState {
    pub sync_service: Arc<SyncService>,
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

    // initialize content reader
    let reader = Arc::new(io::local::LocalContentReader {
        root_path: PathBuf::from("/"), // Rootless reader, relies on config canonicalization
    });

    // initialize notifier
    let notifier = services::WebhookBuildNotifier::new(
        config.webhook_url.clone(),
        config.webhook_secret.clone(),
    );

    // sync_service holds an in-memory hashmap of our database.
    // reading from this (rather, asking it for stuff) is much quicker than reading from db.
    let sync_service = SyncService::new(
        Box::new(repository),
        reader,
        Box::new(notifier),
        shared_config.clone(),
    )
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

    // diagnostic logs
    println!(
        "Main: Serving static files from: {:?}",
        config.frontend_path
    );

    // start background file watchers
    start_directory_watcher(shared_sync_service.clone(), shared_config.clone());

    // trigger initial build notification on startup
    // if the frontend is still booting up, it's fine
    match shared_sync_service.notify_build().await {
        Ok(_) => println!("Initial build notification sent successfully."),
        Err(e) => eprintln!(
            "Initial build notification failed (this is expected if frontend is not running): {}",
            e
        ),
    }

    println!("Starting server...");

    // start router setup
    let api_router = Router::new()
        .nest("/pages", features::pages::pages_router())
        .route(
            "/metadata/{*identifier}",
            axum::routing::get(features::handlers::metadata_handler),
        );

    let app = Router::new()
        .nest("/api", api_router)
        .fallback(features::handlers::universal_dispatch_handler)
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Server listening on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}
