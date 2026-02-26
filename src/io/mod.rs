use anyhow::Result;
use chrono::NaiveDateTime;
use std::path::{Path, PathBuf};
use async_trait::async_trait;

pub mod local;

#[async_trait]
pub trait ContentReader: Send + Sync {
    async fn read_to_string(&self, path: &Path) -> Result<String>;
    async fn get_metadata(&self, path: &Path) -> Result<ContentMetadata>;
    async fn list_markdown_files(&self, root: &Path) -> Result<Vec<PathBuf>>;
}

#[derive(Clone)]
pub struct ContentMetadata {
    pub modified: Option<NaiveDateTime>,
    pub created: Option<NaiveDateTime>,
}
