use crate::io::{ContentReader, ContentMetadata};
use crate::database::PageRepository;
use crate::domain::Page;
use crate::services::sync::SyncService;
use crate::services::ContentBuildNotifier;
use crate::config::ChasquiConfig;
use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// --- Manual Mock: ContentReader ---
// this "fakes" the file system so we don't have to write real files to the disk during tests
// it keeps all our "files" in a simple HashMap in memory
#[derive(Clone)]
pub struct MockContentReader {
    pub files: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl MockContentReader {
    pub fn new() -> Self {
        Self { files: Arc::new(Mutex::new(HashMap::new())) }
    }

    // helper to "create" a file in our fake world
    pub fn add_file(&self, path: &str, content: &str) {
        let mut files = self.files.lock().unwrap();
        files.insert(PathBuf::from(path), content.to_string());
    }
}

#[async_trait]
impl ContentReader for MockContentReader {
    // just look up the path in our HashMap
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let files = self.files.lock().unwrap();
        files.get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("File not found in mock: {:?}", path))
    }

    async fn get_metadata(&self, _path: &Path) -> Result<ContentMetadata> {
        // give back empty dates for now
        Ok(ContentMetadata {
            modified: None,
            created: None,
        })
    }

    // tell the system what files exist in our fake world
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
        Self { call_count: Arc::new(Mutex::new(0)) }
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
        Self { pages: Arc::new(Mutex::new(HashMap::new())) }
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
        config.clone()
    ).await.expect("Failed to create service");

    // 4. Perform the "Two-Pass" sync
    service.full_sync().await.expect("Sync failed");

    // 5. Assertions
    let pages = service.get_all_pages().await;
    assert_eq!(pages.len(), 2);

    // Verify the "Magic": post2's link should have been rewritten to "/hello"
    let post2 = service.get_page_by_identifier("post2").await.unwrap();
    assert!(post2.html_content.contains(r#"href="/hello""#));
}

// a "chaos" test to ensure the system doesn't break when files move or link to each other in loops
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
        config.clone()
    ).await.unwrap();

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
        let mut files = reader.files.lock().unwrap();
        files.remove(&PathBuf::from("/content/a.md"));
        files.insert(PathBuf::from("/content/c.md"), "---\nidentifier: a\n---\nNew location".to_string());
    }
    // simulate a batch operation (delete a.md, add c.md)
    service.process_batch(
        vec![PathBuf::from("/content/c.md")],
        vec![PathBuf::from("/content/a.md")]
    ).await.unwrap();

    // The identifier "a" should now correctly point to "c.md"
    let updated_a = service.get_page_by_identifier("a").await.unwrap();
    assert_eq!(updated_a.filename, "c.md");

    // Scenario: Broken Link
    // linking to something that doesn't exist should just leave the link as-is
    reader.add_file("/content/broken.md", "[Nowhere](void.md)");
    service.handle_file_changed(&PathBuf::from("/content/broken.md")).await.unwrap();
    
    let broken_page = service.get_page_by_identifier("broken").await.unwrap();
    assert!(broken_page.html_content.contains(r#"href="void.md""#));
}
