use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt; 
use crate::AppState;
use crate::features::pages::pages_router;
use crate::features::handlers::metadata_handler;
use crate::services::sync::SyncService;
use crate::tests::mocks::{create_test_repository, MockBuildNotifier};
use crate::config::ChasquiConfig;
use std::sync::Arc;
use std::fs;
use tempfile::tempdir;

async fn setup_api_test_state() -> (AppState, tempfile::TempDir) {
    let repo = create_test_repository().await;
    let notifier = MockBuildNotifier::new();
    
    let dir = tempdir().expect("Failed to create temp dir");
    let content_dir = dir.path().join("content");
    
    fs::create_dir_all(&content_dir).unwrap();
    
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        pages_dir: content_dir.clone(),
        images_dir: content_dir.clone(),
        audio_dir: content_dir.clone(),
        videos_dir: content_dir.clone(),
        page_strip_extension: true,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
        port: 3000,
        nginx_media_prefixes: false,
    });

    let file_path = content_dir.join("api-test.md");
    fs::write(file_path, "---\ntags:\n  - api\n  - test\n---\n# API Test Content").unwrap();

    let reader = Arc::new(crate::io::local::LocalContentReader {
        root_path: content_dir.clone(),
    });

    let service = SyncService::new(
        repo.clone(),
        reader,
        Box::new(notifier),
        config.clone(),
    )
    .await.unwrap();

    service.full_sync().await.unwrap();

    (AppState {
        sync_service: Arc::new(service),
        config: config.clone(),
    }, dir)
}

#[tokio::test]
async fn test_get_page_metadata_success() {
    let (state, _dir) = setup_api_test_state().await;
    
    let app = Router::new()
        .route("/metadata/{*identifier}", axum::routing::get(metadata_handler))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metadata/api-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(json["type"], "Page");
    let data = &json["data"];
    assert_eq!(data["identifier"], "api-test");
    assert!(data["html_content"].as_str().unwrap().contains("<h1>API Test Content</h1>"));

    assert!(data["tags"].is_array());
    let tags = data["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
}

#[tokio::test]
async fn test_list_pages() {
    let (state, _dir) = setup_api_test_state().await;
    let app = Router::new()
        .nest("/pages", pages_router())
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/pages")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_security_metadata_leak_prevention() {
    let (state, _dir) = setup_api_test_state().await;
    
    let image_path = state.config.images_dir.join("leaky-test.png");
    fs::write(&image_path, "fake-png").unwrap();
    state.sync_service.full_sync().await.unwrap();

    let content_dir_str = state.config.pages_dir.to_string_lossy().to_string();
    
    let app = Router::new()
        .route("/metadata/{*identifier}", axum::routing::get(metadata_handler))
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metadata/leaky-test.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let data = &json["data"]["metadata"];
    assert!(data["file_path"].is_null(), "file_path should not be serialized. Raw body: {}", body_str);
    assert!(data["new_path"].is_null(), "new_path should not be serialized");

    assert!(!body_str.contains(&content_dir_str), "Absolute path leaked in JSON body: {}", body_str);
}