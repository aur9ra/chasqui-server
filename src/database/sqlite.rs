use crate::database::PageRepository;
use crate::domain::Page;
use crate::features::pages::model::DbPage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};

pub struct SqliteRepository {
    pool: Pool<Sqlite>,
}

impl SqliteRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PageRepository for SqliteRepository {
    async fn get_page_by_identifier(&self, id: &str) -> Result<Option<Page>> {
        // query the database for the DbPage
        let db_page_opt =
            sqlx::query_as::<_, DbPage>("SELECT * FROM pages WHERE identifier LIKE ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;

        // translate to pure Page model
        match db_page_opt {
            Some(db_page) => {
                let page: Page = db_page.try_into()?;
                Ok(Some(page))
            }
            None => Ok(None),
        }
    }

    async fn get_page_by_filename(&self, filename: &str) -> Result<Option<Page>> {
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

    async fn get_all_pages(&self) -> Result<Vec<Page>> {
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

    async fn save_page(&self, page: &Page) -> Result<()> {
        // translate the pure Page down into a DbPage for SQLite
        let db_page: DbPage = page.into();

        // nifty UPSERT
        // it's important to have the db do the insert/update
        sqlx::query!(
            r#"
            INSERT INTO pages (
                identifier, filename, name, html_content, md_content, 
                md_content_hash, tags, modified_datetime, created_datetime
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                name = excluded.name,
                html_content = excluded.html_content,
                md_content = excluded.md_content,
                md_content_hash = excluded.md_content_hash,
                tags = excluded.tags,
                modified_datetime = excluded.modified_datetime,
                created_datetime = excluded.created_datetime
            "#,
            db_page.identifier,
            db_page.filename,
            db_page.name,
            db_page.html_content,
            db_page.md_content,
            db_page.md_content_hash,
            db_page.tags,
            db_page.modified_datetime,
            db_page.created_datetime
        )
        .execute(&self.pool)
        .await
        .context(format!("Failed to save page {}", page.filename))?;

        Ok(())
    }

    async fn delete_page(&self, filename: &str) -> Result<()> {
        sqlx::query!("DELETE FROM pages WHERE filename = ?", filename)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete page {}", filename))?;

        Ok(())
    }
}
