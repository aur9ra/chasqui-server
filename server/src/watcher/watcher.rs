use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::model::FeatureType;
use crate::services::sync::SyncService;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;

const DEBOUNCE_MS: u64 = 1500;

#[derive(Debug, Clone)]
pub enum SyncCommand {
    SingleFile(PathBuf, PathBuf, FeatureType),
    DeleteFile(PathBuf),
}

pub fn start_directory_watcher(
    sync_service: Arc<SyncService>,
    config: Arc<ChasquiConfig>,
) -> mpsc::Sender<SyncCommand> {
    let (tx, rx) = mpsc::channel::<SyncCommand>(100);
    let tx_clone = tx.clone();
    let service_ref = sync_service.clone();
    let needs_full_sync = Arc::new(AtomicBool::new(false));
    let needs_full_sync_worker = needs_full_sync.clone();

    tokio::spawn(run_watcher_worker(sync_service, rx, needs_full_sync_worker));

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if let Some(path) = event.paths.first() {
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                if filename.starts_with('.') || filename.ends_with('~') {
                    return;
                }

                let command = match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        if let Some((mount, f_type)) = service_ref.identify_mount(path) {
                            Some(SyncCommand::SingleFile(path.clone(), mount.to_path_buf(), f_type))
                        } else {
                            None
                        }
                    }
                    EventKind::Remove(_) => Some(SyncCommand::DeleteFile(path.clone())),
                    _ => None,
                };

                if let Some(cmd) = command {
                    if let Err(mpsc::error::TrySendError::Full(_)) = tx_clone.try_send(cmd) {
                        needs_full_sync.store(true, Ordering::SeqCst);
                    }
                }
            }
        }
    })
    .expect("Failed to initialize file watcher");

    let mut unique_roots = std::collections::HashSet::new();
    unique_roots.insert(&config.pages_dir);
    unique_roots.insert(&config.images_dir);
    unique_roots.insert(&config.audio_dir);
    unique_roots.insert(&config.videos_dir);

    for root in unique_roots {
        let _ = watcher.watch(root, RecursiveMode::Recursive);
    }

    Box::leak(Box::new(watcher));

    tx
}

pub async fn run_watcher_worker(
    sync_service: Arc<SyncService>,
    mut receiver: mpsc::Receiver<SyncCommand>,
    needs_full_sync: Arc<AtomicBool>,
) {
    let mut pending_changes = std::collections::HashMap::new();
    let mut pending_deletions = std::collections::HashSet::new();

    loop {
        let first_cmd = match receiver.recv().await {
            Some(cmd) => cmd,
            None => break,
        };

        match first_cmd {
            SyncCommand::SingleFile(p, m, t) => {
                pending_changes.insert(p.clone(), (m, t));
                pending_deletions.remove(&p);
            }
            SyncCommand::DeleteFile(p) => {
                pending_deletions.insert(p.clone());
                pending_changes.remove(&p);
            }
        }

        loop {
            let timeout =
                tokio::time::timeout(Duration::from_millis(DEBOUNCE_MS), receiver.recv()).await;
            match timeout {
                Ok(Some(cmd)) => match cmd {
                    SyncCommand::SingleFile(p, m, t) => {
                        pending_changes.insert(p.clone(), (m, t));
                        pending_deletions.remove(&p);
                    }
                    SyncCommand::DeleteFile(p) => {
                        pending_deletions.insert(p.clone());
                        pending_changes.remove(&p);
                    }
                },
                Ok(None) => break,
                Err(_) => break,
            }
        }

        let mut sync_occurred = false;
        if needs_full_sync.swap(false, Ordering::SeqCst) {
            if let Err(e) = sync_service.full_sync().await {
                eprintln!("Error: {}", e);
            } else {
                sync_occurred = true;
            }
            pending_changes.clear();
            pending_deletions.clear();
        } else {
            let changes: Vec<(PathBuf, PathBuf, FeatureType)> =
                pending_changes.drain().map(|(p, (m, t))| (p, m, t)).collect();
            let deletions: Vec<PathBuf> = pending_deletions.drain().collect();

            if !changes.is_empty() || !deletions.is_empty() {
                if let Err(e) = sync_service.process_batch(changes, deletions).await {
                    eprintln!("Error: {}", e);
                } else {
                    sync_occurred = true;
                }
            }
        }

        if sync_occurred {
            let service_clone = sync_service.clone();
            tokio::spawn(async move {
                if let Err(e) = service_clone.notify_build().await {
                    eprintln!("Sync Service: Build notification failed: {}", e);
                }
            });
        }
    }
}