pub mod models;

use crate::features::model::{Feature, FeatureType};
use anyhow::Result;
use async_trait::async_trait;

/// The universal interface for feature-specific caches.
/// Allows the SyncService to dispatch updates without knowing the internal storage logic.
#[async_trait]
pub trait SyncableCache: Send + Sync {
    /// Attempts to cache a feature. Implementation should check if it handles the variant.
    async fn add(&self, feature: Feature) -> Result<()>;

    /// Removes a feature from the cache by its filename.
    async fn remove(&self, filename: &str) -> Result<()>;

    /// Fetches all cached features from this instance.
    async fn get_all(&self) -> Vec<Feature>;

    /// Fetches a specific feature by its key (usually identifier or filename).
    async fn get_by_key(&self, key: &str) -> Option<Feature>;

    /// Identifies which FeatureType this cache instance is responsible for.
    fn can_handle(&self, feature_type: FeatureType) -> bool;
}
