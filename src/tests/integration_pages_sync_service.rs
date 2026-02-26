use crate::config::ChasquiConfig;
use crate::database::PageRepository;
use crate::domain::Page;
use crate::io::{ContentMetadata, ContentReader};
use crate::services::ContentBuildNotifier;
use crate::services::sync::SyncService;
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// --- Manual Mock: ContentReader ---
// this "fakes" the file system so we don't have to write real files to the disk during tests
// it keeps all our "files" in a simple HashMap in memory
#[derive(Clone)]
struct MockFile {
    content: String,
    metadata: ContentMetadata,
}

#[derive(Clone)]
pub struct MockContentReader {
    files: Arc<Mutex<HashMap<PathBuf, MockFile>>>,
}

impl MockContentReader {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_file(&self, path: &str, content: &str) {
        self.add_file_with_metadata(path, content, None, None);
    }

    pub fn add_file_with_metadata(
        &self,
        path: &str,
        content: &str,
        modified: Option<NaiveDateTime>,
        created: Option<NaiveDateTime>,
    ) {
        let mut files = self.files.lock().unwrap();
        files.insert(
            PathBuf::from(path),
            MockFile {
                content: content.to_string(),
                metadata: ContentMetadata { modified, created },
            },
        );
    }
}

#[async_trait]
impl ContentReader for MockContentReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let files = self.files.lock().unwrap();
        files
            .get(path)
            .map(|f| f.content.clone())
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))
    }

    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        let files = self.files.lock().unwrap();
        files
            .get(path)
            .map(|f| f.metadata.clone())
            .ok_or_else(|| anyhow::anyhow!("Metadata not found in mock: {:?}", path))
    }

    async fn list_markdown_files(&self, _root: &Path) -> Result<Vec<PathBuf>> {
        let files = self.files.lock().unwrap();
        Ok(files.keys().cloned().collect())
    }
}

// --- Manual Mock: ContentBuildNotifier ---
// this fakes the webhook system so we don't try to hit a real URL during tests
// it just counts how many times the system *tried* to trigger a build
#[derive(Clone)]
pub struct MockBuildNotifier {
    pub call_count: Arc<Mutex<usize>>,
}

impl MockBuildNotifier {
    pub fn new() -> Self {
        Self {
            call_count: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl ContentBuildNotifier for MockBuildNotifier {
    async fn notify(&self) -> Result<()> {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        Ok(())
    }
}

// --- Manual Mock: PageRepository ---
// this fakes the database so we don't need a real SQLite file for logic tests
#[derive(Clone)]
pub struct MockRepository {
    pub pages: Arc<Mutex<HashMap<String, Page>>>,
}

impl MockRepository {
    pub fn new() -> Self {
        Self {
            pages: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl PageRepository for MockRepository {
    async fn get_page_by_identifier(&self, id: &str) -> Result<Option<Page>> {
        let pages = self.pages.lock().unwrap();
        Ok(pages.values().find(|p| p.identifier == id).cloned())
    }

    async fn get_page_by_filename(&self, filename: &str) -> Result<Option<Page>> {
        let pages = self.pages.lock().unwrap();
        Ok(pages.get(filename).cloned())
    }

    async fn get_all_pages(&self) -> Result<Vec<Page>> {
        let pages = self.pages.lock().unwrap();
        Ok(pages.values().cloned().collect())
    }

    async fn save_page(&self, page: &Page) -> Result<()> {
        let mut pages = self.pages.lock().unwrap();
        pages.insert(page.filename.clone(), page.clone());
        Ok(())
    }

    async fn delete_page(&self, filename: &str) -> Result<()> {
        let mut pages = self.pages.lock().unwrap();
        pages.remove(filename);
        Ok(())
    }
}

// --- The Test Logic ---

// helper to create a config that points to our fake content directory
fn mock_config(temp_path: PathBuf) -> Arc<ChasquiConfig> {
    Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        frontend_path: "".into(),
        content_dir: temp_path,
        strip_extensions: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "http://localhost/build".into(),
        webhook_secret: "secret".into(),
    })
}

// this is a "landmark" test that verifies the core of the whole system:
// can we discover files, resolve their links, and ingest them?
#[tokio::test]
async fn test_sync_service_discovery_and_ingestion() {
    // 1. Setup our fake world
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    // 2. Add two files where one links to the other
    // post1 has a custom identifier "hello"
    reader.add_file("/content/post1.md", "---\nidentifier: hello\n---\n# World");
    // post2 uses a filename link [link](post1.md)
    reader.add_file("/content/post2.md", "# Post 2 with [link](post1.md)");

    // 3. Initialize the Orchestrator
    let service = SyncService::new(
        Box::new(repo.clone()),
        Box::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .expect("Failed to create service");

    // 4. Perform the "Two-Pass" sync
    service.full_sync().await.expect("Sync failed");

    // 5. Assertions
    let pages = service.get_all_pages().await;
    assert_eq!(pages.len(), 2);

    // Verify the "Magic": post2's link should have been rewritten to "/hello"
    // verify the ATS parser works and rewrites filenames in links -> identifiers for navigation
    // purposes
    let post2 = service.get_page_by_identifier("post2").await.unwrap();
    assert!(post2.html_content.contains(r#"href="/hello""#));
}

// a "chaos" test to ensure the system doesn't break when files move or link to each other in loops
// at startup: A -> B && B -> A: OK
// new batch: A -> B && B -> A: OK
#[tokio::test]
async fn test_sync_service_chaos_and_resilience() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let content_dir = PathBuf::from("/content");
    let config = mock_config(content_dir.clone());

    let service = SyncService::new(
        Box::new(repo.clone()),
        Box::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    // Scenario: Circular Links (A links to B, B links to A)
    // the system should handle this without getting stuck
    reader.add_file("/content/a.md", "[Go to B](b.md)");
    reader.add_file("/content/b.md", "[Go to A](a.md)");
    service.full_sync().await.unwrap();

    let page_a = service.get_page_by_identifier("a").await.unwrap();
    let page_b = service.get_page_by_identifier("b").await.unwrap();

    assert!(page_a.html_content.contains(r#"href="/b""#));
    assert!(page_b.html_content.contains(r#"href="/a""#));

    // Scenario: Rename with same identifier
    // Move a.md -> c.md, but keep identifier "a" in the metadata
    {
        reader.add_file("/content/c.md", "---\nidentifier: a\n---\nNew location");
    }
    // simulate a batch operation (delete a.md, add c.md)
    service
        .process_batch(
            vec![PathBuf::from("/content/c.md")],
            vec![PathBuf::from("/content/a.md")],
        )
        .await
        .unwrap();

    // The identifier "a" should now correctly point to "c.md"
    let updated_a = service.get_page_by_identifier("a").await.unwrap();
    assert_eq!(updated_a.filename, "c.md");

    // Scenario: Broken Link
    // linking to something that doesn't exist should just leave the link as-is
    let broken_path = PathBuf::from("/content/broken.md");
    reader.add_file("/content/broken.md", "[Nowhere](void.md)");

    // Internal API now requires discover_page_draft -> handle_file_changed
    // We can use process_batch for simplicity to simulate the full flow
    service
        .process_batch(vec![broken_path], vec![])
        .await
        .unwrap();

    let broken_page = service.get_page_by_identifier("broken").await.unwrap();
    assert!(broken_page.html_content.contains(r#"href="void.md""#));
}

#[tokio::test]
async fn test_sync_service_identifier_collision_reject_both() {
    let repo = MockRepository::new();
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = mock_config(PathBuf::from("/content"));

    let service = SyncService::new(
        Box::new(repo.clone()),
        Box::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    // Two files claiming same identifier "collision"
    reader.add_file("/content/a.md", "---\nidentifier: collision\n---\n# A");
    reader.add_file("/content/b.md", "---\nidentifier: collision\n---\n# B");

    service.full_sync().await.unwrap();

    // BOTH should be rejected
    let pages = service.get_all_pages().await;
    assert_eq!(pages.len(), 0);
}
