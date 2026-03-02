use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use tower::ServiceExt; 
use crate::AppState;
use crate::features::pages::pages_router;
use crate::features::handlers::{metadata_handler, universal_dispatch_handler};
use crate::services::sync::SyncService;
use crate::tests::mocks::{MockRepository, MockBuildNotifier};
use crate::config::ChasquiConfig;
use std::sync::Arc;
use std::fs;
use tempfile::tempdir;

// helper to prepare the API with some initial data
async fn setup_api_test_state() -> (AppState, tempfile::TempDir) {
    let repo = MockRepository::new();
    let notifier = MockBuildNotifier::new();
    
    // Create a real temp directory for files that ServeFile can see
    let dir = tempdir().expect("Failed to create temp dir");
    let content_dir = dir.path().join("content");
    let frontend_path = dir.path().join("dist");
    
    fs::create_dir_all(&content_dir).unwrap();
    fs::create_dir_all(&frontend_path).unwrap();
    
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: frontend_path.clone(),
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
    });

    // Write a real file to the temp directory (Source)
    let file_path = content_dir.join("api-test.md");
    fs::write(file_path, "---\ntags:\n  - api\n  - test\n---\n# API Test Content").unwrap();
    
    // Write the "Rendered" HTML (Production - simulating Astro output)
    let html_path = frontend_path.join("api-test.html");
    fs::write(html_path, "<html><body><h1>API Test Content</h1></body></html>").unwrap();

    // Use LocalContentReader so it picks up the real file
    let reader = Arc::new(crate::io::local::LocalContentReader {
        root_path: content_dir.clone(),
    });

    let service = SyncService::new(
        Box::new(repo),
        reader,
        Box::new(notifier),
        config.clone(),
    )
    .await.unwrap();

    // sync it so it moves from the "file system" into the API cache
    service.full_sync().await.unwrap();

    (AppState {
        sync_service: Arc::new(service),
        config: config.clone(),
    }, dir)
}

// test that requesting a valid slug returns the correct JSON via metadata API
#[tokio::test]
async fn test_get_page_metadata_success() {
    let (state, _dir) = setup_api_test_state().await;
    
    let app = Router::new()
        .route("/metadata/{*identifier}", axum::routing::get(metadata_handler))
        .with_state(state);

    // simulate a GET /metadata/api-test request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metadata/api-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // we expect a 200 OK
    assert_eq!(response.status(), StatusCode::OK);

    // parse the JSON body to see if the content is correct
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

// test that requesting a valid slug returns the physical file via universal handler
#[tokio::test]
async fn test_get_page_file_success() {
    let (state, _dir) = setup_api_test_state().await;
    
    let app = Router::new()
        .fallback(universal_dispatch_handler)
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-test.html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // Should be text/html because it's a page
    assert!(response.headers()["content-type"].to_str().unwrap().contains("text/html"));
    
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<html><body><h1>API Test Content</h1></body></html>"));
    // CRITICAL: Ensure no raw markdown leaks (frontmatter delimiters)
    assert!(!body_str.contains("---"));
}

// test that requesting a slug with nested HTML (Astro style) works
#[tokio::test]
async fn test_get_nested_page_file_success() {
    let (state, _dir) = setup_api_test_state().await;
    let frontend_path = state.config.frontend_path.clone();
    
    // Simulate Astro nested output for 'nested-test'
    let nested_html_dir = frontend_path.join("nested-test");
    fs::create_dir_all(&nested_html_dir).unwrap();
    let nested_html_path = nested_html_dir.join("index.html");
    fs::write(nested_html_path, "<html><body><h1>Nested Content</h1></body></html>").unwrap();
    
    // Add the page to the content dir so it's discovered
    let nested_md_path = state.config.pages_dir.join("nested-test.md");
    fs::write(nested_md_path, "# Nested").unwrap();
    
    // Sync to pick up the new page
    state.sync_service.full_sync().await.unwrap();

    let app = Router::new()
        .fallback(universal_dispatch_handler)
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/nested-test/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("<h1>Nested Content</h1>"));
}

// CRITICAL: Ensure that if the HTML is missing, we DON'T leak the raw markdown
#[tokio::test]
async fn test_ensure_no_markdown_leak() {
    let (state, _dir) = setup_api_test_state().await;
    
    // 'api-test' has a markdown file but we will DELETE the HTML file
    let html_path = state.config.frontend_path.join("api-test.html");
    fs::remove_file(html_path).unwrap();

    let app = Router::new()
        .fallback(universal_dispatch_handler)
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Since HTML is missing, it should fallback to registry check.
    // Registry finds it's a Page, and specifically REFUSES to serve it.
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// test asset fallback: if file is NOT in dist, registry should find it in content_dir
#[tokio::test]
async fn test_asset_registry_fallback() {
    let (state, _dir) = setup_api_test_state().await;
    
    // Add an image to content dir
    let image_path = state.config.images_dir.join("logo.png");
    fs::write(image_path, "binary-data").unwrap();
    
    // Sync to register the image
    state.sync_service.full_sync().await.unwrap();

    let app = Router::new()
        .fallback(universal_dispatch_handler)
        .with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/logo.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(body, "binary-data");
}

// test the "List All" endpoint
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

// SECURITY: Ensure no server-side paths are leaked in metadata responses
#[tokio::test]
async fn test_security_metadata_leak_prevention() {
    let (state, _dir) = setup_api_test_state().await;
    
    // Add an image to ensure we test CommonAssetMetadata (which was leaky)
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

    // 1. Check for explicit keys in the data payload
    let data = &json["data"]["metadata"];
    assert!(data["file_path"].is_null(), "file_path should not be serialized. Raw body: {}", body_str);
    assert!(data["new_path"].is_null(), "new_path should not be serialized");

    // 2. Bruteforce search the raw string for the content directory path
    assert!(!body_str.contains(&content_dir_str), "Absolute path leaked in JSON body: {}", body_str);
}
