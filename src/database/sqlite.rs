use crate::features::model::{Feature, FeatureType};
use anyhow::Result;
use sqlx::{Pool, Sqlite};

#[derive(Clone)]
pub struct SqliteRepository {
    pub(crate) pool: Pool<Sqlite>,
}

impl SqliteRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub async fn save_feature(&self, feature: Feature) -> Result<()> {
        match feature {
            Feature::Page(page) => self.save_page(&page).await,
            Feature::Image(img) => self.save_image(&img).await,
            Feature::Audio(aud) => self.save_audio(&aud).await,
            Feature::Video(vid) => self.save_video(&vid).await,
        }
    }

    pub async fn get_feature(&self, filename: &str, feature_type: FeatureType) -> Result<Option<Feature>> {
        match feature_type {
            FeatureType::Page => Ok(self.get_page_by_filename(filename).await?.map(Feature::Page)),
            FeatureType::Image => Ok(self.get_image_by_filename(filename).await?.map(Feature::Image)),
            FeatureType::Audio => Ok(self.get_audio_by_filename(filename).await?.map(Feature::Audio)),
            FeatureType::Video => Ok(self.get_video_by_filename(filename).await?.map(Feature::Video)),
        }
    }

    pub async fn update_feature(&self, feature: Feature) -> Result<()> {
        self.save_feature(feature).await
    }

    pub async fn delete_feature(&self, filename: &str, feature_type: FeatureType) -> Result<()> {
        match feature_type {
            FeatureType::Page => self.delete_page(filename).await,
            FeatureType::Image => self.delete_image(filename).await,
            FeatureType::Audio => self.delete_audio(filename).await,
            FeatureType::Video => self.delete_video(filename).await,
        }
    }

    pub async fn get_all_features(&self, feature_type: FeatureType) -> Result<Vec<Feature>> {
        match feature_type {
            FeatureType::Page => {
                let pages = self.get_all_pages().await?;
                Ok(pages.into_iter().map(Feature::Page).collect())
            }
            FeatureType::Image => {
                let images = self.get_all_images().await?;
                Ok(images.into_iter().map(Feature::Image).collect())
            }
            FeatureType::Audio => {
                let audio = self.get_all_audio().await?;
                Ok(audio.into_iter().map(Feature::Audio).collect())
            }
            FeatureType::Video => {
                let videos = self.get_all_videos().await?;
                Ok(videos.into_iter().map(Feature::Video).collect())
            }
        }
    }
}