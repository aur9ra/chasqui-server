use chasqui_core::config::ChasquiConfig;
use chasqui_core::io::local::LocalContentReader;
use chasqui_db::{create_pool, run_migrations, SqliteRepository};
use crate::app::AppState;
use crate::services::sync::SyncService;
use crate::services::WebhookBuildNotifier;
use crate::watcher::watcher::start_directory_watcher;
use axum::Router;
use dotenv;
use sqlx::migrate::MigrateDatabase;
use sqlx::Sqlite;
use std::path::PathBuf;
use std::sync::Arc;

pub mod app;
pub mod features;
pub mod services;
pub mod watcher;

#[cfg(test)]
mod testutil;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    let docker_runtime = std::env::var("DOCKER_RUNTIME").unwrap_or_default() == "true";

    if docker_runtime {
        dotenv::from_filename(".env.containers.default").ok();
    }

    dotenv::from_filename(".env.default").ok();

    let config = ChasquiConfig::from_env();
    let shared_config = Arc::new(config.clone());

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
        }
    }

    let pool = create_pool(&config.database_url, config.max_connections)
        .await
        .expect("Failed to create database pool");

    run_migrations(&pool)
        .await
        .expect("Failed to run database migrations.");

    let repository = SqliteRepository::new(pool);

    let reader = Arc::new(LocalContentReader {
        root_path: PathBuf::from("/"),
    });

    let notifier = WebhookBuildNotifier::new(
        config.webhook_url.clone(),
        config.webhook_secret.clone(),
    );

    let sync_service = SyncService::new(
        repository,
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

    start_directory_watcher(shared_sync_service.clone(), shared_config.clone());

    match shared_sync_service.notify_build().await {
        Ok(_) => println!("Initial build notification sent successfully."),
        Err(e) => eprintln!(
            "Initial build notification failed (this is expected if frontend is not running): {}",
            e
        ),
    }

    println!("Starting server...");

    let api_router = Router::new()
        .nest("/pages", features::pages::pages_router())
        .route(
            "/metadata/{*identifier}",
            axum::routing::get(features::handlers::metadata_handler),
        );

    let app = Router::new().nest("/api", api_router).with_state(app_state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Server listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}