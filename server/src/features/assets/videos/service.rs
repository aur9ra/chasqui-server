use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::assets::model::CommonAssetMetadata;
use chasqui_core::features::assets::videos::model::VideoAsset;
use chasqui_core::features::assets::metadata::extract_video_metadata;
use chasqui_core::io::ContentReader;
use chasqui_core::io::path_utils::normalize_path;
use crate::services::sync::manifest::Manifest;
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

pub async fn create_video_asset(
    path: &Path,
    config: &ChasquiConfig,
    reader: &dyn ContentReader,
    manifest: &Manifest,
) -> Result<VideoAsset> {
    let filename = normalize_path(path
        .strip_prefix(&config.videos_dir)
        .unwrap_or(path));

    let metadata = reader.get_metadata(path).await?;
    let content_hash = reader.get_hash(path).await?;
    let bytes_size = metadata.size;

    let identifier = manifest.file_to_id.get(&filename).cloned();

    let ext = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();

    let tech_meta = {
        let stream = reader.open_file(path).await?;
        extract_video_metadata(stream, bytes_size, &ext)
    };

    Ok(VideoAsset {
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
        duration_seconds: tech_meta.duration_seconds,
        width: tech_meta.width,
        height: tech_meta.height,
        frame_rate: tech_meta.frame_rate,
        video_codec: tech_meta.video_codec,
        audio_codec: tech_meta.audio_codec,
    })
}