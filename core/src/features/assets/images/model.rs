use crate::features::assets::model::CommonAssetMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAsset {
    pub metadata: CommonAssetMetadata,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub alt_text: Option<String>,
}