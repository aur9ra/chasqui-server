use crate::features::model::{Feature, FeatureType};
use anyhow::Result;
use async_trait::async_trait;

pub mod sqlite;

/// The "Universal Plug" for database operations.
/// Dispatches generic Feature variants to specialized repositories.
#[async_trait]
pub trait SyncRepository: Send + Sync {
    /// Saves a feature (Insert or Update).
    async fn save_feature(&self, feature: Feature) -> Result<()>;

    /// Fetches a specific feature by its filename.
    async fn get_feature(&self, filename: &str, feature_type: FeatureType) -> Result<Option<Feature>>;

    /// Updates an existing feature.
    async fn update_feature(&self, feature: Feature) -> Result<()>;

    /// Deletes a feature from its respective table.
    async fn delete_feature(&self, filename: &str, feature_type: FeatureType) -> Result<()>;

    /// Retrieves all features of a specific type.
    async fn get_all_features(&self, feature_type: FeatureType) -> Result<Vec<Feature>>;
}
