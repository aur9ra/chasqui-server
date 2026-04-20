mod common;

use chasqui_core::features::model::Feature;
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use common::mock_config;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{Duration, timeout};

#[tokio::test]
#[ignore]
async fn test_sync_large_file() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    reader.add_virtual_large_file("/content/videos/large_video.mp4", 1024 * 1024 * 100);

    let service = timeout(
        Duration::from_secs(30),
        SyncService::new(
            repo,
            Arc::new(reader),
            Box::new(notifier),
            config,
        ),
    )
    .await
    .expect("Service creation timed out")
    .expect("Failed to create service");

    let videos = service.get_all_features_by_type(chasqui_core::features::model::FeatureType::Video).await;
    assert_eq!(videos.len(), 1);
}