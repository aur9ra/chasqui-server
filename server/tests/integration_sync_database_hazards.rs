mod common;

use chasqui_core::features::model::FeatureType;
use chasqui_core::io::{ContentMetadata, ContentReader, SyncFile};
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use common::mock_config;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::time::Duration;

struct SlowContentReader {
    inner: MockContentReader,
    delay_ms: u64,
}

impl SlowContentReader {
    fn new(inner: MockContentReader, delay_ms: u64) -> Self {
        Self { inner, delay_ms }
    }
}

#[async_trait]
impl ContentReader for SlowContentReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        self.inner.read_to_string(path).await
    }
    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        self.inner.read_bytes(path).await
    }
    async fn open_file(&self, path: &Path) -> Result<SyncFile> {
        self.inner.open_file(path).await
    }
    async fn get_hash(&self, path: &Path) -> Result<String> {
        tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        self.inner.get_hash(path).await
    }
    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        self.inner.get_metadata(path).await
    }
    async fn list_all_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        self.inner.list_all_files(root).await
    }
    async fn list_files_by_extension(&self, root: &Path, ext: String) {
        self.inner.list_files_by_extension(root, ext).await
    }
    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        self.inner.list_markdown_files(root).await
    }
}

#[tokio::test]
async fn test_concurrent_write_handling() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    reader.add_file("/content/md/slow.md", "---\nidentifier: slow\n---\n# Slow");

    let slow_reader = Arc::new(SlowContentReader::new(reader, 10));

    let service = SyncService::new(
        repo.clone(),
        slow_reader,
        Box::new(notifier),
        config,
    )
    .await
    .unwrap();

    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(pages.len(), 1);
}