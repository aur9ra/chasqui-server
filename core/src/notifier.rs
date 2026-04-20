use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ContentBuildNotifier: Send + Sync {
    async fn notify(&self) -> Result<()>;
}