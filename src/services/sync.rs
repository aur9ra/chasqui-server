use crate::config::ChasquiConfig;
use crate::database::PageRepository;
use crate::domain::Page;
use crate::io::ContentReader;
use crate::parser::markdown::{compile_markdown_to_html, extract_frontmatter};
use crate::services::ContentBuildNotifier;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

// the manifest represents our in-memory knowledge of the database
// during edit events, this will be edited before the SyncCache (for routes) and db.
struct Manifest {
    filename_to_identifier: HashMap<String, String>,
    identifier_to_filename: HashMap<String, String>,
}

impl Manifest {
    fn new() -> Self {
        Self {
            filename_to_identifier: HashMap::new(),
            identifier_to_filename: HashMap::new(),
        }
    }

    fn insert(&mut self, filename: String, identifier: String) {
        self.filename_to_identifier
            .insert(filename.clone(), identifier.clone());
        self.identifier_to_filename.insert(identifier, filename);
    }

    fn remove_by_filename(&mut self, filename: &str) {
        if let Some(identifier) = self.filename_to_identifier.remove(filename) {
            self.identifier_to_filename.remove(&identifier);
        }
    }

    // this function is called by the AST parser on all anchors.
    // this function will give the AST parser links that will navigate to the identifier and catch
    // errors
    // this function will also ignore any external links or mailtos
    fn resolve_link(&self, link: &str, config: &ChasquiConfig) -> String {
        // filter external and anchor-only links
        if link.starts_with("http://")
            || link.starts_with("https://")
            || link.starts_with("mailto:")
            || link.starts_with('#')
        {
            return link.to_string();
        }

        // normalize by stripping fragments
                    let parts: Vec<&str> = link.split('#').collect();
                    let lookup_key = parts[0];
                    let fragment = parts.get(1).map(|f| format!("#{}", f)).unwrap_or_default();
            
                    // attempt to lookup the link by filename & identifier
                    let resolved_identifier =
                        if let Some(identifier) = self.filename_to_identifier.get(lookup_key) {
                            Some(identifier.clone())
                        } else if self.identifier_to_filename.contains_key(lookup_key) {
                            Some(lookup_key.to_string())
                        } else {
                            None
                        };
                // return the "fixed" link that will navigate to the page the writer intended, or the
        // original if broken
        match resolved_identifier {
            Some(id) => {
                if config.serve_home && id == config.home_identifier {
                    format!("/{}", fragment)
                } else {
                    format!("/{}{}", id, fragment)
                }
            }
            None => link.to_string(),
        }
    }
}

// exists to quickly get a page back for our routes rather than calling the db
struct SyncCache {
    pages_by_filename: HashMap<String, Page>,
}

pub struct SyncService {
    repo: Box<dyn PageRepository>,
    reader: Box<dyn ContentReader>,
    notifier: Box<dyn ContentBuildNotifier>,
    config: Arc<ChasquiConfig>,
    // The "Map of the World" - updated during the Discovery Pass
    manifest: RwLock<Manifest>,
    // our in-memory cache, indexed by filename
    cache: RwLock<SyncCache>,
}

impl SyncService {
    // async because upon creation populates internal pages cache
    pub async fn new(
        repo: Box<dyn PageRepository>,
        reader: Box<dyn ContentReader>,
        notifier: Box<dyn ContentBuildNotifier>,
        config: Arc<ChasquiConfig>,
    ) -> Result<Self> {
        println!("Orchestrator: Booting up and building internal cache...");

        // get all pages
        let all_pages = repo
            .get_all_pages()
            .await
            .context("Failed to load pages for cache initialization")?;

        let mut manifest = Manifest::new();
        let mut pages_by_filename = HashMap::new();

        for page in all_pages {
            manifest.insert(page.filename.clone(), page.identifier.clone());
            pages_by_filename.insert(page.filename.clone(), page);
        }

        println!(
            "Orchestrator: Cache and Manifest built with {} pages.",
            pages_by_filename.len()
        );

        Ok(Self {
            repo,
            reader,
            notifier,
            config,
            manifest: RwLock::new(manifest),
            cache: RwLock::new(SyncCache { pages_by_filename }),
        })
    }

    pub async fn notify_build(&self) -> Result<()> {
        self.notifier.notify().await
    }

    /// Stage 1: Discovery Pass - Register a file's identity to the global map
    pub async fn register_file_to_manifest(&self, path: &Path) -> Result<()> {
        let relative_path = path
            .strip_prefix(&self.config.content_dir)
            .with_context(|| format!("File {} is outside of content dir", path.display()))?;

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        // get frontmatter to determine identifier via reader
        let raw_markdown = self.reader.read_to_string(path).await?;
        let (frontmatter, _) = extract_frontmatter(&raw_markdown, &filename)?;

        let identifier = frontmatter
            .identifier
            .unwrap_or_else(|| generate_default_identifier(relative_path));

        let mut manifest_guard = self.manifest.write().await;
        manifest_guard.insert(filename, identifier);

        Ok(())
    }

    /// Performs a complete synchronization of the content directory.
    pub async fn full_sync(&self) -> Result<()> {
        println!("Orchestrator: Performing full directory sync...");
        let entries = self
            .reader
            .list_markdown_files(&self.config.content_dir)
            .await
            .context("Failed to list files for full sync")?;

        self.process_batch(entries, Vec::new()).await
    }

    /// Processes a batch of file changes and deletions atomically to ensure consistency.
    pub async fn process_batch(
        &self,
        changes: Vec<std::path::PathBuf>,
        deletions: Vec<std::path::PathBuf>,
    ) -> Result<()> {
        // 1. Priority: Purge Deletions
        for path in deletions {
            self.handle_file_deleted(&path).await?;
        }

        // 2. Priority: Discovery Pass (Register all changes to Manifest)
        for path in &changes {
            self.register_file_to_manifest(path).await?;
        }

        // 3. Priority: Ingestion Pass (Compile and Save)
        for path in changes {
            self.handle_file_changed(&path).await?;
        }

        Ok(())
    }

    // handles writing to the RwLock by updating the filename index
    async fn update_cache(&self, page: Page) {
        let mut cache_guard = self.cache.write().await;
        cache_guard
            .pages_by_filename
            .insert(page.filename.clone(), page);
    }

    // handles removing a page from the stores
    async fn remove_from_cache(&self, filename: &str) {
        let mut cache_guard = self.cache.write().await;
        cache_guard.pages_by_filename.remove(filename);

        let mut manifest_guard = self.manifest.write().await;
        manifest_guard.remove_by_filename(filename);
    }

    pub async fn get_all_pages(&self) -> Vec<Page> {
        let cache_guard = self.cache.read().await;
        cache_guard.pages_by_filename.values().cloned().collect()
    }

    pub async fn get_page_by_identifier(&self, identifier: &str) -> Option<Page> {
        // Normalize: if someone asks for "", "/", or the home_identifier, give them the home page if enabled
        let lookup_key = if self.config.serve_home
            && (identifier.is_empty()
                || identifier == "/"
                || identifier == self.config.home_identifier)
        {
            &self.config.home_identifier
        } else {
            identifier
        };

        let manifest_guard = self.manifest.read().await;
        let filename = manifest_guard.identifier_to_filename.get(lookup_key)?;

        let cache_guard = self.cache.read().await;
        cache_guard.pages_by_filename.get(filename).cloned()
    }

    // a file has changed and we must submit the changed file to db
    pub async fn handle_file_changed(&self, path: &Path) -> Result<()> {
        // resolve relative filename (e.g., "posts/my-post.md")
        let relative_path = path
            .strip_prefix(&self.config.content_dir)
            .with_context(|| {
                format!(
                    "File {} is outside of content directory {}",
                    path.display(),
                    self.config.content_dir.display()
                )
            })?;

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        let raw_markdown = self
            .reader
            .read_to_string(path)
            .await
            .with_context(|| format!("Failed to read markdown file: {}", path.display()))?;

        // get os metadata for fallback dates via reader
        let metadata = self.reader.get_metadata(path).await?;
        let os_modified = metadata.modified;
        let os_created = metadata.created;

        // extract the frontmatter
        let (frontmatter, content_body) = extract_frontmatter(&raw_markdown, &filename)?;

        // resolve identifier early for manifest registration
        let identifier = frontmatter
            .identifier
            .clone()
            .unwrap_or_else(|| generate_default_identifier(relative_path));

        // Discovery stage: Update manifest immediately so other files can link to this one
        {
            let mut manifest_guard = self.manifest.write().await;
            manifest_guard.insert(filename.clone(), identifier.clone());
        }

        // Acquire read lock for the duration of compilation to provide the resolver with manifest access
        let manifest_guard = self.manifest.read().await;

        // compile the markdown with on-the-fly link resolution using the MANIFEST
        let html_content = compile_markdown_to_html(&content_body, |link| {
            manifest_guard.resolve_link(link, &self.config)
        })?;

        // hash md content
        let md_content_hash = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(raw_markdown.as_bytes())
        );

        // resolve dates and fallback to OS metadata if not in frontmatter
        let modified_datetime = resolve_datetime(frontmatter.modified_datetime, os_modified);
        let created_datetime = resolve_datetime(frontmatter.created_datetime, os_created);

        let page = Page {
            identifier,
            filename,
            name: frontmatter.name,
            html_content,
            md_content: content_body,
            md_content_hash,
            tags: frontmatter.tags.unwrap_or_default(),
            modified_datetime,
            created_datetime,
        };

        // Release the manifest lock before performing write operations
        drop(manifest_guard);

        // save the pure page in our in-memory repo
        self.repo.save_page(&page).await?;

        // update content store
        self.update_cache(page).await;

        Ok(())
    }

    pub async fn handle_file_deleted(&self, path: &Path) -> Result<()> {
        let relative_path = path.strip_prefix(&self.config.content_dir).unwrap_or(path);

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        self.repo.delete_page(&filename).await?;

        self.remove_from_cache(&filename).await;

        println!("Successfully deleted {}", filename);

        Ok(())
    }
}

fn generate_default_identifier(relative_path: &std::path::Path) -> String {
    relative_path
        .with_extension("")
        .to_string_lossy()
        .replace("\\", "/")
}

fn resolve_datetime(
    frontmatter_date: Option<String>,
    os_date: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    // tier 1: try to use frontmatter data
    if let Some(date_str) = frontmatter_date {
        // attempt to parse RFC3339
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&date_str) {
            return Some(dt.naive_utc());
        }

        // fallback to YYYY-MM-DD
        if let Ok(dt) = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
            return Some(dt.and_hms_opt(0, 0, 0).unwrap_or_default());
        }
    }

    // tier 2 & 3
    os_date
}
