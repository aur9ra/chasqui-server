use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub md_content: String,
    pub content_hash: String,
    pub tags: Vec<String>,
    pub modified_datetime: Option<NaiveDateTime>,
    pub created_datetime: Option<NaiveDateTime>,
    pub file_path: PathBuf,
    pub new_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
pub struct JsonPage {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub md_content: String,
    pub content_hash: String,
    pub tags: Vec<String>,
    pub modified_datetime: Option<String>,
    pub created_datetime: Option<String>,
}

impl From<&Page> for JsonPage {
    fn from(page: &Page) -> Self {
        let format = "%Y-%m-%d %H:%M:%S";
        let modified_datetime = page
            .modified_datetime
            .map(|dt| dt.format(format).to_string());
        let created_datetime = page
            .created_datetime
            .map(|dt| dt.format(format).to_string());

        JsonPage {
            identifier: page.identifier.clone(),
            filename: page.filename.clone(),
            name: page.name.clone(),
            md_content: page.md_content.clone(),
            content_hash: page.content_hash.clone(),
            tags: page.tags.clone(),
            modified_datetime,
            created_datetime,
        }
    }
}