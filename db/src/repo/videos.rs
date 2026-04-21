use crate::sqlite::SqliteRepository;
use anyhow::{Context, Result};
use chasqui_core::features::assets::model::CommonAssetMetadata;
use chasqui_core::features::assets::videos::model::VideoAsset;
use chrono::NaiveDateTime;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct DbVideo {
    id: Uuid,
    filename: String,
    identifier: Option<String>,
    file_path: String,
    content_hash: String,
    new_path: Option<String>,
    bytes_size: i64,
    created_at: Option<NaiveDateTime>,
    modified_at: Option<NaiveDateTime>,
    duration_seconds: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    frame_rate: Option<i64>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
}

impl TryFrom<DbVideo> for VideoAsset {
    type Error = anyhow::Error;

    fn try_from(db: DbVideo) -> Result<Self> {
        Ok(VideoAsset {
            metadata: CommonAssetMetadata {
                id: db.id,
                filename: db.filename,
                identifier: db.identifier,
                file_path: PathBuf::from(db.file_path),
                content_hash: db.content_hash,
                new_path: db.new_path.map(PathBuf::from),
        bytes_size: db.bytes_size as u64,
        created_at: db.created_at,
                modified_at: db.modified_at,
            },
            duration_seconds: db.duration_seconds.map(|v| v as u32),
            width: db.width.map(|v| v as u32),
            height: db.height.map(|v| v as u32),
            frame_rate: db.frame_rate.map(|v| v as u32),
            video_codec: db.video_codec,
            audio_codec: db.audio_codec,
        })
    }
}

impl SqliteRepository {
    pub async fn get_video_by_filename(&self, filename: &str) -> Result<Option<VideoAsset>> {
        let row = sqlx::query_as!(
            DbVideo,
            r#"
            SELECT 
                id as "id: Uuid", 
                filename, 
                identifier, 
                file_path, 
                content_hash, 
                new_path, 
        bytes_size,
        created_at,
                modified_at, 
                duration_seconds, 
                width, 
                height, 
                frame_rate, 
                video_codec, 
                audio_codec 
            FROM video_assets 
            WHERE filename = ?
            "#,
            filename
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(db_video) => Ok(Some(db_video.try_into()?)),
            None => Ok(None),
        }
    }

    pub async fn get_all_videos(&self) -> Result<Vec<VideoAsset>> {
        let rows = sqlx::query_as!(
            DbVideo,
            r#"
            SELECT 
                id as "id: Uuid", 
                filename, 
                identifier, 
                file_path, 
                content_hash, 
                new_path, 
        bytes_size,
        created_at,
                modified_at, 
                duration_seconds, 
                width, 
                height, 
                frame_rate, 
                video_codec, 
                audio_codec 
            FROM video_assets
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut video_list = Vec::new();
        for row in rows {
            video_list.push(row.try_into()?);
        }
        Ok(video_list)
    }

    pub async fn save_video(&self, video: &VideoAsset) -> Result<()> {
        let meta = &video.metadata;
        let file_path = meta.file_path.to_string_lossy().to_string();
        let new_path = meta
            .new_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let bytes_size = meta.bytes_size as i64;
        let duration_seconds = video.duration_seconds.map(|v| v as i64);
        let width = video.width.map(|v| v as i64);
        let height = video.height.map(|v| v as i64);
        let frame_rate = video.frame_rate.map(|v| v as i64);

        sqlx::query!(
            r#"
            INSERT INTO video_assets (
                id, filename, identifier, file_path, content_hash,
                new_path, bytes_size, created_at, modified_at,
                duration_seconds, width, height, frame_rate, video_codec, audio_codec
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                content_hash = excluded.content_hash,
                new_path = excluded.new_path,
                bytes_size = excluded.bytes_size,
                modified_at = excluded.modified_at,
                duration_seconds = excluded.duration_seconds,
                width = excluded.width,
                height = excluded.height,
                frame_rate = excluded.frame_rate,
                video_codec = excluded.video_codec,
                audio_codec = excluded.audio_codec
            "#,
            meta.id,
            meta.filename,
            meta.identifier,
            file_path,
            meta.content_hash,
            new_path,
            bytes_size,
            meta.created_at,
            meta.modified_at,
            duration_seconds,
            width,
            height,
            frame_rate,
            video.video_codec,
            video.audio_codec
        )
        .execute(&self.pool)
        .await
        .context(format!("Failed to save video asset {}", meta.filename))?;

        Ok(())
    }

    pub async fn delete_video(&self, filename: &str) -> Result<()> {
        sqlx::query!("DELETE FROM video_assets WHERE filename = ?", filename)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete video asset {}", filename))?;
        Ok(())
    }
}