use crate::database::PageRepository;
use crate::domain::Page;
use crate::parser::markdown::{compile_markdown_to_html, extract_frontmatter};
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::sync::RwLock;

pub struct SyncService<R: PageRepository> {
    repo: R,
    // our in-memory cache
    // Key: filename (e.g., "post1.md")
    // Value: Page entity
    page_cache: RwLock<HashMap<String, Page>>,
}

impl<R: PageRepository> SyncService<R> {
    // async because upon creation populates internal pages cache
    pub async fn new(repo: R) -> Result<Self> {
        println!("Orchestrator: Booting up and building internal cache...");

        // get all pages
        let all_pages = repo
            .get_all_pages()
            .await
            .context("Failed to load pages for cache initialization")?;

        // index by filename
        let mut cache = HashMap::new();
        for page in all_pages {
            cache.insert(page.filename.clone(), page);
        }

        println!("Orchestrator: Cache built with {} pages.", cache.len());

        Ok(Self {
            repo,
            // wrap the HashMap in the async Read-Write Lock
            page_cache: RwLock::new(cache),
        })
    }

    pub async fn get_all_pages(&self) -> Vec<Page> {
        let cache_guard = self.page_cache.read().await;
        cache_guard.values().cloned().collect()
    }

    pub async fn get_page_by_identifier(&self, identifier: &str) -> Option<Page> {
        let cache_guard = self.page_cache.read().await;
        cache_guard
            .values()
            .find(|page| page.identifier == identifier)
            .cloned()
    }

    // helper function to validate links against cache
    pub async fn validate_links(&self, extracted_links: &[String]) -> Result<()> {
        let cache_guard = self.page_cache.read().await;

        for link in extracted_links {
            // TODO: redo path normalization
            let normalized_link = link.clone();

            if !cache_guard.contains_key(&normalized_link) {
                return Err(anyhow::anyhow!(
                    "Broken internal link detected: '{}' does not exist in the system.",
                    normalized_link
                ));
            }
        }

        Ok(())
    }

    // a file has changed and we must submit the changed file to db
    pub async fn handle_file_changed(&self, path: &Path) -> Result<()> {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("Invalid file path")?
            .to_string();

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

        // compile the markdown and get ulist of links
        let (html_content, extracted_links) = compile_markdown_to_html(&content_body)?;

        // validate links from our markdown validation, discard operation if any user supplied
        // links are invalid
        self.validate_links(&extracted_links).await?;

        // hash md content
        let md_content_hash = format!(
            "{:016x}",
            xxhash_rust::xxh3::xxh3_64(raw_markdown.as_bytes())
        );

        // resolve identifier, fallback to filename if not in frontmatter
        let identifier = frontmatter
            .identifier
            .unwrap_or_else(|| filename.to_string());

        // resolve dates and fallback to OS metadata if not in frontmatter
        let modified_datetime = resolve_datetime(frontmatter.modified_datetime, os_modified);
        let created_datetime = resolve_datetime(frontmatter.created_datetime, os_created);

        let page = Page {
            identifier,
            filename: filename.to_string(),
            name: frontmatter.name,
            html_content,
            md_content: content_body,
            md_content_hash,
            tags: frontmatter.tags.unwrap_or_default(),
            modified_datetime,
            created_datetime,
        };

        // save the pure page in our in-memory repo
        self.repo.save_page(&page).await?;

        // update page_cache for fast lookups
        let mut cache_guard = self.page_cache.write().await;
        cache_guard.insert(page.filename.clone(), page);

        println!("Successfully processed and saved {}", filename);

        Ok(())
    }

    pub async fn handle_file_deleted(&self, path: &Path) -> Result<()> {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .context("Invalid file path")?
            .to_string();

        self.repo.delete_page(&filename).await?;

        let mut cache_guard = self.page_cache.write().await;
        cache_guard.remove(&filename);

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
