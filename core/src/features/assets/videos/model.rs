use crate::features::assets::model::CommonAssetMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoAsset {
    pub metadata: CommonAssetMetadata,
    pub duration_seconds: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<u32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}