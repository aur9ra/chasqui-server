use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use derive_more::derive::Display;
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

#[derive(sqlx::FromRow, Eq, PartialEq, Clone, Display)]
#[display("{}", filename)]
pub struct DbPage {
    pub identifier: String,
    pub filename: String,
    pub name: Option<String>,
    pub md_content: String,
    pub content_hash: String,
    pub tags: Option<String>,
    pub modified_datetime: Option<NaiveDateTime>,
    pub created_datetime: Option<NaiveDateTime>,
    pub file_path: String,
    pub new_path: Option<String>,
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

impl TryFrom<DbPage> for Page {
    type Error = anyhow::Error;

    fn try_from(db_page: DbPage) -> Result<Self, Self::Error> {
        let parsed_tags: Vec<String> = match db_page.tags {
            Some(tags_str) => serde_json::from_str(&tags_str).context(format!(
                "Failed to parse JSON tags for {}",
                db_page.filename
            ))?,
            None => Vec::new(),
        };

        Ok(Page {
            identifier: db_page.identifier,
            filename: db_page.filename,
            name: db_page.name,
            md_content: db_page.md_content,
            content_hash: db_page.content_hash,
            tags: parsed_tags,
            modified_datetime: db_page.modified_datetime,
            created_datetime: db_page.created_datetime,
            file_path: PathBuf::from(db_page.file_path),
            new_path: db_page.new_path.map(PathBuf::from),
        })
    }
}

impl From<&Page> for DbPage {
    fn from(page: &Page) -> Self {
        let tags_str = if page.tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&page.tags).unwrap_or_default())
        };

        DbPage {
            identifier: page.identifier.clone(),
            filename: page.filename.clone(),
            name: page.name.clone(),
            md_content: page.md_content.clone(),
            content_hash: page.content_hash.clone(),
            tags: tags_str,
            modified_datetime: page.modified_datetime,
            created_datetime: page.created_datetime,
            file_path: page.file_path.to_string_lossy().to_string(),
            new_path: page
                .new_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        }
    }
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
