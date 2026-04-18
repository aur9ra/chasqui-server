use crate::features::assets::model::CommonAssetMetadata;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioAsset {
    pub metadata: CommonAssetMetadata,
    pub bitrate_kbps: Option<u32>,
    pub duration_seconds: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub codec: Option<String>,
}