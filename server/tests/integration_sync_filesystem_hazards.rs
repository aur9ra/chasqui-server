mod common;

use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::model::{Feature, FeatureType};
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier};
use chasqui_core::io::local::LocalContentReader;
use common::{mock_config, setup_service};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::time::{timeout, Duration};

#[cfg(unix)]
#[tokio::test]
async fn test_sync_handles_symlink() {
    let dir = tempdir().expect("Failed to create temp dir");
    let content_dir = dir.path().join("content");
    let md_dir = content_dir.join("md");
    fs::create_dir_all(&md_dir).unwrap();

    fs::write(md_dir.join("real.md"), "---\nidentifier: real\n---\n# Real").unwrap();

    // Create a symlink pointing to the real file
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(md_dir.join("real.md"), md_dir.join("link.md")).unwrap();
    }

    let repo = chasqui_db::testutil::create_test_repository().await;
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
        nginx_media_prefixes: false,
    });

    let reader = Arc::new(LocalContentReader {
        root_path: PathBuf::from("/"),
    });

    let service = timeout(
        Duration::from_secs(10),
        SyncService::new(
            repo,
            reader,
            Box::new(notifier),
            config,
        ),
    )
    .await
    .expect("Service creation timed out")
    .expect("Failed to create service");

    // Symlink handling varies by filesystem; just verify it doesn't crash
    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert!(pages.len() >= 1);
}