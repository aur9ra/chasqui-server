use std::sync::OnceLock;
use crate::features::pages::model::DbPage;
use crate::features::pages::repo::{get_pages_from_db, process_md_dir, process_page_operations, process_single_file, get_entry_by_filename};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use reqwest::Client;
use sqlx::{Pool, Sqlite};
use std::env::var;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

// config cache
struct WebhookConfig {
    url: String,
    secret: String,
}

static WEBHOOK_CONFIG: OnceLock<WebhookConfig> = OnceLock::new();

fn get_webhook_config() -> &'static WebhookConfig {
    WEBHOOK_CONFIG.get_or_init(|| WebhookConfig {
        url: var("FRONTEND_WEBHOOK_URL").unwrap_or_else(|_| "http://127.0.0.1:4000/build".to_string()),
        secret: var("WEBHOOK_SECRET").unwrap_or_default(),
    })
}

// what operations does our async worker know?
enum SyncCommand {
    SingleFile(PathBuf),
    FullSync,
}

/// Spawns a background task that watches for file changes and syncs the database.
pub fn start_directory_watcher(pool: Pool<Sqlite>) {
    // 1. The Conveyor Belt (Channel)
    let (tx, mut rx) = mpsc::channel::<SyncCommand>(100);
    
    // emergency alarm for channel overflow
    // wrap it in an arc to share with OS watcher and async worker
    let needs_full_sync = Arc::new(AtomicBool::new(false));
    let needs_full_sync_clone = needs_full_sync.clone();

    // create os watcher
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_)) {
                
                // grab the path of the file that was modified
                if let Some(path) = event.paths.first() {
                    
                    // try to send the single file to the worker
                    // 'try_send' does NOT block. if the queue is full, it immediately returns an Err.
                    match tx.try_send(SyncCommand::SingleFile(path.clone())) {
                        Ok(_) => {},
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            // the channel is overflowing
                            // flip the emergency alarm to true
                            // Ordering::SeqCst propogates this info to all threads instantly
                            needs_full_sync_clone.store(true, Ordering::SeqCst);
                            println!("Warning: File event dropped due to high traffic. Triggering Full Sync.");
                        },
                        Err(_) => {}
                    }
                }
            }
        }
    }).expect("Failed to initialize file watcher");

    // tell the watcher where to look
    watcher
        .watch(Path::new("./content/md"), RecursiveMode::Recursive)
        .expect("Failed to watch content directory");

    // generate a worker to process reciever
    tokio::spawn(async move {
        let _kept_alive_watcher = watcher;
        let http_client = Client::new();

        // Check the channel for messages
        while let Some(command) = rx.recv().await {
            
            let mut sync_occurred = false;

            // did we overflow while we were busy?
            // swap(false) reads the current value and resets it to false atomically,
            // again using Ordering::SeqCst to propogate the lowering the flag to all threads
            if needs_full_sync.swap(false, Ordering::SeqCst) {
                println!("Executing Fallback Full Directory Sync...");
                
                // process file logic
                if let Ok(db_pages) = get_pages_from_db(&pool).await {
                                        let borrowable_db_pages: Vec<&DbPage> = db_pages.iter().collect();
                                        
                                        if let Ok(page_operations) = process_md_dir(Path::new("./content/md"), borrowable_db_pages) {
                                        if process_page_operations(&pool, page_operations).await.is_ok() {
                                            sync_occurred = true;
                                        }
                                    }
                                    }
                
                // clear queue (we just synced the whole file system)
                while let Ok(_) = rx.try_recv() {} 
            } else {
                // Normal Operation: Handle the command from the queue
match command {
                    SyncCommand::SingleFile(path) => {
                        println!("Processing single file change: {:?}", path);
                        
                        // Extract filename to query the database
                        let md_location_prefix = Path::new("./content/md/");
                        let relative_path = path.strip_prefix(md_location_prefix).unwrap_or(&path);
                        let filename = relative_path.to_string_lossy().to_string();

                        // 2. Single File Logic
                        // Query ONLY the file that changed
                        if let Ok(db_page_opt) = get_entry_by_filename(&filename, &pool).await {
                            // Process it
                            if let Ok(operation_report) = process_single_file(&path, db_page_opt) {
                                // Execute the single operation by wrapping it in a Vec
                                let ops = vec![operation_report];
                                if process_page_operations(&pool, ops).await.is_ok() {
                                    sync_occurred = true;
                                }
                            }
                        }
                    },
                    SyncCommand::FullSync => {} // not yet implemented
                }
            }

            // if a database write happened, try to trigger the node.js build
            if sync_occurred {
                trigger_frontend_build(&http_client).await;
            }
            
            // Our 500ms debounce
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn trigger_frontend_build(client: &Client) {
    let config = get_webhook_config();

    println!("Triggering frontend build at {}...", config.url);

    let res = client
        .post(&config.url)
        .header("Authorization", format!("Bearer {}", config.secret))
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
