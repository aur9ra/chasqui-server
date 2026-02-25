use crate::io::{ContentMetadata, ContentReader};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct LocalContentReader;

#[async_trait]
impl ContentReader for LocalContentReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path)?)
    }

    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        let metadata = fs::metadata(path)?;
        let modified = metadata
            .modified()
            .ok()
            .map(|t| DateTime::<Utc>::from(t).naive_utc());
        let created = metadata
            .created()
            .ok()
            .map(|t| DateTime::<Utc>::from(t).naive_utc());

        Ok(ContentMetadata { modified, created })
    }

    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file()
                && entry.path().extension().and_then(|s| s.to_str()) == Some("md")
            {
                entries.push(entry.into_path());
            }
        }
        Ok(entries)
    }
}
