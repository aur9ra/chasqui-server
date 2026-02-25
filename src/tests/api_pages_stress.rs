use axum::{body::Body, http::Request};
use tower::ServiceExt;
use crate::AppState;
use crate::features::pages::pages_router;
use crate::services::sync::SyncService;
use crate::tests::integration_pages_sync_service::{MockRepository, MockContentReader, MockBuildNotifier};
use crate::config::ChasquiConfig;
use std::sync::Arc;
use std::path::PathBuf;
use tokio::task::JoinSet;
use std::time::Instant;
use rand::Rng;

// helper to flood the system with N unique pages for stress testing
async fn setup_stress_state(page_count: usize) -> AppState {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: "".into(),
        content_dir: PathBuf::from("/content"),
        strip_extensions: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
    });

    // generate a bunch of fake blog posts
    for i in 0..page_count {
        let path = format!("/content/post_{}.md", i);
        let content = format!("---\nidentifier: post-{}\n---\n# Post {}", i, i);
        reader.add_file(&path, &content);
    }

    let service = SyncService::new(
        Box::new(repo), 
        Box::new(reader.clone()), 
        Box::new(notifier), 
        config.clone()
    ).await.unwrap();

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
                .oneshot(
                    Request::builder()
                        .uri(&uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            
            assert_eq!(response.status(), 200);
        });
    }

    // wait for the "hammering" to finish
    while let Some(_) = set.join_next().await {}
    
    let duration = start.elapsed();
    println!("\nNUCLEAR RANDOM ACCESS TEST RESULT:");
    println!("Pages in system: {}", page_count);
    println!("Served {} random requests in {:?}", request_count, duration);
    println!("Requests per second: {:.2}", request_count as f64 / duration.as_secs_f64());
}
