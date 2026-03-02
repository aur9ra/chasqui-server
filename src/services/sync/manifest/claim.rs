use crate::features::model::FeatureType;
use crate::io::path_utils::path_to_identifier;
use crate::config::ChasquiConfig;
use crate::io::ContentReader;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// An identity claim for a file discovered during the first pass.
#[derive(Clone)]
pub struct ManifestClaim {
    pub feature_type: FeatureType,
    pub filename: String,
    pub mount_path: PathBuf,
    pub identifier: Option<String>,
    pub content_hash: String,
}

impl ManifestClaim {
    /// Generates a potential identity claim for a file.
    /// Returns None if the file type is unsupported (ignored).
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

        // The internal filename key is strictly relative to the mount point
        let filename = relative_path.to_string_lossy().replace("\\", "/");

        let hash = reader.get_hash(path).await?;

        // Optimization: if hash matches, no update needed
        if manifest.hashes.get(&filename) == Some(&hash) {
            return Ok(None);
        }

        let identifier = if feature_type == FeatureType::Page {
            let bytes = reader.read_bytes(path).await?;
            Some(crate::features::pages::model::Page::resolve_identity(relative_path, &bytes, config)?)
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
