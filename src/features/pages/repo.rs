use crate::database::sqlite::SqliteRepository;
use crate::features::pages::model::{DbPage, Page};
use anyhow::{Context, Result};

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
                identifier, filename, name, html_content, md_content, 
                content_hash, tags, modified_datetime, created_datetime,
                file_path, new_path, mime_type
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                name = excluded.name,
                html_content = excluded.html_content,
                md_content = excluded.md_content,
                content_hash = excluded.content_hash,
                tags = excluded.tags,
                modified_datetime = excluded.modified_datetime,
                created_datetime = excluded.created_datetime,
                file_path = excluded.file_path,
                new_path = excluded.new_path,
                mime_type = excluded.mime_type
            "#,
            db_page.identifier,
            db_page.filename,
            db_page.name,
            db_page.html_content,
            db_page.md_content,
            db_page.content_hash,
            db_page.tags,
            db_page.modified_datetime,
            db_page.created_datetime,
            db_page.file_path,
            db_page.new_path,
            db_page.mime_type
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