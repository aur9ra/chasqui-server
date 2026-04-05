use crate::database::sqlite::SqliteRepository;
use crate::features::assets::audio::model::AudioAsset;
use crate::features::assets::model::CommonAssetMetadata;
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct DbAudio {
    id: Uuid,
    filename: String,
    identifier: Option<String>,
    file_path: String,
    content_hash: String,
    new_path: Option<String>,
    bytes_size: i64,
    mime_type: String,
    created_at: Option<NaiveDateTime>,
    modified_at: Option<NaiveDateTime>,
    bitrate_kbps: Option<i64>,
    duration_seconds: Option<i64>,
    sample_rate_hz: Option<i64>,
    channels: Option<i64>,
    codec: Option<String>,
}

impl TryFrom<DbAudio> for AudioAsset {
    type Error = anyhow::Error;

    fn try_from(db: DbAudio) -> Result<Self> {
        Ok(AudioAsset {
            metadata: CommonAssetMetadata {
                id: db.id,
                filename: db.filename,
                identifier: db.identifier,
                file_path: PathBuf::from(db.file_path),
                content_hash: db.content_hash,
                new_path: db.new_path.map(PathBuf::from),
                bytes_size: db.bytes_size as u64,
                mime_type: db.mime_type,
                created_at: db.created_at,
                modified_at: db.modified_at,
            },
            bitrate_kbps: db.bitrate_kbps.map(|v| v as u32),
            duration_seconds: db.duration_seconds.map(|v| v as u32),
            sample_rate_hz: db.sample_rate_hz.map(|v| v as u32),
            channels: db.channels.map(|v| v as u8),
            codec: db.codec,
        })
    }
}

impl SqliteRepository {
    pub async fn get_audio_by_filename(&self, filename: &str) -> Result<Option<AudioAsset>> {
        let row = sqlx::query_as!(
            DbAudio,
            r#"
            SELECT 
                id as "id: Uuid", 
                filename, 
                identifier, 
                file_path, 
                content_hash, 
                new_path, 
                bytes_size, 
                mime_type, 
                created_at, 
                modified_at, 
                bitrate_kbps, 
                duration_seconds, 
                sample_rate_hz, 
                channels, 
                codec 
            FROM audio_assets 
            WHERE filename = ?
            "#,
            filename
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(db_audio) => Ok(Some(db_audio.try_into()?)),
            None => Ok(None),
        }
    }

    pub async fn get_all_audio(&self) -> Result<Vec<AudioAsset>> {
        let rows = sqlx::query_as!(
            DbAudio,
            r#"
            SELECT 
                id as "id: Uuid", 
                filename, 
                identifier, 
                file_path, 
                content_hash, 
                new_path, 
                bytes_size, 
                mime_type, 
                created_at, 
                modified_at, 
                bitrate_kbps, 
                duration_seconds, 
                sample_rate_hz, 
                channels, 
                codec 
            FROM audio_assets
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut audio_list = Vec::new();
        for row in rows {
            audio_list.push(row.try_into()?);
        }
        Ok(audio_list)
    }

    pub async fn save_audio(&self, audio: &AudioAsset) -> Result<()> {
        let meta = &audio.metadata;
        let file_path = meta.file_path.to_string_lossy().to_string();
        let new_path = meta
            .new_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let bytes_size = meta.bytes_size as i64;
        let bitrate_kbps = audio.bitrate_kbps.map(|v| v as i64);
        let duration_seconds = audio.duration_seconds.map(|v| v as i64);
        let sample_rate_hz = audio.sample_rate_hz.map(|v| v as i64);
        let channels = audio.channels.map(|v| v as i64);

        sqlx::query!(
            r#"
            INSERT INTO audio_assets (
                id, filename, identifier, file_path, content_hash, 
                new_path, bytes_size, mime_type, created_at, modified_at,
                bitrate_kbps, duration_seconds, sample_rate_hz, channels, codec
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                content_hash = excluded.content_hash,
                new_path = excluded.new_path,
                bytes_size = excluded.bytes_size,
                mime_type = excluded.mime_type,
                modified_at = excluded.modified_at,
                bitrate_kbps = excluded.bitrate_kbps,
                duration_seconds = excluded.duration_seconds,
                sample_rate_hz = excluded.sample_rate_hz,
                channels = excluded.channels,
                codec = excluded.codec
            "#,
            meta.id,
            meta.filename,
            meta.identifier,
            file_path,
            meta.content_hash,
            new_path,
            bytes_size,
            meta.mime_type,
            meta.created_at,
            meta.modified_at,
            bitrate_kbps,
            duration_seconds,
            sample_rate_hz,
            channels,
            audio.codec
        )
        .execute(&self.pool)
        .await
        .context(format!("Failed to save audio asset {}", meta.filename))?;

        Ok(())
    }

    pub async fn delete_audio(&self, filename: &str) -> Result<()> {
        sqlx::query!("DELETE FROM audio_assets WHERE filename = ?", filename)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete audio asset {}", filename))?;
        Ok(())
    }
}