use crate::features::model::Feature;
use crate::services::sync::SyncService;
use crate::tests::integration_sync_core::mock_config;
use crate::tests::mocks::{create_test_repository, MockBuildNotifier, MockContentReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::test]
async fn test_sync_with_real_media_metadata() {
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
    )
    .await
    .unwrap();

    // Load all 7 test files with their specific locations (matches mock_config mount points)
    reader.load_real_file(
        "/content/videos/test_video1.mp4",
        Path::new("test_files/test_video1.mp4"),
    );
    reader.load_real_file(
        "/content/videos/test-video2.mp4",
        Path::new("test_files/test-video2.mp4"),
    );
    reader.load_real_file(
        "/content/images/test_image_small_1.png",
        Path::new("test_files/test_image_small_1.png"),
    );
    reader.load_real_file(
        "/content/images/test_image_small_2.png",
        Path::new("test_files/test_image_small_2.png"),
    );
    reader.load_real_file(
        "/content/audio/test-sound1.mp3",
        Path::new("test_files/test-sound1.mp3"),
    );
    reader.load_real_file(
        "/content/audio/test-sound2.mp3",
        Path::new("test_files/test-sound2.mp3"),
    );
    reader.load_real_file(
        "/content/audio/test-sound3.mp3",
        Path::new("test_files/test-sound3.mp3"),
    );

    service.full_sync().await.unwrap();

    // --- Verify Video 1 (36MB, ~20s, 30fps) ---
    if let Some(Feature::Video(v)) = service.get_feature_by_identifier("test_video1.mp4").await {
        assert_eq!(v.width.unwrap(), 1920);
        assert_eq!(v.height.unwrap(), 1080);
        assert!(v.duration_seconds.unwrap() >= 19 && v.duration_seconds.unwrap() <= 20);
        assert!(v.frame_rate.unwrap() >= 29 && v.frame_rate.unwrap() <= 30);
        // We accept that codec might be 'None' or 'Some("unknown")' for now
    } else {
        panic!("Video 1 missing");
    }

    // --- Verify Video 2 (13MB, ~7s, 29fps) ---
    if let Some(Feature::Video(v)) = service.get_feature_by_identifier("test-video2.mp4").await {
        assert_eq!(v.width.unwrap(), 1920);
        assert_eq!(v.height.unwrap(), 1080);
        assert!(v.duration_seconds.unwrap() >= 7 && v.duration_seconds.unwrap() <= 8);
        assert!(v.frame_rate.unwrap() >= 29 && v.frame_rate.unwrap() <= 30);
    } else {
        panic!("Video 2 missing");
    }

    // --- Verify Image 1 (100x100) ---
    if let Some(Feature::Image(i)) = service
        .get_feature_by_identifier("test_image_small_1.png")
        .await
    {
        assert_eq!(i.width.unwrap(), 100);
        assert_eq!(i.height.unwrap(), 100);
    } else {
        panic!("Image 1 missing");
    }

    // --- Verify Audio 1 (~33s, 24kHz, 70kbps, Stereo) ---
    if let Some(Feature::Audio(a)) = service.get_feature_by_identifier("test-sound1.mp3").await {
        assert!(a.duration_seconds.unwrap() >= 32 && a.duration_seconds.unwrap() <= 34);
        assert_eq!(a.sample_rate_hz.unwrap(), 24000);
        assert_eq!(a.channels.unwrap(), 2);
        assert!(a.bitrate_kbps.unwrap() >= 65 && a.bitrate_kbps.unwrap() <= 75);
        assert!(a.codec.is_some());
    } else {
        panic!("Audio 1 missing");
    }

    // --- Verify Audio 2 (~26s, 67kbps) ---
    if let Some(Feature::Audio(a)) = service.get_feature_by_identifier("test-sound2.mp3").await {
        assert!(a.duration_seconds.unwrap() >= 25 && a.duration_seconds.unwrap() <= 27);
        assert_eq!(a.channels.unwrap(), 2);
        assert!(a.bitrate_kbps.unwrap() >= 65 && a.bitrate_kbps.unwrap() <= 75);
    } else {
        panic!("Audio 2 missing");
    }

    // --- Verify Audio 3 (~151s, 66kbps) ---
    if let Some(Feature::Audio(a)) = service.get_feature_by_identifier("test-sound3.mp3").await {
        assert!(a.duration_seconds.unwrap() >= 150 && a.duration_seconds.unwrap() <= 152);
        assert_eq!(a.channels.unwrap(), 2);
        assert!(a.bitrate_kbps.unwrap() >= 65 && a.bitrate_kbps.unwrap() <= 75);
    } else {
        panic!("Audio 3 missing");
    }
}

#[tokio::test]
async fn test_sync_image_with_alt_text_sidecar() {
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

    // 1. Add image and a corresponding .alt sidecar file
    reader.add_binary_file("/content/images/alt-test.png", vec![0, 1, 2, 3]);
    reader.add_file("/content/images/alt-test.png.alt", "A beautiful test image");

    service.full_sync().await.unwrap();

    // 2. ASSERT: Image has the correct alt text
    if let Some(Feature::Image(i)) = service.get_feature_by_identifier("alt-test.png").await {
        assert_eq!(i.alt_text, Some("A beautiful test image".to_string()));
    } else {
        panic!("Image missing");
    }
}