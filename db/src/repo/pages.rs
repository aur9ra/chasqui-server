use crate::sqlite::SqliteRepository;
use anyhow::{Context, Result};
use chasqui_core::features::pages::model::Page;
use chrono::NaiveDateTime;
use derive_more::derive::Display;

use std::path::PathBuf;

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

impl SqliteRepository {
    pub async fn get_page_by_identifier(&self, id: &str) -> Result<Option<Page>> {
        let db_page_opt =
            sqlx::query_as::<_, DbPage>("SELECT * FROM pages WHERE identifier LIKE ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        match db_page_opt {
            Some(db_page) => {
                let page: Page = db_page.try_into()?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    pub async fn get_page_by_filename(&self, filename: &str) -> Result<Option<Page>> {
        let db_page_opt = sqlx::query_as::<_, DbPage>("SELECT * FROM pages WHERE filename = ?")
            .bind(filename)
            .fetch_optional(&self.pool)
            .await?;

        match db_page_opt {
            Some(db_page) => {
                let page: Page = db_page.try_into()?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    pub async fn get_all_pages(&self) -> Result<Vec<Page>> {
        let db_pages = sqlx::query_as::<_, DbPage>("SELECT * FROM pages")
            .fetch_all(&self.pool)
            .await?;

        let mut pages = Vec::new();
        for db_page in db_pages {
            let page: Page = db_page.try_into()?;
            pages.push(page);
        }

        Ok(pages)
    }

    pub async fn save_page(&self, page: &Page) -> Result<()> {
        let db_page: DbPage = page.into();

        sqlx::query!(
            r#"
            INSERT INTO pages (
                identifier, filename, name, md_content, 
                content_hash, tags, modified_datetime, created_datetime,
                file_path, new_path
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                name = excluded.name,
                md_content = excluded.md_content,
                content_hash = excluded.content_hash,
                tags = excluded.tags,
                modified_datetime = excluded.modified_datetime,
                created_datetime = excluded.created_datetime,
                file_path = excluded.file_path,
                new_path = excluded.new_path
            "#,
            db_page.identifier,
            db_page.filename,
            db_page.name,
            db_page.md_content,
            db_page.content_hash,
            db_page.tags,
            db_page.modified_datetime,
            db_page.created_datetime,
            db_page.file_path,
            db_page.new_path
        )
        .execute(&self.pool)
        .await
        .context(format!("Failed to save page {}", page.filename))?;

        Ok(())
    }

    pub async fn delete_page(&self, filename: &str) -> Result<()> {
        sqlx::query!("DELETE FROM pages WHERE filename = ?", filename)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete page {}", filename))?;

        Ok(())
    }
}