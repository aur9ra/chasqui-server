mod common;

use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::model::FeatureType;
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use chasqui_server::watcher::watcher::{SyncCommand, run_watcher_worker};
use common::setup_service_with_options;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

pub struct TestOptions {
    pub pages_dir: PathBuf,
    pub images_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub videos_dir: PathBuf,
}

impl Default for TestOptions {
    fn default() -> Self {
        let content_dir = PathBuf::from("/content");
        Self {
            pages_dir: content_dir.clone(),
            images_dir: content_dir.clone(),
            audio_dir: content_dir.clone(),
            videos_dir: content_dir,
        }
    }
}

async fn setup_watcher_service() -> (
    Arc<SyncService>,
    MockContentReader,
    MockBuildNotifier,
    Arc<ChasquiConfig>,
    chasqui_db::SqliteRepository,
) {
    setup_service_with_options(common::TestOptions::default()).await
}

#[tokio::test]
async fn test_watcher_worker_batching() {
    let (service, reader, notifier, config, _repo) = setup_watcher_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    for i in 0..50 {
        let path = format!("/content/file_{}.md", i);
        reader.add_file(&path, "# Content");
        tx.send(SyncCommand::SingleFile(PathBuf::from(path), config.pages_dir.clone(), FeatureType::Page))
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(2500)).await;

    assert_eq!(service.get_all_features_by_type(FeatureType::Page).await.len(), 50);
    assert_eq!(*notifier.call_count.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_watcher_worker_full_sync_trigger() {
    let (service, reader, _notifier, config, _repo) = setup_watcher_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    full_sync_flag.store(true, Ordering::SeqCst);
    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    reader.add_file("/content/trigger.md", "# Trigger");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/trigger.md"),
        config.pages_dir.clone(),
        FeatureType::Page,
    ))
    .await
    .unwrap();

    reader.add_file("/content/background.md", "# Existed already");

    sleep(Duration::from_millis(2500)).await;

    assert_eq!(service.get_all_features_by_type(FeatureType::Page).await.len(), 2);
}

#[tokio::test]
async fn test_watcher_worker_redundant_commands() {
    let (service, reader, notifier, config, _repo) = setup_watcher_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    let path = PathBuf::from("/content/redundant.md");
    reader.add_file("/content/redundant.md", "# Content");

    for _ in 0..20 {
        tx.send(SyncCommand::SingleFile(path.clone(), config.pages_dir.clone(), FeatureType::Page))
            .await
            .unwrap();
    }

    sleep(Duration::from_millis(2500)).await;

    assert_eq!(service.get_all_features_by_type(FeatureType::Page).await.len(), 1);
    assert_eq!(*notifier.call_count.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_watcher_worker_add_delete_recreate_cancellation() {
    let (service, reader, _notifier, config, _repo) = setup_watcher_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    let path = PathBuf::from("/content/flicker.md");

    reader.add_file("/content/flicker.md", "# Version 1");
    tx.send(SyncCommand::SingleFile(path.clone(), config.pages_dir.clone(), FeatureType::Page))
        .await
        .unwrap();

    tx.send(SyncCommand::DeleteFile(path.clone()))
        .await
        .unwrap();

    reader.add_file("/content/flicker.md", "# Version 2");
    tx.send(SyncCommand::SingleFile(path.clone(), config.pages_dir.clone(), FeatureType::Page))
        .await
        .unwrap();

    sleep(Duration::from_millis(2500)).await;

    let feature = service.get_feature_by_identifier("flicker").await.unwrap();
    let page = if let chasqui_core::features::model::Feature::Page(p) = feature { p } else { panic!("Expected page") };
    assert_eq!(page.md_content.trim(), "# Version 2");
}