mod common;

use chasqui_core::features::model::FeatureType;
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use chasqui_server::watcher::watcher::{SyncCommand, run_watcher_worker};
use common::mock_config;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::test]
async fn test_build_webhook_resilience() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let mut notifier = MockBuildNotifier::new();
    notifier.set_fail(true);

    let config = mock_config(PathBuf::from("/content"));

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    reader.add_file("/content/md/test.md", "# Test");

    // Verify that even when the webhook fails, the sync still succeeds
    service.full_sync().await.unwrap();

    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(pages.len(), 1);
}

#[tokio::test]
async fn test_webhook_called_after_watcher_batch() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let service = Arc::new(SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap());

    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    reader.add_file("/content/md/watcher_test.md", "# Watcher Test");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/md/watcher_test.md"),
        config.pages_dir.clone(),
        FeatureType::Page,
    ))
    .await
    .unwrap();

    tokio::time::sleep(Duration::from_millis(2500)).await;

    // Verify the page was synced
    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert!(pages.len() >= 1);

    // Verify that the webhook notification was sent
    assert!(*notifier.call_count.lock().unwrap() >= 1);
}