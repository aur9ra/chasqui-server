use crate::io::{
    verified_fs_metadata, verified_fs_read_to_string, verify_absolute_path, ContentMetadata,
    ContentReader,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct LocalContentReader {
    pub root_path: PathBuf,
}

#[async_trait]
impl ContentReader for LocalContentReader {
    async fn read_to_string(&self, path: &Path) -> Result<String> {
        let verified = verify_absolute_path(&self.root_path, path)?;
        verified_fs_read_to_string(verified)
    }

    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata> {
        let verified = verify_absolute_path(&self.root_path, path)?;
        let metadata = verified_fs_metadata(verified)?;

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
