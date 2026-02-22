use crate::features::pages::model::DbPage;
use crate::features::pages::repo::{get_pages_from_db, process_md_dir, process_page_operations};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Client;
use sqlx::{Pool, Sqlite};
use std::env::var;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;

/// Spawns a background task that watches for file changes and syncs the database.
pub fn start_directory_watcher(pool: Pool<Sqlite>) {
    // 1. Create a communication channel between the synchronous file watcher
    // and our asynchronous Tokio environment.
    let (tx, mut rx) = mpsc::channel(100);

    // 2. Set up the notify watcher. It takes a closure that is triggered on file events.
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            // We only care about "Modify" events (like saving a file)
            if matches!(event.kind, EventKind::Modify(_)) {
                // Send the event down the channel to our async worker
                let _ = tx.blocking_send(event);
            }
        }
    })
    .expect("Failed to initialize file watcher");

    // Tell the watcher to keep an eye on our content folder
    watcher
        .watch(Path::new("./content/md"), RecursiveMode::Recursive)
        .expect("Failed to watch content directory");

    // 3. Hire an independent worker (tokio task) to process the events.
    tokio::spawn(async move {
        // Keep the watcher alive by moving it into this async block
        let _kept_alive_watcher = watcher;
        let http_client = Client::new();

        // This loop waits sleepily until a message arrives down the channel
        while let Some(_event) = rx.recv().await {
            println!("File change detected! Syncing database...");

            // Get current pages from DB using our "cloned" pool card
            if let Ok(db_pages) = get_pages_from_db(&pool).await {
                let borrowable_db_pages: Vec<&DbPage> = db_pages.iter().collect();

                // Identify what changed
                if let Ok(page_operations) =
                    process_md_dir(Path::new("./content/md"), borrowable_db_pages)
                {
                    // Execute the changes
                    if process_page_operations(&pool, page_operations)
                        .await
                        .is_ok()
                    {
                        println!("Database synced successfully.");

                        // 4. Trigger the Node.js Webhook!
                        trigger_frontend_build(&http_client).await;
                    }
                }
            }

            // Wait a brief moment to debounce rapid events (like saving a file twice quickly)
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn trigger_frontend_build(client: &Client) {
    let webhook_url =
        var("FRONTEND_WEBHOOK_URL").unwrap_or_else(|_| "http://127.0.0.1:4000/build".to_string());
    let webhook_secret = var("WEBHOOK_SECRET").unwrap_or_default();

    println!("Triggering frontend build at {}...", webhook_url);

    let res = client
        .post(&webhook_url)
        .header("Authorization", format!("Bearer {}", webhook_secret))
        .send()
        .await;

    match res {
        Ok(response) if response.status().is_success() => {
            println!("Frontend acknowledged build request.");
        }
        Ok(response) => {
            eprintln!(
                "Frontend rejected build request. Status: {}",
                response.status()
            );
        }
        Err(e) => {
            eprintln!("Failed to connect to frontend webhook: {}", e);
        }
    }
}
