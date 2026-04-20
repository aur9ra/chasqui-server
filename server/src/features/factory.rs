use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::assets::audio::model::AudioAsset;
use chasqui_core::features::assets::images::model::ImageAsset;
use chasqui_core::features::assets::videos::model::VideoAsset;
use chasqui_core::features::model::{Feature, FeatureType};
use chasqui_core::features::pages::model::Page;
use chasqui_core::io::ContentReader;
use crate::services::sync::manifest::Manifest;
use crate::services::sync::manifest::claim::ManifestClaim;
use crate::features::pages::service::create_page;
use crate::features::assets::images::service::create_image_asset;
use crate::features::assets::audio::service::create_audio_asset;
use crate::features::assets::videos::service::create_video_asset;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct FeatureFactory {
    pub manifest: Arc<RwLock<Manifest>>,
    pub reader: Arc<dyn ContentReader>,
    pub config: Arc<ChasquiConfig>,
}

impl FeatureFactory {
    pub fn new(
        manifest: Arc<RwLock<Manifest>>,
        reader: Arc<dyn ContentReader>,
        config: Arc<ChasquiConfig>,
    ) -> Self {
        Self {
            manifest,
            reader,
            config,
        }
    }

    pub async fn get_feature_from_file(&self, claim: ManifestClaim) -> Result<Feature> {
        let manifest_snapshot = { self.manifest.read().await.snapshot() };
        self.get_feature_from_file_with_manifest(claim, &manifest_snapshot).await
    }

    pub async fn get_feature_from_file_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<Feature> {
        match claim.feature_type {
            FeatureType::Page => Ok(Feature::Page(self.build_page_with_manifest(claim, manifest).await?)),
            FeatureType::Video => Ok(Feature::Video(self.build_video_with_manifest(claim, manifest).await?)),
            FeatureType::Audio => Ok(Feature::Audio(self.build_audio_with_manifest(claim, manifest).await?)),
            FeatureType::Image => Ok(Feature::Image(self.build_image_with_manifest(claim, manifest).await?)),
        }
    }

    async fn build_page_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<Page> {
        let full_path = claim.mount_path.join(&claim.filename);
        create_page(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_video_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<VideoAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        create_video_asset(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_audio_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<AudioAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        create_audio_asset(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_image_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<ImageAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        create_image_asset(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }
}