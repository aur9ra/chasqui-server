pub mod models;

use chasqui_core::features::model::{Feature, FeatureType};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait SyncableCache: Send + Sync {
    async fn add(&self, feature: Feature) -> Result<()>;
    async fn remove(&self, filename: &str) -> Result<()>;
    async fn get_all(&self) -> Vec<Feature>;
    async fn get_by_key(&self, key: &str) -> Option<Feature>;
    fn can_handle(&self, feature_type: FeatureType) -> bool;
}