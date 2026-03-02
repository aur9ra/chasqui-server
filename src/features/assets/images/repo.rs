use crate::database::sqlite::SqliteRepository;
use crate::features::assets::images::model::ImageAsset;
use crate::features::assets::model::CommonAssetMetadata;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use std::path::PathBuf;
use uuid::Uuid;

#[async_trait]
pub trait ImageRepository: Send + Sync {
    async fn get_image_by_filename(&self, filename: &str) -> Result<Option<ImageAsset>>;
    async fn get_all_images(&self) -> Result<Vec<ImageAsset>>;
    async fn save_image(&self, image: &ImageAsset) -> Result<()>;
    async fn delete_image(&self, filename: &str) -> Result<()>;
}

#[derive(sqlx::FromRow)]
struct DbImage {
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
    width: Option<i64>,
    height: Option<i64>,
    alt_text: Option<String>,
}

impl TryFrom<DbImage> for ImageAsset {
    type Error = anyhow::Error;

    fn try_from(db: DbImage) -> Result<Self> {
        Ok(ImageAsset {
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
            width: db.width.map(|w| w as u32),
            height: db.height.map(|h| h as u32),
            alt_text: db.alt_text,
        })
    }
}

#[async_trait]
impl ImageRepository for SqliteRepository {
    async fn get_image_by_filename(&self, filename: &str) -> Result<Option<ImageAsset>> {
        let row = sqlx::query_as!(
            DbImage,
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
                width, 
                height, 
                alt_text 
            FROM image_assets 
            WHERE filename = ?
            "#,
            filename
        )
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(db_image) => Ok(Some(db_image.try_into()?)),
            None => Ok(None),
        }
    }

    async fn get_all_images(&self) -> Result<Vec<ImageAsset>> {
        let rows = sqlx::query_as!(
            DbImage,
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
                width, 
                height, 
                alt_text 
            FROM image_assets
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut images = Vec::new();
        for row in rows {
            images.push(row.try_into()?);
        }
        Ok(images)
    }

    async fn save_image(&self, image: &ImageAsset) -> Result<()> {
        let meta = &image.metadata;
        let file_path = meta.file_path.to_string_lossy().to_string();
        let new_path = meta
            .new_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let bytes_size = meta.bytes_size as i64;
        let width = image.width.map(|w| w as i64);
        let height = image.height.map(|h| h as i64);

        sqlx::query!(
            r#"
            INSERT INTO image_assets (
                id, filename, identifier, file_path, content_hash, 
                new_path, bytes_size, mime_type, created_at, modified_at,
                width, height, alt_text
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(filename) DO UPDATE SET
                identifier = excluded.identifier,
                content_hash = excluded.content_hash,
                new_path = excluded.new_path,
                bytes_size = excluded.bytes_size,
                mime_type = excluded.mime_type,
                modified_at = excluded.modified_at,
                width = excluded.width,
                height = excluded.height,
                alt_text = excluded.alt_text
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
            width,
            height,
            image.alt_text
        )
        .execute(&self.pool)
        .await
        .context(format!("Failed to save image asset {}", meta.filename))?;

        Ok(())
    }

    async fn delete_image(&self, filename: &str) -> Result<()> {
        sqlx::query!("DELETE FROM image_assets WHERE filename = ?", filename)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete image asset {}", filename))?;
        Ok(())
    }
}
