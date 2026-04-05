use crate::features::model::FeatureType;
use crate::services::sync::SyncService;
use crate::tests::mocks::{create_test_repository, MockBuildNotifier, MockContentReader};
use crate::tests::integration_sync_core::mock_config;
use crate::watcher::watcher::{SyncCommand, run_watcher_worker};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[tokio::test]
async fn test_watcher_resilience_to_hanging_webhook() {
    let repo = create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();
    let service_arc = Arc::new(service);

    // 1. Setup Hanging Webhook (10s delay)
    notifier.set_latency(Duration::from_secs(10));

    let (tx, rx) = mpsc::channel(100);
    let full_sync_flag = Arc::new(AtomicBool::new(false));
    tokio::spawn(run_watcher_worker(service_arc.clone(), rx, full_sync_flag));

    // 2. Trigger first change -> Wait for it to be ingested
    reader.add_file("/content/md/first.md", "# First");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/md/first.md"),
        config.pages_dir.clone(),
        FeatureType::Page
    )).await.unwrap();

    // Wait for first file to appear - this confirms the watcher loop has finished process_batch
    // and is now potentially hanging on notify_build()
    loop {
        if service_arc.get_feature_by_identifier("first").await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // 3. Trigger second change while the first webhook is still hanging
    reader.add_file("/content/md/second.md", "# Second");
    tx.send(SyncCommand::SingleFile(
        PathBuf::from("/content/md/second.md"),
        config.pages_dir.clone(),
        FeatureType::Page
    )).await.unwrap();

    // 4. ASSERT: We should see 'second.md' in the registry within 2 seconds.
    // In the CURRENT implementation, this WILL fail (timeout) because the watcher 
    // is stuck on the first notify_build().await call.
    let check_second = async {
        loop {
            if service_arc.get_feature_by_identifier("second").await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let result = timeout(Duration::from_secs(2), check_second).await;
    
    assert!(result.is_ok(), "SYSTEM EXPLODED: The hanging webhook blocked the watcher from processing the second event!");
}

#[tokio::test]
async fn test_sync_resilience_to_offline_webhook() {
    let repo = create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    // 1. Setup Failing Webhook (500 Error)
    notifier.set_fail(true);

    // 2. Perform batch update
    reader.add_file("/content/md/fail-test.md", "# Test");
    service.process_batch(
        vec![(PathBuf::from("/content/md/fail-test.md"), config.pages_dir.clone(), FeatureType::Page)],
        vec![]
    ).await.unwrap();

    // 3. Manually call notify_build (simulating what watcher does)
    let notify_result = service.notify_build().await;
    
    // 4. ASSERT: The notification failed, but the data IS still in our system.
    assert!(notify_result.is_err());
    assert!(service.get_feature_by_identifier("fail-test").await.is_some());
}