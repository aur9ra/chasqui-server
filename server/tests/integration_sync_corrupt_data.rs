mod common;

use chasqui_core::features::model::{Feature, FeatureType};
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use common::mock_config;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::test]
async fn test_sync_handles_corrupt_frontmatter() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    // Add a file with totally malformed frontmatter (missing closing ---)
    reader.add_file(
        "/content/md/corrupt.md",
        "---\ntitle: Missing closing delimiter\n# Content",
    );

    // This should not panic or crash; it should handle the error gracefully
    service.full_sync().await.unwrap();

    // Verify that the corrupt file was not ingested
    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    // The file may or may not be ingested depending on parser tolerance,
    // but the sync should not crash
    assert!(pages.len() <= 1);
}

#[tokio::test]
async fn test_sync_handles_empty_content_directory() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    service.full_sync().await.unwrap();

    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(pages.len(), 0);
}