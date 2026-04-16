use crate::config::ChasquiConfig;
use crate::features::assets::audio::model::AudioAsset;
use crate::features::assets::images::model::ImageAsset;
use crate::features::assets::videos::model::VideoAsset;
use crate::features::pages::model::Page;
use crate::io::ContentReader;
use crate::services::sync::manifest::Manifest;
use crate::services::sync::manifest::claim::ManifestClaim;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feature {
    Page(Page),
    Video(VideoAsset),
    Audio(AudioAsset),
    Image(ImageAsset),
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
pub enum JsonFeature {
    Page(crate::features::pages::model::JsonPage),
    Video(VideoAsset),
    Audio(AudioAsset),
    Image(ImageAsset),
}

impl From<Feature> for JsonFeature {
    fn from(f: Feature) -> Self {
        match f {
            Feature::Page(p) => JsonFeature::Page((&p).into()),
            Feature::Video(v) => JsonFeature::Video(v),
            Feature::Audio(a) => JsonFeature::Audio(a),
            Feature::Image(i) => JsonFeature::Image(i),
        }
    }
}

pub fn match_feature_to_type(f: &Feature) -> FeatureType {
    match f {
        Feature::Page(_) => FeatureType::Page,
        Feature::Video(_) => FeatureType::Video,
        Feature::Audio(_) => FeatureType::Audio,
        Feature::Image(_) => FeatureType::Image,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureType {
    Page,
    Video,
    Audio,
    Image,
}

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

    /// Orchestrates the production of a full Feature from a validated ManifestClaim.
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
        Page::new_from_file(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_video_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<VideoAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        VideoAsset::new_from_file(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_audio_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<AudioAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        AudioAsset::new_from_file(
            &full_path,
            &self.config,
            &*self.reader,
            manifest,
        )
        .await
    }

    async fn build_image_with_manifest(&self, claim: ManifestClaim, manifest: &Manifest) -> Result<ImageAsset> {
        let full_path = claim.mount_path.join(&claim.filename);
        ImageAsset::new_from_file(
&full_path,
             &self.config,
             &*self.reader,
             manifest,
         )
         .await
     }
}
