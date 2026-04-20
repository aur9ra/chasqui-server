use chasqui_core::features::assets::audio::model::AudioAsset;
use chasqui_core::features::assets::images::model::ImageAsset;
use chasqui_core::features::assets::videos::model::VideoAsset;
use chasqui_core::features::model::{Feature, FeatureType};
use chasqui_core::features::pages::model::Page;
use crate::services::cache::SyncableCache;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub trait AsFeature: Sized {
    fn from_feature(f: Feature) -> Option<Self>;
    fn into_feature(&self) -> Feature;
    fn get_filename(&self) -> &str;
}

impl AsFeature for Page {
    fn from_feature(f: Feature) -> Option<Self> {
        match f {
            Feature::Page(p) => Some(p),
            _ => None,
        }
    }

    fn into_feature(&self) -> Feature {
        Feature::Page(self.clone())
    }

    fn get_filename(&self) -> &str {
        &self.filename
    }
}

impl AsFeature for ImageAsset {
    fn from_feature(f: Feature) -> Option<Self> {
        match f {
            Feature::Image(i) => Some(i),
            _ => None,
        }
    }

    fn into_feature(&self) -> Feature {
        Feature::Image(self.clone())
    }

    fn get_filename(&self) -> &str {
        &self.metadata.filename
    }
}

impl AsFeature for AudioAsset {
    fn from_feature(f: Feature) -> Option<Self> {
        match f {
            Feature::Audio(a) => Some(a),
            _ => None,
        }
    }

    fn into_feature(&self) -> Feature {
        Feature::Audio(self.clone())
    }

    fn get_filename(&self) -> &str {
        &self.metadata.filename
    }
}

impl AsFeature for VideoAsset {
    fn from_feature(f: Feature) -> Option<Self> {
        match f {
            Feature::Video(v) => Some(v),
            _ => None,
        }
    }

    fn into_feature(&self) -> Feature {
        Feature::Video(self.clone())
    }

    fn get_filename(&self) -> &str {
        &self.metadata.filename
    }
}

pub struct InMemoryCache<F> {
    storage: RwLock<HashMap<String, F>>,
    feature_type: FeatureType,
}

impl<F> InMemoryCache<F> {
    pub fn new(feature_type: FeatureType) -> Self {
        Self {
            storage: RwLock::new(HashMap::new()),
            feature_type,
        }
    }
}

#[async_trait]
impl<F: AsFeature + Send + Sync + Clone> SyncableCache for InMemoryCache<F> {
    async fn add(&self, feature: Feature) -> Result<()> {
        if let Some(item) = F::from_feature(feature) {
            let mut storage = self.storage.write().await;
            storage.insert(item.get_filename().to_string(), item);
        }
        Ok(())
    }

    async fn remove(&self, filename: &str) -> Result<()> {
        let mut storage = self.storage.write().await;
        storage.remove(filename);
        Ok(())
    }

    async fn get_all(&self) -> Vec<Feature> {
        let storage = self.storage.read().await;
        storage.values().map(|v| v.into_feature()).collect()
    }

    async fn get_by_key(&self, key: &str) -> Option<Feature> {
        let storage = self.storage.read().await;
        storage.get(key).map(|v| v.into_feature())
    }

    fn can_handle(&self, feature_type: FeatureType) -> bool {
        self.feature_type == feature_type
    }
}