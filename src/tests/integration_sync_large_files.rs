use crate::features::model::Feature;
use crate::services::sync::SyncService;
use crate::tests::integration_sync_core::mock_config;
use crate::tests::mocks::{create_test_repository, MockBuildNotifier, MockContentReader};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{Duration, timeout};

#[tokio::test]
#[ignore]
async fn test_sync_creates_large_features() {
    let repo = create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    // 1. Create "Virtual" 999GB assets
    let large_size = 999u64 * 1024 * 1024 * 1024; // 999GB
    reader.add_virtual_large_file("/content/videos/LARGE_VIDEO.mp4", large_size);
    reader.add_virtual_large_file("/content/audio/LARGE_AUDIO.mp3", large_size);
    reader.add_virtual_large_file("/content/images/LARGE_IMAGE.png", large_size);

    // 2. Initialize sync service with strict 250ms timeout
    // If we hash more than 1MB, this WILL fail.
    let service = timeout(Duration::from_millis(250), async {
        SyncService::new(
            repo.clone(),
            Arc::new(reader.clone()),
            Box::new(notifier.clone()),
            config.clone(),
        )
        .await
        .unwrap()
    })
    .await
    .expect("Sync service OOM or Timeout! It tried to process too much of the 999GB file.");

    // 3. Confirm files are in the manifest
    let manifest = service.manifest.read().await;
    assert!(manifest.filenames.contains("LARGE_VIDEO.mp4"));
    assert!(manifest.filenames.contains("LARGE_AUDIO.mp3"));
    assert!(manifest.filenames.contains("LARGE_IMAGE.png"));

    println!("Large File Test: Successfully synced 3TB of virtual assets in < 250ms.");
}

#[tokio::test]
async fn test_sync_renders_large_page() {
    let repo = create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    // create a very large file
    let body =
        "## Section\n\nThis is a repeating line of text to bulk up the page.\n\n".repeat(1000000);
    let content = format!(
        "---\nidentifier: large-page\nname: Large Page\n---\n{}",
        body
    );

    reader.add_file("/content/md/large.md", &content);

    // 2. Sync and Render
    // We expect this to take more than a few milliseconds but well under a second.
    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    // 3. Verify the rendered result
    let feature = service
        .get_feature_by_identifier("large-page")
        .await
        .expect("Large page missing");
    if let Feature::Page(p) = feature {
        assert_eq!(p.identifier, "large-page");
        assert!(p.html_content.len() > 10 * 1024 * 1024); // Should be > 10MB of HTML
        assert!(p.html_content.contains("This is a repeating line"));
    }

    println!("Large Page Test: Rendered 10MB of Markdown successfully.");
}