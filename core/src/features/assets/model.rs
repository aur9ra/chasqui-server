use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommonAssetMetadata {
    pub id: Uuid,
    pub filename: String,
    pub identifier: Option<String>,
    #[serde(skip_serializing)]
    pub file_path: PathBuf,
    pub content_hash: String,
    #[serde(skip_serializing)]
    pub new_path: Option<PathBuf>,
    pub bytes_size: u64,
    pub created_at: Option<NaiveDateTime>,
    pub modified_at: Option<NaiveDateTime>,
}