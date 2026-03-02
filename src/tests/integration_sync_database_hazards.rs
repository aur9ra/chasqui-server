use crate::config::ChasquiConfig;
use crate::database::sqlite::SqliteRepository;
use crate::features::model::{Feature, FeatureType};
use crate::services::sync::SyncService;
use crate::tests::mocks::{MockBuildNotifier, MockContentReader};
use crate::tests::integration_sync_core::mock_config;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Executor};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_sync_handles_database_locked_gracefully() {
    // 1. Create a temporary sqlite database file
    let db_file = NamedTempFile::new().expect("Failed to create temp db file");
    let db_path = db_file.path().to_str().unwrap();
    let db_url = format!("sqlite:{}", db_path);

    // 2. Initialize the real SqliteRepository with this file
    // Set a very short busy_timeout so the test fails fast
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_millis(100)); // 100ms timeout

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to connect to temp db");

    // Run migrations to setup schema
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let repo = SqliteRepository::new(pool.clone());
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = tempfile::tempdir().unwrap();
    let config = mock_config(content_dir.path().to_path_buf());

    let service = SyncService::new(
        Box::new(repo),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    // 3. Open a SECOND connection and EXCLUSIVELY lock the database
    let mut lock_conn = SqliteConnectOptions::new()
        .filename(db_path)
        .connect()
        .await
        .unwrap();
    
    // Start an immediate transaction to lock the whole DB file
    lock_conn.execute("BEGIN IMMEDIATE").await.unwrap();

    // 4. Attempt to sync a new page
    reader.add_file(content_dir.path().join("md/locked.md").to_str().unwrap(), "# Locked");
    
    let sync_attempt = service.process_batch(
        vec![(content_dir.path().join("md/locked.md"), config.pages_dir.clone(), FeatureType::Page)],
        vec![]
    ).await;

    // 5. ASSERT: The sync should return an error (Database is locked) instead of panicking
    assert!(sync_attempt.is_err(), "Sync should have failed due to database lock");
    let err_msg = format!("{:#}", sync_attempt.unwrap_err());
    println!("Actual Error Message (Full Chain): {}", err_msg);
    assert!(err_msg.contains("database is locked") || err_msg.contains("pool timed out"));

    // 6. RELEASE the lock
    lock_conn.execute("ROLLBACK").await.unwrap();

    // 7. Verify the system is still alive and can sync now
    let retry_sync = service.process_batch(
        vec![(content_dir.path().join("md/locked.md"), config.pages_dir.clone(), FeatureType::Page)],
        vec![]
    ).await;

    assert!(retry_sync.is_ok(), "Sync should succeed after lock is released");
    
    let page = service.get_feature_by_identifier("locked").await;
    assert!(page.is_some());
}
