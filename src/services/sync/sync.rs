use crate::config::ChasquiConfig;
use crate::database::PageRepository;
use crate::domain::Page;
use crate::features::pages::model::PageDraft;
use crate::io::ContentReader;
use crate::parser::markdown::{compile_markdown_to_html, extract_frontmatter};
use crate::services::ContentBuildNotifier;
use crate::services::sync::pages_cache::SyncCache;
use crate::services::sync::pages_manifest::Manifest;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

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
        let mut cache = SyncCache::new();

        for page in all_pages {
            manifest.insert(page.filename.clone(), page.identifier.clone());
            cache.pages_by_filename.insert(page.filename.clone(), page);
        }

        println!(
            "Orchestrator: Cache and Manifest built with {} pages.",
            cache.pages_by_filename.len()
        );

        Ok(Self {
            repo,
            reader,
            notifier,
            config,
            manifest: RwLock::new(manifest),
            cache: RwLock::new(cache),
        })
    }

    pub async fn notify_build(&self) -> Result<()> {
        self.notifier.notify().await
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

    /// Register a file's identity to the global map, begin processing
    pub async fn register_file_to_manifest(&self, path: &Path) -> Result<()> {
        let draft = self.discover_page_draft(path).await?;
        self.handle_file_changed(draft).await
    }

    /// Converts a file on disk into a PageDraft.
    /// This is the "Discovery Pass" - we read and parse the file once.
    async fn discover_page_draft(&self, path: &Path) -> Result<PageDraft> {
        let relative_path = path
            .strip_prefix(&self.config.content_dir)
            .with_context(|| format!("File {} is outside of content dir", path.display()))?;

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        let raw_markdown = self.reader.read_to_string(path).await?;
        let metadata = self.reader.get_metadata(path).await?;

        let (frontmatter, content_body) = extract_frontmatter(&raw_markdown, &filename)?;

        let identifier = frontmatter
            .identifier
            .unwrap_or_else(|| generate_default_identifier(relative_path));

        let md_content_hash = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(raw_markdown.as_bytes())
        );

        let modified_datetime = resolve_datetime(frontmatter.modified_datetime, metadata.modified);
        let created_datetime = resolve_datetime(frontmatter.created_datetime, metadata.created);

        Ok(PageDraft {
            filename,
            identifier,
            name: frontmatter.name,
            content_body,
            md_content_hash,
            tags: frontmatter.tags.unwrap_or_default(),
            modified_datetime,
            created_datetime,
        })
    }

    /// Applies the collision policy to a batch of drafts.
    async fn validate_batch_collisions(&self, drafts: Vec<PageDraft>) -> Vec<PageDraft> {
        let manifest_guard = self.manifest.read().await;
        let mut batch_claims: HashMap<String, Vec<String>> = HashMap::new();

        for draft in &drafts {
            batch_claims
                .entry(draft.identifier.clone())
                .or_default()
                .push(draft.filename.clone());
        }

        drafts
            .into_iter()
            .filter(|draft| {
                // Policy #2: Reject BOTH if collision within current batch
                if batch_claims
                    .get(&draft.identifier)
                    .map_or(false, |v| v.len() > 1)
                {
                    eprintln!(
                        "Collision: identifier '{}' claimed multiple times in batch.",
                        draft.identifier
                    );
                    return false;
                }

                // Policy #1: Reject new files that collide with existing manifest/cache records
                if manifest_guard.has_identifier(&draft.identifier) {
                    if manifest_guard.get_filename_for_identifier(&draft.identifier)
                        != Some(draft.filename.clone())
                    {
                        eprintln!(
                            "Collision: identifier '{}' already owned by other file.",
                            draft.identifier
                        );
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Processes a batch of file changes and deletions atomically to ensure consistency between db
    /// and disk.
    pub async fn process_batch(
        &self,
        changes: Vec<std::path::PathBuf>,
        deletions: Vec<std::path::PathBuf>,
    ) -> Result<()> {
        // 1. Priority: Purge Deletions
        for path in deletions {
            self.handle_file_deleted(&path).await?;
        }

        // 2. Discovery Pass: Read and parse all changes into drafts
        let mut drafts = Vec::new();
        for path in &changes {
            match self.discover_page_draft(path).await {
                Ok(draft) => drafts.push(draft),
                Err(e) => eprintln!(
                    "Orchestrator: Failed to discover draft for {}: {}",
                    path.display(),
                    e
                ),
            }
        }

        // 3. Validation Pass: Apply Collision Policy
        let valid_drafts = self.validate_batch_collisions(drafts).await;

        // 4. Manifest Update: Register all valid identities before compilation
        {
            let mut manifest_guard = self.manifest.write().await;
            for draft in &valid_drafts {
                manifest_guard.insert(draft.filename.clone(), draft.identifier.clone());
            }
        }

        // 5. Ingestion Pass: Compile and Save
        for draft in valid_drafts {
            self.handle_file_changed(draft).await?;
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
    pub async fn handle_file_changed(&self, draft: PageDraft) -> Result<()> {
        let identifier = draft.identifier.clone();
        let filename = draft.filename.clone();

        // Acquire read lock for the duration of compilation to provide the resolver with manifest access
        let manifest_guard = self.manifest.read().await;

        // compile the markdown with on-the-fly link resolution using the MANIFEST
        let html_content = compile_markdown_to_html(&draft.content_body, |link| {
            manifest_guard.resolve_link(link, &filename, &self.config)
        })?;

        let page = Page {
            identifier,
            filename,
            name: draft.name,
            html_content,
            md_content: draft.content_body,
            md_content_hash: draft.md_content_hash,
            tags: draft.tags,
            modified_datetime: draft.modified_datetime,
            created_datetime: draft.created_datetime,
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
