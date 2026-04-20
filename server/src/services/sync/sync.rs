use chasqui_core::config::ChasquiConfig;
use chasqui_core::features::model::{match_feature_to_type, Feature, FeatureType};
use chasqui_core::io::ContentReader;
use chasqui_db::SqliteRepository;
use crate::features::factory::FeatureFactory;
use crate::services::cache::models::InMemoryCache;
use crate::services::cache::SyncableCache;
use crate::services::sync::manifest::Manifest;
use chasqui_core::notifier::ContentBuildNotifier;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SyncService {
    repo: SqliteRepository,
    pub reader: Arc<dyn ContentReader>,
    pub notifier: Box<dyn ContentBuildNotifier>,
    pub config: Arc<ChasquiConfig>,
    pub manifest: Arc<RwLock<Manifest>>,
    pub factory: FeatureFactory,
    pub caches: HashMap<FeatureType, Box<dyn SyncableCache>>,
}

impl SyncService {
    pub async fn new(
        repo: SqliteRepository,
        reader: Arc<dyn ContentReader>,
        notifier: Box<dyn ContentBuildNotifier>,
        config: Arc<ChasquiConfig>,
    ) -> Result<Self> {
        print!(
            "Sync Service: Booting up universal sync engine and performing full multi-mount sync... "
        );

        let manifest = Arc::new(RwLock::new(Manifest::new()));
        let factory = FeatureFactory::new(manifest.clone(), reader.clone(), config.clone());
        let caches = Self::initialize_caches();

        let service = Self {
            repo,
            reader,
            notifier,
            config,
            manifest,
            factory,
            caches,
        };

        match service.full_sync().await {
            Ok(_) => {
                println!("Success.");
                return Ok(service);
            }

            Err(e) => {
                println!("FAILURE.");
                return Err(e);
            }
        }
    }

    fn initialize_caches() -> HashMap<FeatureType, Box<dyn SyncableCache>> {
        let mut caches: HashMap<FeatureType, Box<dyn SyncableCache>> = HashMap::new();
        caches.insert(
            FeatureType::Page,
            Box::new(InMemoryCache::<chasqui_core::features::pages::model::Page>::new(
                FeatureType::Page,
            )),
        );
        caches.insert(
            FeatureType::Image,
            Box::new(InMemoryCache::<
                chasqui_core::features::assets::images::model::ImageAsset,
            >::new(FeatureType::Image)),
        );
        caches.insert(
            FeatureType::Audio,
            Box::new(InMemoryCache::<
                chasqui_core::features::assets::audio::model::AudioAsset,
            >::new(FeatureType::Audio)),
        );
        caches.insert(
            FeatureType::Video,
            Box::new(InMemoryCache::<
                chasqui_core::features::assets::videos::model::VideoAsset,
            >::new(FeatureType::Video)),
        );
        caches
    }

    pub fn identify_mount(&self, path: &Path) -> Option<(&Path, FeatureType)> {
        let mounts = [
            (&self.config.pages_dir, FeatureType::Page),
            (&self.config.images_dir, FeatureType::Image),
            (&self.config.audio_dir, FeatureType::Audio),
            (&self.config.videos_dir, FeatureType::Video),
        ];

        mounts
            .into_iter()
            .find(|(root, f_type)| {
                path.starts_with(root) && self.is_file_matching_type(path, *f_type)
            })
            .map(|(root, f_type)| (root.as_path(), f_type))
    }

    pub fn is_file_matching_type(&self, path: &Path, f_type: FeatureType) -> bool {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        match f_type {
            FeatureType::Page => ext == "md",
            FeatureType::Video => {
                matches!(ext.as_str(), "mp4" | "mov" | "webm" | "mkv" | "ogv" | "avi")
            }
            FeatureType::Audio => matches!(
                ext.as_str(),
                "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "opus"
            ),
            FeatureType::Image => matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "heic" | "svg" | "ico" | "tiff"
            ),
        }
    }

    pub async fn notify_build(&self) -> Result<()> {
        self.notifier.notify().await
    }

    pub async fn full_sync(&self) -> Result<()> {
        let mut all_entries = Vec::new();

        let mounts = [
            (&self.config.pages_dir, FeatureType::Page),
            (&self.config.images_dir, FeatureType::Image),
            (&self.config.audio_dir, FeatureType::Audio),
            (&self.config.videos_dir, FeatureType::Video),
        ];

        for (mount, f_type) in mounts {
            if let Ok(entries) = self.reader.list_all_files(mount).await {
                for e in entries {
                    if self.is_file_matching_type(&e, f_type) {
                        all_entries.push((e, (*mount).clone(), f_type));
                    }
                }
            }
        }

        self.process_batch(all_entries, Vec::new()).await
    }

    pub async fn process_batch(
        &self,
        changes: Vec<(std::path::PathBuf, std::path::PathBuf, FeatureType)>,
        deletions: Vec<std::path::PathBuf>,
    ) -> Result<()> {
        for path in deletions {
            self.handle_deletion(&path).await?;
        }

        let (valid_claims, manifest_snapshot) = {
            let mut manifest_guard = self.manifest.write().await;
            let claims = manifest_guard
                .register_claims(changes, &*self.reader, &self.config)
                .await;

            (claims, manifest_guard.snapshot())
        };

        for claim in valid_claims {
            match self
                .factory
                .get_feature_from_file_with_manifest(claim.clone(), &manifest_snapshot)
                .await
            {
                Ok(feature) => {
                    if let Err(e) = self.repo.save_feature(feature.clone()).await {
                        eprintln!("Sync Service: Failed to save feature to repository: {}. Rolling back manifest claim.", e);
                        let mut manifest_guard = self.manifest.write().await;
                        manifest_guard.remove_by_filename(&claim.filename);
                        return Err(e);
                    }
                    self.update_cache(feature).await?;
                }
                Err(e) => {
                    eprintln!("Sync Service: Failed to produce feature: {}", e);
                    let mut manifest_guard = self.manifest.write().await;
                    manifest_guard.remove_by_filename(&claim.filename);
                }
            }
        }

        Ok(())
    }

    async fn handle_deletion(&self, path: &Path) -> Result<()> {
        let filename = if let Some((mount_root, _)) = self.identify_mount(path) {
            path.strip_prefix(mount_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace("\\", "/")
        } else {
            path.to_string_lossy().replace("\\", "/")
        };

        let mut manifest_guard = self.manifest.write().await;
        if let Some(f_type) = manifest_guard.feature_types.get(&filename).cloned() {
            self.repo.delete_feature(&filename, f_type).await?;
            if let Some(cache) = self.caches.get(&f_type) {
                cache.remove(&filename).await?;
            }
        }

        manifest_guard.remove_by_filename(&filename);
        println!("Successfully deleted {}", filename);
        Ok(())
    }

    async fn update_cache(&self, feature: Feature) -> Result<()> {
        let f_type = match_feature_to_type(&feature);
        if let Some(cache) = self.caches.get(&f_type) {
            cache.add(feature).await?;
        }
        Ok(())
    }

    pub async fn get_all_features_by_type(&self, f_type: FeatureType) -> Vec<Feature> {
        if let Some(cache) = self.caches.get(&f_type) {
            return cache.get_all().await;
        }
        Vec::new()
    }

    pub async fn get_feature_by_identifier(&self, identifier: &str) -> Option<Feature> {
        let manifest_guard = self.manifest.read().await;
        let filename = manifest_guard.id_to_file.get(identifier)?;
        let f_type = manifest_guard.feature_types.get(filename)?;

        if let Some(cache) = self.caches.get(f_type) {
            return cache.get_by_key(filename).await;
        }
        None
    }
}