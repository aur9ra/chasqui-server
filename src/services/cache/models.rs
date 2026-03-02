use crate::features::assets::audio::model::AudioAsset;
use crate::features::assets::images::model::ImageAsset;
use crate::features::assets::videos::model::VideoAsset;
use crate::features::model::{Feature, FeatureType};
use crate::features::pages::model::Page;
use crate::services::cache::SyncableCache;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// A trait that allows a feature struct to be extracted from and wrapped into the universal Feature enum.
pub trait AsFeature: Sized {
    fn from_feature(f: Feature) -> Option<Self>;
    fn into_feature(&self) -> Feature;
    fn get_filename(&self) -> &str;
}

impl AsFeature for Page {
    fn from_feature(f: Feature) -> Option<Self> { if let Feature::Page(p) = f { Some(p) } else { None } }
    fn into_feature(&self) -> Feature { Feature::Page(self.clone()) }
    fn get_filename(&self) -> &str { &self.filename }
}

impl AsFeature for ImageAsset {
    fn from_feature(f: Feature) -> Option<Self> { if let Feature::Image(img) = f { Some(img) } else { None } }
    fn into_feature(&self) -> Feature { Feature::Image(self.clone()) }
    fn get_filename(&self) -> &str { &self.metadata.filename }
}

impl AsFeature for AudioAsset {
    fn from_feature(f: Feature) -> Option<Self> { if let Feature::Audio(aud) = f { Some(aud) } else { None } }
    fn into_feature(&self) -> Feature { Feature::Audio(self.clone()) }
    fn get_filename(&self) -> &str { &self.metadata.filename }
}

impl AsFeature for VideoAsset {
    fn from_feature(f: Feature) -> Option<Self> { if let Feature::Video(vid) = f { Some(vid) } else { None } }
    fn into_feature(&self) -> Feature { Feature::Video(self.clone()) }
    fn get_filename(&self) -> &str { &self.metadata.filename }
}

/// A simple, thread-safe in-memory store for a specific feature type.
pub struct InMemoryCache<F> {
    pub storage: RwLock<HashMap<String, F>>,
    pub feature_type: FeatureType,
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
            self.storage.write().await.insert(item.get_filename().to_string(), item);
        }
        Ok(())
    }

    async fn remove(&self, filename: &str) -> Result<()> {
        self.storage.write().await.remove(filename);
        Ok(())
    }

    async fn get_all(&self) -> Vec<Feature> {
        let storage = self.storage.read().await;
        storage.values().map(|item| item.into_feature()).collect()
    }

    async fn get_by_key(&self, key: &str) -> Option<Feature> {
        let storage = self.storage.read().await;
        storage.get(key).map(|item| item.into_feature())
    }

    fn can_handle(&self, feature_type: FeatureType) -> bool {
        self.feature_type == feature_type
    }
}

/*
pub struct MemoryRespectingCache<F> {
    // TODO LATER
    // later I will implement a RAMRespectingCache to keep a cache's memory foothold behind a
    // threshold by moving/adding the pointer of a resource to the head of the (sortable???) cache upon request,
    // and removing assets from the cache that have been "requested" the longest time ago
    // additionally we will have the frontend specify that its requests should not impact this
    // cache.
}

pub struct NoCache<F> {}
*/
