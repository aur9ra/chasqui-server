use crate::config::ChasquiConfig;
use crate::features::assets::images::model::ImageAsset;
use crate::features::assets::model::CommonAssetMetadata;
use crate::features::assets::metadata::extract_image_metadata;
use crate::io::ContentReader;
use crate::io::path_utils::normalize_path;
use crate::services::sync::manifest::Manifest;
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

impl ImageAsset {
    /// Produces an ImageAsset record from a file on disk using streaming IO.
    pub async fn new_from_file(
        path: &Path,
        config: &ChasquiConfig,
        reader: &dyn ContentReader,
        manifest: &Manifest,
    ) -> Result<Self> {
        let filename = normalize_path(path
            .strip_prefix(&config.images_dir)
            .unwrap_or(path));
        
        let metadata = reader.get_metadata(path).await?;
        let content_hash = reader.get_hash(path).await?;
        let bytes_size = metadata.size;

        let identifier = manifest.file_to_id.get(&filename).cloned();

        let mime_type = match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("svg") => "image/svg+xml",
            _ => "application/octet-stream",
        };

        // Extract technical metadata using a streaming reader
        let tech_meta = {
            let stream = reader.open_file(path).await?;
            extract_image_metadata(stream)
        };

        // Look for an alt-text sidecar file (e.g. image.png.alt)
        let mut alt_text = None;
        let alt_path = path.with_extension(format!("{}.alt", path.extension().and_then(|s| s.to_str()).unwrap_or("")));
        if let Ok(content) = reader.read_to_string(&alt_path).await {
            alt_text = Some(content.trim().to_string());
        }

        Ok(ImageAsset {
            metadata: CommonAssetMetadata {
                id: Uuid::new_v4(),
                filename,
                identifier,
                file_path: path.to_path_buf(),
                content_hash,
                new_path: None,
                bytes_size,
                mime_type: mime_type.to_string(),
                created_at: metadata.created,
                modified_at: metadata.modified,
            },
            width: tech_meta.width,
            height: tech_meta.height,
            alt_text,
        })
    }
}
