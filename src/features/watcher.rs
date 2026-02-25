use crate::config::ChasquiConfig;
use crate::database::sqlite::SqliteRepository;
use crate::services::sync::SyncService;
use notify::{EventKind, RecursiveMode, Watcher};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use walkdir;

// what operations does our async worker know?
enum SyncCommand {
    SingleFile(PathBuf),
    DeleteFile(PathBuf),
}

/// Spawns a background task that watches for file changes and syncs the database.
pub fn start_directory_watcher(
    sync_service: Arc<SyncService<SqliteRepository>>,
    config: Arc<ChasquiConfig>,
) {
    // the conveyor belt
    let (tx, mut rx) = mpsc::channel::<SyncCommand>(100);

    // emergency alarm for channel overflow
    // wrap it in an arc to share with OS watcher and async worker
    let needs_full_sync = Arc::new(AtomicBool::new(false));
    let needs_full_sync_clone = needs_full_sync.clone();

    // create os watcher
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            // determine if this event is something we care about
            let command = match event.kind {
                // catch creations and modifications
                EventKind::Create(_) | EventKind::Modify(_) => event
                    .paths
                    .first()
                    .map(|p| SyncCommand::SingleFile(p.clone())),
                // catch deletions
                EventKind::Remove(_) => event
                    .paths
                    .first()
                    .map(|p| SyncCommand::DeleteFile(p.clone())),
                _ => None,
            };

            // If it's a command we care about, try to send it
            if let Some(cmd) = command {
                match tx.try_send(cmd) {
                    Ok(_) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        needs_full_sync_clone.store(true, Ordering::SeqCst);
                        println!(
                            "Warning: File event dropped due to high traffic. Triggering Full Sync."
                        );
                    }
                    Err(_) => {}
                }
            }
        }
    })
    .expect("Failed to initialize file watcher");
    // tell the watcher where to look
    watcher
        .watch(&config.content_dir, RecursiveMode::Recursive)
        .expect("Failed to watch content directory");

    // generate a worker to process reciever
    let worker_sync_service = sync_service.clone(); // Clone for the spawned task
    let worker_config = config.clone(); // Clone for the spawned task
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

                // iterate through content directory and process all files
                for entry in walkdir::WalkDir::new(&worker_config.content_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file()
                        && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
                    {
                        let path = entry.into_path();
                        if let Err(e) = worker_sync_service.handle_file_changed(&path).await {
                            eprintln!("Error during full sync of {}: {}", path.display(), e);
                        } else {
                            sync_occurred = true; // Mark sync as occurred if at least one file is processed successfully
                        }
                    }
                }

                // clear queue (we just synced the whole file system)
                while let Ok(_) = rx.try_recv() {}
            } else {
                match command {
                    // handle updates
                    SyncCommand::SingleFile(path) => {
                        println!("Processing single file change: {:?}", path);
                        if let Err(e) = worker_sync_service.handle_file_changed(&path).await {
                            eprintln!("Error processing file change {}: {}", path.display(), e);
                        } else {
                            sync_occurred = true;
                        }
                    }
                    // handle deletions
                    SyncCommand::DeleteFile(path) => {
                        println!("Processing single file deletion: {:?}", path);
                        if let Err(e) = worker_sync_service.handle_file_deleted(&path).await {
                            eprintln!("Error processing file deletion {}: {}", path.display(), e);
                        } else {
                            sync_occurred = true;
                        }
                    }
                }
            }

            if sync_occurred {
                trigger_frontend_build(&http_client, &worker_config.webhook_url, &worker_config.webhook_secret)
                    .await;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

async fn trigger_frontend_build(client: &Client, url: &str, secret: &str) {
    println!("Triggering frontend build at {}...", url);

    let res = client
        .post(url)
        .header("Authorization", format!("Bearer {}", secret))
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
