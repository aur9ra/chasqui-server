use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt; 
use crate::AppState;
use crate::features::pages::pages_router;
use crate::services::sync::SyncService;
use crate::tests::integration_pages_sync_service::{MockRepository, MockContentReader, MockBuildNotifier};
use crate::config::ChasquiConfig;
use std::sync::Arc;
use std::path::PathBuf;

// helper to prepare the API with some initial data
async fn setup_api_test_state() -> AppState {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: "".into(),
        content_dir,
        strip_extensions: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
    });

    // put a "seed" page into our fake file system
    reader.add_file("/content/api-test.md", "# API Test Content");
    
    let service = SyncService::new(
        Box::new(repo), 
        Box::new(reader.clone()), 
        Box::new(notifier), 
        config.clone()
    ).await.unwrap();

    // sync it so it moves from the "file system" into the API cache
    service.full_sync().await.unwrap();

    AppState {
        sync_service: Arc::new(service),
        config: config.clone(),
    }
}

// test that requesting a valid slug returns the correct HTML and JSON
#[tokio::test]
async fn test_get_page_success() {
    let state = setup_api_test_state().await;
    // build the real router but plug in our fake test state
    let app = pages_router().with_state(state);

    // simulate a GET /api-test request
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-test")
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
    
    assert_eq!(json["identifier"], "api-test");
    assert!(json["html_content"].as_str().unwrap().contains("<h1>API Test Content</h1>"));
}

// ensure the API correctly returns 404 for pages that don't exist
#[tokio::test]
async fn test_get_page_not_found() {
    let state = setup_api_test_state().await;
    let app = pages_router().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// test the "List All" endpoint
#[tokio::test]
async fn test_list_pages() {
    let state = setup_api_test_state().await;
    let app = pages_router().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    
    // verify we got an array back with our 1 seeded page
    assert!(json.is_array());
    assert_eq!(json.as_array().unwrap().len(), 1);
}
