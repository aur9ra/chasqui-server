use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::assets::images::model::ImageAsset;
use chasqui_core::features::assets::model::CommonAssetMetadata;
use chasqui_core::features::assets::metadata::extract_image_metadata;
use chasqui_core::io::ContentReader;
use chasqui_core::io::path_utils::normalize_path;
use crate::services::sync::manifest::Manifest;
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

pub async fn create_image_asset(
    path: &Path,
    config: &ChasquiConfig,
    reader: &dyn ContentReader,
    manifest: &Manifest,
) -> Result<ImageAsset> {
    let filename = normalize_path(path
        .strip_prefix(&config.images_dir)
        .unwrap_or(path));

    let metadata = reader.get_metadata(path).await?;
    let content_hash = reader.get_hash(path).await?;
    let bytes_size = metadata.size;

    let identifier = manifest.file_to_id.get(&filename).cloned();

    let tech_meta = {
        let stream = reader.open_file(path).await?;
        extract_image_metadata(stream)
    };

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
        created_at: metadata.created,
            modified_at: metadata.modified,
        },
        width: tech_meta.width,
        height: tech_meta.height,
        alt_text,
    })
}