use crate::domain::Page;
use anyhow::Result;
use async_trait::async_trait;

pub mod sqlite;

// a pagerepository can be shared between threads (referencable)
// sqlx::Pool is thread safe
// generic implementation of page operations, db specific implementations in "sqlite.rs", future:
// "postgresql.rs", "mysql.rs"
#[async_trait]
pub trait PageRepository: Send + Sync {
    async fn get_page_by_identifier(&self, id: &str) -> Result<Option<Page>>;
    async fn get_page_by_filename(&self, filename: &str) -> Result<Option<Page>>;
    async fn get_all_pages(&self) -> Result<Vec<Page>>;

    // write operations
    async fn save_page(&self, page: &Page) -> Result<()>;
    async fn delete_page(&self, filename: &str) -> Result<()>;
}
