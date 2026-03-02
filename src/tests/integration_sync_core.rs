use crate::config::ChasquiConfig;
use crate::features::model::{Feature, FeatureType};
use crate::services::sync::SyncService;
use crate::tests::mocks::{MockBuildNotifier, MockContentReader, MockRepository};
use chrono::NaiveDate;
use std::path::{PathBuf};
use std::sync::Arc;

// helper to create a config that points to our fake content directory
pub fn mock_config(temp_path: PathBuf) -> Arc<ChasquiConfig> {
    Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: "".into(),
        pages_dir: temp_path.join("md"),
        images_dir: temp_path.join("images"),
        audio_dir: temp_path.join("audio"),
        videos_dir: temp_path.join("videos"),
        page_strip_extension: true,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "http://localhost/build".into(),
        webhook_secret: "secret".into(),
    })
}

#[tokio::test]
async fn test_sync_service_discovery_and_ingestion() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    reader.add_file("/content/md/post1.md", "---
identifier: hello
---
# World");
    reader.add_file("/content/md/post2.md", "# Post 2 with [link](post1.md)");

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.expect("Failed to create service");

    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(pages.len(), 2);

    let feature = service.get_feature_by_identifier("post2").await.unwrap();
    let post2 = if let Feature::Page(p) = feature { p } else { panic!("Expected page") };
    assert!(post2.html_content.contains(r#"href="/hello""#));
}

#[tokio::test]
async fn test_sync_service_link_validation() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    reader.add_file("/content/md/a.md", "[Go to B](b.md)");
    reader.add_file("/content/md/b.md", "[Go to A](a.md)");
    service.full_sync().await.unwrap();

    let page_a = if let Some(Feature::Page(p)) = service.get_feature_by_identifier("a").await { p } else { panic!("Expected page a") };
    let page_b = if let Some(Feature::Page(p)) = service.get_feature_by_identifier("b").await { p } else { panic!("Expected page b") };

    assert!(page_a.html_content.contains(r#"href="/b""#));
    assert!(page_b.html_content.contains(r#"href="/a""#));

    reader.add_file("/content/md/c.md", "---
identifier: a
---
New location");
    service.process_batch(
        vec![(PathBuf::from("/content/md/c.md"), config.pages_dir.clone(), FeatureType::Page)],
        vec![PathBuf::from("/content/md/a.md")],
    ).await.unwrap();

    let updated_a = if let Some(Feature::Page(p)) = service.get_feature_by_identifier("a").await { p } else { panic!("Expected updated page a") };
    assert_eq!(updated_a.filename, "c.md");
}

#[tokio::test]
async fn test_sync_service_identifier_collision_reject_both() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    reader.add_file("/content/md/a.md", "---
identifier: collision
---
# A");
    reader.add_file("/content/md/b.md", "---
identifier: collision
---
# B");

    service.full_sync().await.unwrap();

    let pages = service.get_all_features_by_type(FeatureType::Page).await;
    assert_eq!(pages.len(), 0);
}

#[tokio::test]
async fn test_sync_service_datetime_resolution() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let time_a = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let time_b = NaiveDate::from_ymd_opt(2026, 12, 25).unwrap().and_hms_opt(0, 0, 0).unwrap();
    let time_b_str = "2026-12-25T00:00:00Z";

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    reader.add_file_with_metadata("/content/md/fs_only.md", "# Title", Some(time_a), Some(time_a));
    reader.add_file("/content/md/fm_only.md", &format!("---
modified_datetime: {}
---
# Title", time_b_str));
    reader.add_file_with_metadata("/content/md/both.md", &format!("---
modified_datetime: {}
---
# Title", time_b_str), Some(time_a), Some(time_a));

    service.full_sync().await.unwrap();

    let p1 = if let Some(Feature::Page(p)) = service.get_feature_by_identifier("fs_only").await { p } else { panic!("Expected p1") };
    assert_eq!(p1.modified_datetime, Some(time_a));

    let p2 = if let Some(Feature::Page(p)) = service.get_feature_by_identifier("fm_only").await { p } else { panic!("Expected p2") };
    assert_eq!(p2.modified_datetime, Some(time_b));
}

#[tokio::test]
async fn test_sync_prevent_identity_hijack() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        Box::new(repo.clone()),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    ).await.unwrap();

    // 1. First Pass: Add an image 'logo.png' -> Identifier 'logo.png'
    reader.add_binary_file("/content/images/logo.png", vec![0, 1, 2, 3]);
    service.full_sync().await.unwrap();

    {
        let manifest = service.manifest.read().await;
        assert!(manifest.id_to_file.contains_key("logo.png"));
    }

    // 2. Second Pass: Add a page 'hijack.md' that CLAIMS the 'logo.png' identifier via frontmatter
    reader.add_file("/content/md/hijack.md", "---
identifier: logo.png
---
# Hijack Attempt");
    
    service.process_batch(
        vec![(PathBuf::from("/content/md/hijack.md"), config.pages_dir.clone(), FeatureType::Page)],
        vec![]
    ).await.unwrap();

    // 3. The Hijack should be REJECTED. Identifier 'logo.png' should still belong to the image.
    let manifest_after = service.manifest.read().await;
    let owner = manifest_after.id_to_file.get("logo.png").unwrap();
    assert_eq!(owner, "logo.png"); // The filename of the original image
    assert_eq!(manifest_after.feature_types.get(owner), Some(&FeatureType::Image));
}
