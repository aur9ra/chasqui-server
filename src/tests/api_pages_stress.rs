use crate::AppState;
use crate::config::ChasquiConfig;
use crate::features::pages::pages_router;
use crate::services::sync::SyncService;
use crate::tests::mocks::{create_test_repository, MockBuildNotifier, MockContentReader};
use axum::{body::Body, http::Request, Router, http::StatusCode};
use rand::Rng;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::task::JoinSet;
use tokio::time::{timeout};
use tower::ServiceExt;

// helper to flood the system with N unique pages for stress testing
async fn setup_stress_state(page_count: usize) -> AppState {
    let repo = create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        pages_dir: PathBuf::from("/content"),
        images_dir: PathBuf::from("/content"),
        audio_dir: PathBuf::from("/content"),
        videos_dir: PathBuf::from("/content"),
        page_strip_extension: false,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
        port: 3000,
    });

    // generate a bunch of fake blog posts
    for i in 0..page_count {
        let path = format!("/content/post_{}.md", i);
        let content = format!("---\nidentifier: post-{}\n---\n# Post {}", i, i);
        reader.add_file(&path, &content);
    }

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier),
        config.clone(),
    )
    .await
    .unwrap();

    // ingest them all into memory
    service.full_sync().await.unwrap();

    AppState {
        sync_service: Arc::new(service),
        config,
    }
}

// the "Hammer" test: 10,000 users hitting 1,000 random pages simultaneously
// this proves that our RwLock (Read-Write Lock) can handle massive concurrency
#[tokio::test]
#[ignore] // we ignore this by default because it's heavy; run with `cargo test -- --ignored`
async fn test_api_hammer_random_access() {
    let page_count = 1000;
    let request_count = 10000;

    let state = setup_stress_state(page_count).await;
    // we use an Arc for the app so all 10,000 tasks can point to the same router
    let app = Arc::new(pages_router().with_state(state));

    let mut set = JoinSet::new();
    let start = Instant::now();

    for _ in 0..request_count {
        let app_clone = app.clone();
        // spawn a new "user" task
        set.spawn(async move {
            // generate a random target page
            let uri = {
                let mut rng = rand::rng();
                let random_id = rng.random_range(0..page_count);
                format!("/post-{}", random_id)
            };

            // clone the router (cheap pointer clone) and send the request
            let local_app = app_clone.as_ref().clone();
            let response = local_app
                .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
                .await
                .unwrap();

            let status = response.status();
            if status != 200 {
                panic!("Hammer failed with status {}. URI: {}", status, uri);
            }
        });
    }

    // wait for the "hammering" to finish
    while let Some(res) = set.join_next().await {
        res.expect("Worker task panicked during hammer test");
    }

    let duration = start.elapsed();
    println!("\nNUCLEAR RANDOM ACCESS TEST RESULT:");
    println!("Pages in system: {}", page_count);
    println!("Served {} random requests in {:?}", request_count, duration);
    println!(
        "Requests per second: {:.2}",
        request_count as f64 / duration.as_secs_f64()
    );
}

#[tokio::test]
async fn test_api_responsive_during_sync() {
    let page_count = 100;
    let request_count = 50;

    let state = setup_stress_state(page_count).await;
    let service = state.sync_service.clone();
    
    // Create a router that we'll hit while sync is happening
    let app = Router::new()
        .nest("/pages", pages_router())
        .with_state(state.clone());

    // 1. Trigger a "Slow Sync" in the background
    // We'll use a task that repeatedly calls full_sync
    // To make it slow, we can't easily inject latency into the existing service's reader
    // without re-creating it, so we'll just rely on the fact that full_sync
    // acquires write locks on the manifest and caches.
    let service_clone = service.clone();
    let sync_handle = tokio::spawn(async move {
        for _ in 0..5 {
            let _ = service_clone.full_sync().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // 2. Hammer the API while the sync is churning
    let mut set = JoinSet::new();
    let start = Instant::now();

    for i in 0..request_count {
        let app_clone = app.clone();
        set.spawn(async move {
            let uri = format!("/pages/post-{}", i % page_count);
            let response = app_clone
                .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        });
    }

    // 3. Ensure all requests finish within a reasonable "responsive" time
    // Even with sync churning, these should not be blocked for seconds.
    let timeout_duration = Duration::from_secs(2);
    let result = timeout(timeout_duration, async {
        while let Some(res) = set.join_next().await {
            res.unwrap();
        }
    }).await;

    assert!(result.is_ok(), "API requests timed out during sync - indicates heavy contention or deadlock");
    sync_handle.await.unwrap();
    
    let duration = start.elapsed();
    println!("Responsive-during-sync: {} requests in {:?}", request_count, duration);
}