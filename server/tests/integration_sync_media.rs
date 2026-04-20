mod common;

use chasqui_core::features::model::Feature;
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use common::mock_config;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::test]
async fn test_sync_with_real_media_metadata() {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    reader.load_real_file(
        "/content/videos/test-video1.mp4",
        Path::new("../test_files/test-video1.mp4"),
    );
    reader.load_real_file(
        "/content/videos/test-video2.mp4",
        Path::new("../test_files/test-video2.mp4"),
    );
    reader.load_real_file(
        "/content/images/test-image-small-1.png",
        Path::new("../test_files/test-image-small-1.png"),
    );
    reader.load_real_file(
        "/content/images/test-image-small-2.png",
        Path::new("../test_files/test-image-small-2.png"),
    );
    reader.load_real_file(
        "/content/audio/test-sound1.mp3",
        Path::new("../test_files/test-sound1.mp3"),
    );
    reader.load_real_file(
        "/content/audio/test-sound2.mp3",
        Path::new("../test_files/test-sound2.mp3"),
    );
    reader.load_real_file(
        "/content/audio/test-sound3.mp3",
        Path::new("../test_files/test-sound3.mp3"),
    );

    service.full_sync().await.unwrap();

    if let Some(Feature::Video(v)) = service.get_feature_by_identifier("test-video1.mp4").await {
        assert_eq!(v.width.unwrap(), 1920);
        assert_eq!(v.height.unwrap(), 1080);
        assert!(v.duration_seconds.unwrap() >= 19 && v.duration_seconds.unwrap() <= 20);
        assert!(v.frame_rate.unwrap() >= 29 && v.frame_rate.unwrap() <= 30);
    } else {
        panic!("Video 1 missing");
    }

    if let Some(Feature::Video(v)) = service.get_feature_by_identifier("test-video2.mp4").await {
        assert_eq!(v.width.unwrap(), 1920);
        assert_eq!(v.height.unwrap(), 1080);
        assert!(v.duration_seconds.unwrap() >= 7 && v.duration_seconds.unwrap() <= 8);
        assert!(v.frame_rate.unwrap() >= 29 && v.frame_rate.unwrap() <= 30);
    } else {
        panic!("Video 2 missing");
    }

    if let Some(Feature::Image(i)) = service
        .get_feature_by_identifier("test-image-small-1.png")
        .await
    {
        assert_eq!(i.width.unwrap(), 100);
        assert_eq!(i.height.unwrap(), 100);
    } else {
        panic!("Image 1 missing");
    }

    if let Some(Feature::Audio(a)) = service.get_feature_by_identifier("test-sound1.mp3").await {
        assert!(a.duration_seconds.unwrap() >= 32 && a.duration_seconds.unwrap() <= 34);
        assert_eq!(a.sample_rate_hz.unwrap(), 24000);
        assert_eq!(a.channels.unwrap(), 2);
        assert!(a.bitrate_kbps.unwrap() >= 65 && a.bitrate_kbps.unwrap() <= 75);
        assert!(a.codec.is_some());
    } else {
        panic!("Audio 1 missing");
    }
}

#[tokio::test]
async fn test_sync_image_with_alt_text_sidecar() {
    let repo = chasqui_db::testutil::create_test_repository().await;
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

    reader.add_binary_file("/content/images/alt-test.png", vec![0, 1, 2, 3]);
    reader.add_file("/content/images/alt-test.png.alt", "A beautiful test image");

    service.full_sync().await.unwrap();

    if let Some(Feature::Image(i)) = service.get_feature_by_identifier("alt-test.png").await {
        assert_eq!(i.alt_text, Some("A beautiful test image".to_string()));
    } else {
        panic!("Image missing");
    }
}