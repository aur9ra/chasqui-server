use crate::config::ChasquiConfig;
use crate::features::model::{Feature, FeatureType};
use crate::services::sync::SyncService;
use crate::tests::mocks::{MockBuildNotifier, MockRepository};
use crate::io::local::LocalContentReader;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::time::{timeout, Duration};

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[tokio::test]
async fn test_sync_handles_circular_and_dead_symlinks() {
    // 1. Initialize real temp directory for OS-level hazard testing
    let dir = tempdir().expect("Failed to create temp dir");
    let content_dir = dir.path().join("content");
    let md_dir = content_dir.join("md");
    fs::create_dir_all(&md_dir).unwrap();

    // 2. Create broken symbolic links (pointing to non-existent places)
    #[cfg(unix)]
    {
        symlink(content_dir.join("void"), md_dir.join("dead_link")).unwrap();
    }

    // 3. Create recursive symlinks
    #[cfg(unix)]
    {
        symlink(&md_dir, md_dir.join("circular_link")).unwrap();
    }

    // 4. Create a normal index.md
    let index_path = md_dir.join("index.md");
    fs::write(index_path, "# Index").unwrap();

    // 5. Initialize sync service
    let repo = MockRepository::new();
    let notifier = MockBuildNotifier::new();
    
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        pages_dir: md_dir.clone(),
        images_dir: content_dir.join("images"),
        audio_dir: content_dir.join("audio"),
        videos_dir: content_dir.join("videos"),
        page_strip_extension: true,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
        port: 3000,
    });

    let reader = Arc::new(LocalContentReader {
        root_path: PathBuf::from("/"), // Rootless reader for absolute path support
    });

    // Enforce 250ms limit for the entire initialization + initial sync
    let service = timeout(Duration::from_millis(250), async {
        SyncService::new(
            Box::new(repo.clone()),
            reader.clone(),
            Box::new(notifier.clone()),
            config.clone(),
        ).await.unwrap()
    }).await.expect("Initialization + Full Sync timed out! Possible infinite loop in symlink handling.");

    // 6. Assert that we can get index.md
    let index = service.get_feature_by_identifier("index").await;
    assert!(index.is_some(), "Index.md should have been discovered despite hazards");

    // 7. Add a second page to the file system
    let second_path = md_dir.join("second.md");
    fs::write(second_path.clone(), "# Second").unwrap();

    // 8. Make a batch, tell sync to process it
    // Enforce 250ms limit for the partial sync
    timeout(Duration::from_millis(250), async {
        service.process_batch(
            vec![(second_path, md_dir.clone(), FeatureType::Page)],
            vec![]
        ).await.unwrap()
    }).await.expect("Partial sync timed out! Possible performance degradation due to hazards.");

    // 9. Attempt to get the page, assert that we can
    let second = service.get_feature_by_identifier("second").await;
    assert!(second.is_some(), "Second page should have been discovered after partial sync");
    
    if let Some(Feature::Page(p)) = second {
        assert_eq!(p.identifier, "second");
    }
}
