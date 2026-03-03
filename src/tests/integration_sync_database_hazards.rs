use crate::config::ChasquiConfig;
use crate::database::sqlite::SqliteRepository;
use crate::features::model::{Feature, FeatureType};
use crate::io::{ContentMetadata, ContentReader, SyncFile};
use crate::services::sync::SyncService;
use crate::tests::integration_sync_core::mock_config;
use crate::tests::mocks::{MockBuildNotifier, MockContentReader};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Executor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::{Duration, timeout};

fn get_test_dir(name: &str) -> PathBuf {
    let mut path = std::env::current_dir().unwrap();
    path.push("testing_ground");
    path.push(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).expect("Failed to clean testing_ground");
    }
    std::fs::create_dir_all(&path).expect("Failed to create testing_ground");
    path
}

#[tokio::test]
async fn test_sync_handles_database_locked_gracefully() {
    // 1. Create a temporary sqlite database file
    let db_file = NamedTempFile::new().expect("Failed to create temp db file");
    let db_path = db_file.path().to_str().unwrap();
    let _db_url = format!("sqlite:{}", db_path);

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
    )
    .await
    .unwrap();

    // 3. Open a SECOND connection and EXCLUSIVELY lock the database
    let mut lock_conn = SqliteConnectOptions::new()
        .filename(db_path)
        .connect()
        .await
        .unwrap();

    // Start an immediate transaction to lock the whole DB file
    lock_conn.execute("BEGIN IMMEDIATE").await.unwrap();

    // 4. Attempt to sync a new page
    reader.add_file(
        content_dir.path().join("md/locked.md").to_str().unwrap(),
        "# Locked",
    );

    let sync_attempt = service
        .process_batch(
            vec![(
                content_dir.path().join("md/locked.md"),
                config.pages_dir.clone(),
                FeatureType::Page,
            )],
            vec![],
        )
        .await;

    // 5. ASSERT: The sync should return an error (Database is locked) instead of panicking
    assert!(
        sync_attempt.is_err(),
        "Sync should have failed due to database lock"
    );
    let err_msg = format!("{:#}", sync_attempt.unwrap_err());
    println!("Actual Error Message (Full Chain): {}", err_msg);
    assert!(err_msg.contains("database is locked") || err_msg.contains("pool timed out"));

    // 6. RELEASE the lock
    lock_conn.execute("ROLLBACK").await.unwrap();

    // 7. Verify the system is still alive and can sync now
    let retry_sync = service
        .process_batch(
            vec![(
                content_dir.path().join("md/locked.md"),
                config.pages_dir.clone(),
                FeatureType::Page,
            )],
            vec![],
        )
        .await;

    assert!(
        retry_sync.is_ok(),
        "Sync should succeed after lock is released"
    );

    let page = service.get_feature_by_identifier("locked").await;
    assert!(page.is_some());
}

struct ProxyReader {
    inner: MockContentReader,
    trigger_file: String,
    tx: tokio::sync::mpsc::Sender<()>,
}

#[async_trait]
impl ContentReader for ProxyReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        if path.to_string_lossy().contains(&self.trigger_file) {
            let _ = self.tx.send(()).await;
            // Wait slightly to ensure lock task wins
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        self.inner.read_to_string(path).await
    }
    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        self.inner.read_bytes(path).await
    }
    async fn open_file(&self, path: &Path) -> Result<SyncFile> {
        self.inner.open_file(path).await
    }
    async fn get_hash(&self, path: &Path) -> Result<String> {
        self.inner.get_hash(path).await
    }
    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        self.inner.get_metadata(path).await
    }
    async fn list_all_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        self.inner.list_all_files(root).await
    }
    async fn list_files_by_extension(&self, root: &Path, extension: String) {
        self.inner.list_files_by_extension(root, extension).await
    }
    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        self.inner.list_markdown_files(root).await
    }
}

#[tokio::test]
async fn test_sync_handles_db_lock_during_update_gracefully() {
    // 1. create a temporary sqlite database file in testing_ground
    let test_dir = get_test_dir("sync_db_lock_update");
    let db_path = test_dir.join("test.db");

    // 2. initialize real sqlite repo with a very short busy_timeout
    let options = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_millis(50));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to connect to temp db");

    // 3. run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let repo = SqliteRepository::new(pool.clone());
    let inner_reader = MockContentReader::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    let reader = Arc::new(ProxyReader {
        inner: inner_reader.clone(),
        trigger_file: "h.md".to_string(),
        tx,
    });

    let notifier = MockBuildNotifier::new();
    let config = mock_config(test_dir.clone());

    // make a bunch of files
    let mut files = Vec::new();
    // from a-z
    for c in b'a'..=b'z' {
        let name = format!("{}.md", c as char);
        let path = test_dir.join("md").join(&name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("# Page {}", c as char)).unwrap();
        inner_reader.add_file(path.to_str().unwrap(), &format!("# Page {}", c as char));
        files.push((path, config.pages_dir.clone(), FeatureType::Page));
    }

    let db_path_clone = db_path.clone();
    tokio::spawn(async move {
        if let Some(_) = rx.recv().await {
            let mut lock_conn = SqliteConnectOptions::new()
                .filename(&db_path_clone)
                .connect()
                .await
                .unwrap();
            lock_conn.execute("BEGIN IMMEDIATE").await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
            lock_conn.execute("ROLLBACK").await.unwrap();
        }
    });

    let service_result = SyncService::new(
        Box::new(repo),
        reader.clone(),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await;

    // Initial sync will likely fail due to the lock being triggered
    assert!(
        service_result.is_err(),
        "Initial sync should have failed due to injected lock"
    );

    // Wait for lock to release
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // Now init for real
    let repo_retry = SqliteRepository::new(pool.clone());
    let service = SyncService::new(
        Box::new(repo_retry),
        reader.clone(),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .expect("Second init should succeed");

    // verify they're all there
    let all_pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(all_pages.len(), 16);
}
