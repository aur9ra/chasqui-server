use crate::config::ChasquiConfig;
use crate::features::assets::audio::model::AudioAsset;
use crate::features::assets::model::CommonAssetMetadata;
use crate::features::assets::metadata::extract_audio_metadata;
use crate::io::ContentReader;
use crate::io::path_utils::normalize_path;
use crate::services::sync::manifest::Manifest;
use anyhow::Result;
use std::path::Path;
use uuid::Uuid;

impl AudioAsset {
    /// Produces an AudioAsset record from a file on disk using streaming IO.
    pub async fn new_from_file(
        path: &Path,
        config: &ChasquiConfig,
        reader: &dyn ContentReader,
        manifest: &Manifest,
    ) -> Result<Self> {
        let filename = normalize_path(path
            .strip_prefix(&config.audio_dir)
            .unwrap_or(path));
        
        let metadata = reader.get_metadata(path).await?;
        let content_hash = reader.get_hash(path).await?;
        let bytes_size = metadata.size;

        let identifier = manifest.file_to_id.get(&filename).cloned();

        let mime_type = match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("mp3") => "audio/mpeg",
            Some("wav") => "audio/wav",
            Some("ogg") => "audio/ogg",
            Some("flac") => "audio/flac",
            Some("m4a") => "audio/mp4",
            _ => "application/octet-stream",
        };

        // Extract technical metadata using a streaming reader
        let tech_meta = {
            let stream = reader.open_file(path).await?;
            extract_audio_metadata(stream)
        };

        Ok(AudioAsset {
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
            bitrate_kbps: tech_meta.bitrate_kbps,
            duration_seconds: tech_meta.duration_seconds,
            sample_rate_hz: tech_meta.sample_rate_hz,
            channels: tech_meta.channels,
            codec: tech_meta.codec,
        })
    }
}
