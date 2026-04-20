mod common;

use chasqui_core::features::model::FeatureType;
use chasqui_server::watcher::watcher::{SyncCommand, run_watcher_worker};
use common::setup_service_with_options;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

#[tokio::test]
async fn test_watcher_mounts_overlap() {
    let opts = common::TestOptions {
        pages_dir: PathBuf::from("/content/md"),
        images_dir: PathBuf::from("/content/images"),
        audio_dir: PathBuf::from("/content/audio"),
        videos_dir: PathBuf::from("/content/videos"),
    };

    let (service, reader, _notifier, config, _repo) = setup_service_with_options(opts).await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    reader.add_file("/content/images/photo.jpg", "fake-image");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/images/photo.jpg"),
        config.images_dir.clone(),
        FeatureType::Image,
    ))
    .await
    .unwrap();

    sleep(Duration::from_millis(2500)).await;

    let images = service.get_all_features_by_type(FeatureType::Image).await;
    assert!(images.len() >= 1);
}