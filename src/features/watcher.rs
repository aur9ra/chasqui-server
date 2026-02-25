use crate::config::ChasquiConfig;
use crate::services::sync::SyncService;
use notify::{EventKind, RecursiveMode, Watcher};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

const DEBOUNCE_MS: u64 = 1500;

// what operations does our async worker know?
enum SyncCommand {
    SingleFile(PathBuf),
    DeleteFile(PathBuf),
}

/// Spawns a background task that watches for file changes and syncs the database.
pub fn start_directory_watcher(
    sync_service: Arc<SyncService>,
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
            // only care about the first path for these simple events
            if let Some(path) = event.paths.first() {
                // Filter: Only care about .md files and ignore editor swap/temp files
                let ext = path.extension().and_then(|s| s.to_str());
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                if ext != Some("md") || filename.starts_with('.') || filename.ends_with('~') {
                    return;
                }

                // determine if this event is something we care about
                let command = match event.kind {
                    // catch creations and modifications
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        Some(SyncCommand::SingleFile(path.clone()))
                    }
                    // catch deletions
                    EventKind::Remove(_) => Some(SyncCommand::DeleteFile(path.clone())),
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

        let mut pending_changes = std::collections::HashSet::new();
        let mut pending_deletions = std::collections::HashSet::new();

        loop {
            // 1. Wait for the FIRST message to arrive
            let first_cmd = match rx.recv().await {
                Some(cmd) => cmd,
                None => break, // Channel closed, exit worker
            };

            // Add first message to the appropriate set
            match first_cmd {
                SyncCommand::SingleFile(p) => {
                    pending_changes.insert(p.clone());
                    pending_deletions.remove(&p);
                }
                SyncCommand::DeleteFile(p) => {
                    pending_deletions.insert(p.clone());
                    pending_changes.remove(&p);
                }
            }

            // 2. Collection Mode: Continue picking up messages until we see silence
            loop {
                let timeout =
                    tokio::time::timeout(Duration::from_millis(DEBOUNCE_MS), rx.recv()).await;

                match timeout {
                    Ok(Some(cmd)) => {
                        // More activity! Add to sets and reset the silence timer
                        match cmd {
                            SyncCommand::SingleFile(p) => {
                                pending_changes.insert(p.clone());
                                pending_deletions.remove(&p);
                            }
                            SyncCommand::DeleteFile(p) => {
                                pending_deletions.insert(p.clone());
                                pending_changes.remove(&p);
                            }
                        }
                    }
                    Ok(None) => break, // Channel closed
                    Err(_) => break,   // 500ms of silence reached! Proceed to processing
                }
            }

            let mut sync_occurred = false;

            // 3. Process the accumulated batch
            if needs_full_sync.swap(false, Ordering::SeqCst) {
                if let Err(e) = worker_sync_service.full_sync().await {
                    eprintln!("Error during fallback full sync: {}", e);
                } else {
                    sync_occurred = true;
                }
                // Clear the sets as full sync covers everything
                pending_changes.clear();
                pending_deletions.clear();
            } else {
                // Collect into vectors for the batch call
                let changes: Vec<PathBuf> = pending_changes.drain().collect();
                let deletions: Vec<PathBuf> = pending_deletions.drain().collect();

                if !changes.is_empty() || !deletions.is_empty() {
                    if let Err(e) = worker_sync_service.process_batch(changes, deletions).await {
                        eprintln!("Error processing batch: {}", e);
                    } else {
                        sync_occurred = true;
                    }
                }
            }

            // 4. Trigger build ONCE per batch
            if sync_occurred {
                trigger_frontend_build(
                    &http_client,
                    &worker_config.webhook_url,
                    &worker_config.webhook_secret,
                )
                .await;
            }
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
