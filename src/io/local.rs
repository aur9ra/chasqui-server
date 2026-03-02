use crate::io::{
    verified_fs_metadata, verified_fs_read, verified_fs_read_to_string, verify_absolute_path,
    ContentMetadata, ContentReader,
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

    async fn read_bytes(&self, path: &Path) -> Result<Vec<u8>> {
        let verified = verify_absolute_path(&self.root_path, path)?;
        verified_fs_read(verified)
    }

    async fn open_file(&self, path: &Path) -> Result<crate::io::SyncFile> {
        let verified = verify_absolute_path(&self.root_path, path)?;
        let file = std::fs::File::open(verified)?;
        Ok(Box::new(file))
    }

    async fn get_hash(&self, path: &Path) -> Result<String> {
        use std::io::{Read, BufReader};
        use xxhash_rust::xxh3::Xxh3;

        let verified = verify_absolute_path(&self.root_path, path)?;
        let file = std::fs::File::open(verified)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut hasher = Xxh3::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:016x}", hasher.digest()))
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

        Ok(ContentMetadata { 
            modified, 
            created,
            size: metadata.len(),
        })
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

    async fn list_all_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                entries.push(entry.into_path());
            }
        }
        Ok(entries)
    }

    async fn list_files_by_extension(&self, _root: &Path, _extension: String) {
        // Implementation logic if needed, but SyncService will likely use list_all_files
        // or we can implement it similarly to list_markdown_files
    }
}
