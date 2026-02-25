use crate::watcher::watcher::{run_watcher_worker, SyncCommand};
use crate::services::sync::SyncService;
use crate::tests::integration_pages_sync_service::{MockRepository, MockContentReader, MockBuildNotifier};
use crate::config::ChasquiConfig;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

// helper to setup a fresh service and mock world for watcher testing
async fn setup_service() -> (Arc<SyncService>, MockContentReader, MockBuildNotifier, Arc<ChasquiConfig>) {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: "".into(),
        content_dir,
        strip_extensions: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
    });

    let service = SyncService::new(
        Box::new(repo), 
        Box::new(reader.clone()), 
        Box::new(notifier.clone()), 
        config.clone()
    ).await.unwrap();
    
    (Arc::new(service), reader, notifier, config)
}

// test that the watcher correctly batches multiple rapid events into a single sync
// if a writer saves 50 times in a second, we don't want to sync 50 times!
#[tokio::test]
async fn test_watcher_worker_batching() {
    let (service, reader, notifier, _config) = setup_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    // start the background worker logic directly
    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    // simulate a "burst" of 50 file creations
    for i in 0..50 {
        let path = format!("/content/file_{}.md", i);
        reader.add_file(&path, "# Content");
        tx.send(SyncCommand::SingleFile(PathBuf::from(path))).await.unwrap();
    }

    // wait for the 1.5s debounce window to close
    sleep(Duration::from_millis(2500)).await;

    // assert that all 50 files were processed
    assert_eq!(service.get_all_pages().await.len(), 50);
    // CRITICAL: despite 50 events, we should have only triggered ONE build notification
    assert_eq!(*notifier.call_count.lock().unwrap(), 1);
}

// test the "Nuclear Safety Valve"
// if the event channel is flooded, the system should pivot to a Full Sync
#[tokio::test]
async fn test_watcher_worker_full_sync_trigger() {
    let (service, reader, _notifier, _config) = setup_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    // manually trip the "emergency" flag
    full_sync_flag.store(true, Ordering::SeqCst);
    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    // send just one event
    reader.add_file("/content/trigger.md", "# Trigger");
    tx.send(SyncCommand::SingleFile(PathBuf::from("/content/trigger.md"))).await.unwrap();

    // hide a file in the "file system" that we never sent an event for
    reader.add_file("/content/background.md", "# Existed already");
    
    sleep(Duration::from_millis(2500)).await;
    
    // because the flag was set, the system should have scanned EVERYTHING, finding both files
    assert_eq!(service.get_all_pages().await.len(), 2);
}

// test the system's ability to handle identical redundant commands
// some editors save files in weird ways that fire multiple "Modify" events for one save
#[tokio::test]
async fn test_watcher_worker_redundant_commands() {
    let (service, reader, notifier, _config) = setup_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    let path = PathBuf::from("/content/redundant.md");
    reader.add_file("/content/redundant.md", "# Content");
    
    // send the exact same command 20 times in a row
    for _ in 0..20 {
        tx.send(SyncCommand::SingleFile(path.clone())).await.unwrap();
    }

    sleep(Duration::from_millis(2500)).await;

    // the worker uses a HashSet internally, so these 20 should collapse into 1 operation
    assert_eq!(service.get_all_pages().await.len(), 1);
    assert_eq!(*notifier.call_count.lock().unwrap(), 1);
}

// test the "Flicker" scenario: Add -> Delete -> Add
// this ensures the final state of the batch reflects the final state of the disk
#[tokio::test]
async fn test_watcher_worker_add_delete_recreate_cancellation() {
    let (service, reader, notifier, _config) = setup_service().await;
    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));

    tokio::spawn(run_watcher_worker(service.clone(), rx, full_sync_flag));

    let path = PathBuf::from("/content/flicker.md");

    // add, delete, and add again in rapid succession
    reader.add_file("/content/flicker.md", "# Version 1");
    tx.send(SyncCommand::SingleFile(path.clone())).await.unwrap();

    tx.send(SyncCommand::DeleteFile(path.clone())).await.unwrap();

    reader.add_file("/content/flicker.md", "# Version 2");
    tx.send(SyncCommand::SingleFile(path.clone())).await.unwrap();

    sleep(Duration::from_millis(2500)).await;

    // the final result should be Version 2
    let page = service.get_page_by_identifier("flicker").await.unwrap();
    assert_eq!(page.html_content.trim(), "<h1>Version 2</h1>");
}
