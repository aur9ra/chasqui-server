use crate::features::model::FeatureType;
use crate::watcher::watcher::{SyncCommand, run_watcher_worker};
use crate::tests::integration_pages_directory_watcher::{TestOptions, setup_service_with_options};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

// verify that overlapping mounts (pointing to same folder) don't duplicate events
#[tokio::test]
async fn test_watcher_overlapping_mounts_no_redundancy() {
    // In this setup, TestOptions::default() already points all mounts to /content
    let (service, reader, notifier, _config, repo) = setup_service_with_options(TestOptions::default()).await;
    
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));
    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    // Reset counts after initial sync
    {
        let mut n_count = notifier.call_count.lock().unwrap();
        *n_count = 0;
        let mut s_count = repo.save_count.lock().unwrap();
        *s_count = 0;
    }

    // Trigger one file change
    let content_dir = PathBuf::from("/content");
    reader.add_file("/content/overlap.md", "# Overlap");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/overlap.md"),
        content_dir.clone(),
        FeatureType::Page
    )).await.unwrap();

    sleep(Duration::from_millis(2500)).await;

    // Despite multiple mounts watching this folder, we should only have ONE sync event
    assert_eq!(*notifier.call_count.lock().unwrap(), 1);
    // AND exactly one DB write
    assert_eq!(*repo.save_count.lock().unwrap(), 1);
}
