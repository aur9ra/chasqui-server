use chasqui_core::features::model::FeatureType;
use chasqui_core::io::path_utils::path_to_identifier;
use chasqui_core::config::ChasquiConfig;
use chasqui_core::io::ContentReader;
use crate::features::pages::service::resolve_page_identity;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct ManifestClaim {
    pub feature_type: FeatureType,
    pub filename: String,
    pub mount_path: PathBuf,
    pub identifier: Option<String>,
    pub content_hash: String,
}

impl ManifestClaim {
    pub async fn new(
        path: &Path,
        mount_path: &Path,
        reader: &dyn ContentReader,
        config: &ChasquiConfig,
        manifest: &crate::services::sync::manifest::Manifest,
        feature_type: FeatureType,
    ) -> Result<Option<Self>> {
        let relative_path = path
            .strip_prefix(mount_path)
            .map_err(|_| anyhow::anyhow!("File {} is outside of mount path {}", path.display(), mount_path.display()))?;

        let filename = relative_path.to_string_lossy().replace("\\", "/");

        let hash = reader.get_hash(path).await?;

        if manifest.hashes.get(&filename) == Some(&hash) {
            return Ok(None);
        }

        let identifier = if feature_type == FeatureType::Page {
            let bytes = reader.read_bytes(path).await?;
            Some(resolve_page_identity(relative_path, &bytes, config)?)
        } else {
            Some(path_to_identifier(relative_path, config.asset_strip_extension))
        };

        Ok(Some(Self {
            feature_type,
            filename,
            mount_path: mount_path.to_path_buf(),
            identifier,
            content_hash: hash,
        }))
    }
}