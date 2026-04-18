use chasqui_core::io::{ContentMetadata, ContentReader, SyncFile};
use chasqui_core::notifier::ContentBuildNotifier;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub use chasqui_core::testutil::MockContentReader;

#[derive(Clone)]
pub struct BlockingReader {
    pub inner: chasqui_core::testutil::MockContentReader,
    pub block_on: Arc<Mutex<HashSet<String>>>,
    pub blocked_files: Arc<Mutex<HashSet<String>>>,
    pub barrier: Arc<tokio::sync::Barrier>,
}

impl BlockingReader {
    pub fn new(inner: chasqui_core::testutil::MockContentReader, barrier: Arc<tokio::sync::Barrier>) -> Self {
        Self {
            inner,
            block_on: Arc::new(Mutex::new(HashSet::new())),
            blocked_files: Arc::new(Mutex::new(HashSet::new())),
            barrier,
        }
    }

    pub fn block_at(&self, filename: &str) {
        let mut block = self.block_on.lock().unwrap();
        block.insert(filename.to_string());
    }
}

#[async_trait]
impl ContentReader for BlockingReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let path_str = path.to_string_lossy();
        let should_block = {
            let block = self.block_on.lock().unwrap();
            let mut blocked = self.blocked_files.lock().unwrap();

            let match_found = block.iter().any(|b| path_str.contains(b));
            if match_found && !blocked.contains(&path_str.to_string()) {
                blocked.insert(path_str.to_string());
                true
            } else {
                false
            }
        };

        if should_block {
            println!("BlockingReader: [WAIT] Task for {:?} is waiting at barrier...", path_str);
            self.barrier.wait().await;
            println!("BlockingReader: [GO] Task for {:?} released from barrier!", path_str);
        }

        self.inner.read_to_string(path).await
    }

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        self.inner.read_bytes(path).await
    }

    async fn open_file(&self, path: &Path) -> Result<SyncFile> { self.inner.open_file(path).await }
    async fn get_hash(&self, path: &Path) -> Result<String> { self.inner.get_hash(path).await }
    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> { self.inner.get_metadata(path).await }
    async fn list_all_files(&self, root: &Path) -> Result<Vec<PathBuf>> { self.inner.list_all_files(root).await }
    async fn list_files_by_extension(&self, root: &Path, ext: String) { self.inner.list_files_by_extension(root, ext).await }
    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>> { self.inner.list_markdown_files(root).await }
}

#[derive(Clone)]
pub struct MockBuildNotifier {
    pub call_count: Arc<Mutex<usize>>,
    pub simulate_latency: Arc<Mutex<Option<Duration>>>,
    pub should_fail: Arc<Mutex<bool>>,
}

impl MockBuildNotifier {
    pub fn new() -> Self {
        Self {
            call_count: Arc::new(Mutex::new(0)),
            simulate_latency: Arc::new(Mutex::new(None)),
            should_fail: Arc::new(Mutex::new(false)),
        }
    }

    pub fn set_latency(&self, duration: Duration) {
        let mut latency = self.simulate_latency.lock().unwrap();
        *latency = Some(duration);
    }

    pub fn set_fail(&self, fail: bool) {
        let mut should_fail = self.should_fail.lock().unwrap();
        *should_fail = fail;
    }
}

#[async_trait]
impl ContentBuildNotifier for MockBuildNotifier {
    async fn notify(&self) -> Result<()> {
        let latency = { *self.simulate_latency.lock().unwrap() };
        if let Some(d) = latency {
            tokio::time::sleep(d).await;
        }

        let fail = { *self.should_fail.lock().unwrap() };
        if fail {
            return Err(anyhow::anyhow!("Webhook Failed"));
        }

        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        Ok(())
    }
}