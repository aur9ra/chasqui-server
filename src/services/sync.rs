use crate::config::ChasquiConfig;
use crate::database::PageRepository;
use crate::domain::Page;
use crate::parser::markdown::{compile_markdown_to_html, extract_frontmatter};
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

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

    fn resolve_link(&self, link: &str) -> Option<String> {
        // 1. Filter external and anchor-only links
        if link.starts_with("http://")
            || link.starts_with("https://")
            || link.starts_with("mailto:")
            || link.starts_with('#')
        {
            return Some(link.to_string());
        }

        // 2. Normalize by stripping fragments for lookup
        let parts: Vec<&str> = link.split('#').collect();
        let lookup_key = parts[0];
        let fragment = parts.get(1).map(|f| format!("#{}", f)).unwrap_or_default();

        println!("LinkResolver: Debugging link '{}'", link);
        println!("LinkResolver:   Lookup Key: '{}'", lookup_key);

        // 3. Attempt dual-index lookup in the MANIFEST (The Map of the World)
        let resolved_identifier =
            if let Some(identifier) = self.filename_to_identifier.get(lookup_key) {
                println!(
                    "LinkResolver:   Matched by filename! Resolved to identifier: '{}'",
                    identifier
                );
                Some(identifier.clone())
            } else if self.identifier_to_filename.contains_key(lookup_key) {
                println!("LinkResolver:   Matched by identifier!");
                Some(lookup_key.to_string())
            } else {
                println!("LinkResolver:   FAILED to match. Checking manifest...");
                println!(
                    "LinkResolver:     Available Filenames: {:?}",
                    self.filename_to_identifier.keys().collect::<Vec<_>>()
                );
                println!(
                    "LinkResolver:     Available Identifiers: {:?}",
                    self.identifier_to_filename.keys().collect::<Vec<_>>()
                );
                None
            };

        // 4. Return the "fixed" identifier with fragment preserved, or None if broken
        resolved_identifier.map(|id| format!("{}{}", id, fragment))
    }
}

struct SyncCache {
    pages_by_filename: HashMap<String, Page>,
}

pub struct SyncService<R: PageRepository> {
    repo: R,
    config: Arc<ChasquiConfig>,
    // The "Map of the World" - updated during the Discovery Pass
    manifest: RwLock<Manifest>,
    // our in-memory cache, indexed by filename
    cache: RwLock<SyncCache>,
}

impl<R: PageRepository> SyncService<R> {
    // async because upon creation populates internal pages cache
    pub async fn new(repo: R, config: Arc<ChasquiConfig>) -> Result<Self> {
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
            config,
            manifest: RwLock::new(manifest),
            cache: RwLock::new(SyncCache { pages_by_filename }),
        })
    }

    /// Stage 1: Discovery Pass - Register a file's identity to the global map
    pub async fn register_file_to_manifest(&self, path: &Path) -> Result<()> {
        let relative_path = path
            .strip_prefix(&self.config.content_dir)
            .with_context(|| format!("File {} is outside of content dir", path.display()))?;

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        // Shallow scan: read only enough to get frontmatter
        let raw_markdown = fs::read_to_string(path)?;
        let (frontmatter, _) = extract_frontmatter(&raw_markdown, &filename)?;

        let identifier = frontmatter.identifier.unwrap_or_else(|| {
            relative_path
                .with_extension("")
                .to_string_lossy()
                .replace("\\", "/")
        });

        let mut manifest_guard = self.manifest.write().await;
        manifest_guard.insert(filename, identifier);

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
        let manifest_guard = self.manifest.read().await;
        let filename = manifest_guard.identifier_to_filename.get(identifier)?;

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

        let raw_markdown = fs::read_to_string(path)
            .with_context(|| format!("Failed to read markdown file: {}", path.display()))?;

        // get os metadata for fallback dates
        let metadata = fs::metadata(path)?;
        let os_modified = metadata
            .modified()
            .ok()
            .map(|t| chrono::DateTime::<Utc>::from(t).naive_utc());
        let os_created = metadata
            .created()
            .ok()
            .map(|t| chrono::DateTime::<Utc>::from(t).naive_utc());

        // extract the frontmatter
        let (frontmatter, content_body) = extract_frontmatter(&raw_markdown, &filename)?;

        // resolve identifier early for manifest registration
        let identifier = frontmatter.identifier.clone().unwrap_or_else(|| {
            relative_path
                .with_extension("")
                .to_string_lossy()
                .replace("\\", "/")
        });

        // Discovery stage: Update manifest immediately so other files can link to this one
        {
            let mut manifest_guard = self.manifest.write().await;
            manifest_guard.insert(filename.clone(), identifier.clone());
        }

        // Acquire read lock for the duration of compilation to provide the resolver with manifest access
        let manifest_guard = self.manifest.read().await;

        // compile the markdown with on-the-fly link resolution using the MANIFEST
        let html_content =
            compile_markdown_to_html(&content_body, |link| manifest_guard.resolve_link(link))?;

        // hash md content
        let md_content_hash = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(raw_markdown.as_bytes())
        );

        // resolve dates and fallback to OS metadata if not in frontmatter
        let modified_datetime = resolve_datetime(frontmatter.modified_datetime, os_modified);
        let created_datetime = resolve_datetime(frontmatter.created_datetime, os_created);

        println!("Successfully processed and saved {}", filename);

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
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("Invalid file path")?
            .to_string();

        self.repo.delete_page(&filename).await?;

        self.remove_from_cache(&filename).await;

        println!("Successfully deleted {}", filename);

        Ok(())
    }
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
