use crate::database::SyncRepository;
use crate::features::assets::audio::repo::AudioRepository;
use crate::features::assets::images::repo::ImageRepository;
use crate::features::assets::videos::repo::VideoRepository;
use crate::features::model::{Feature, FeatureType};
use crate::features::pages::repo::PageRepository;
use anyhow::Result;
use async_trait::async_trait;
use sqlx::{Pool, Sqlite};

pub struct SqliteRepository {
    pub pool: Pool<Sqlite>,
}

impl SqliteRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SyncRepository for SqliteRepository {
    async fn save_feature(&self, feature: Feature) -> Result<()> {
        match feature {
            Feature::Page(page) => self.save_page(&page).await,
            Feature::Image(img) => self.save_image(&img).await,
            Feature::Audio(aud) => self.save_audio(&aud).await,
            Feature::Video(vid) => self.save_video(&vid).await,
        }
    }

    async fn get_feature(&self, filename: &str, feature_type: FeatureType) -> Result<Option<Feature>> {
        match feature_type {
            FeatureType::Page => Ok(self.get_page_by_filename(filename).await?.map(Feature::Page)),
            FeatureType::Image => Ok(self.get_image_by_filename(filename).await?.map(Feature::Image)),
            FeatureType::Audio => Ok(self.get_audio_by_filename(filename).await?.map(Feature::Audio)),
            FeatureType::Video => Ok(self.get_video_by_filename(filename).await?.map(Feature::Video)),
        }
    }

    async fn update_feature(&self, feature: Feature) -> Result<()> {
        self.save_feature(feature).await
    }

    async fn delete_feature(&self, filename: &str, feature_type: FeatureType) -> Result<()> {
        match feature_type {
            FeatureType::Page => self.delete_page(filename).await,
            FeatureType::Image => self.delete_image(filename).await,
            FeatureType::Audio => self.delete_audio(filename).await,
            FeatureType::Video => self.delete_video(filename).await,
        }
    }

    async fn get_all_features(&self, feature_type: FeatureType) -> Result<Vec<Feature>> {
        match feature_type {
            FeatureType::Page => {
                let pages = <Self as PageRepository>::get_all_pages(self).await?;
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
