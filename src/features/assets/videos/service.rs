use crate::config::ChasquiConfig;
use crate::features::assets::model::CommonAssetMetadata;
use crate::features::assets::videos::model::VideoAsset;
use crate::features::assets::metadata::extract_video_metadata;
use crate::io::ContentReader;
use crate::io::path_utils::normalize_path;
use crate::services::sync::manifest::Manifest;
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

impl VideoAsset {
    /// Produces a VideoAsset record from a file on disk using streaming IO.
    pub async fn new_from_file(
        path: &Path,
        config: &ChasquiConfig,
        reader: &dyn ContentReader,
        manifest: &Manifest,
    ) -> Result<Self> {
        let filename = normalize_path(path
            .strip_prefix(&config.videos_dir)
            .unwrap_or(path));
        
        let metadata = reader.get_metadata(path).await?;
        let content_hash = reader.get_hash(path).await?;
        let bytes_size = metadata.size;

        let identifier = manifest.file_to_id.get(&filename).cloned();

        let ext = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).unwrap_or_default();
        let mime_type = match ext.as_str() {
            "mp4" => "video/mp4",
            "mov" => "video/quicktime",
            "webm" => "video/webm",
            _ => "application/octet-stream",
        };

        // Extract technical metadata using a streaming reader
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
                mime_type: mime_type.to_string(),
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
}
