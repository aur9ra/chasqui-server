use chasqui_core::config::ChasquiConfig;
use chasqui_db::SqliteRepository;
use chasqui_server::services::sync::SyncService;
use chasqui_server::testutil::{MockBuildNotifier, MockContentReader};
use std::path::PathBuf;
use std::sync::Arc;

pub fn mock_config(temp_path: PathBuf) -> Arc<ChasquiConfig> {
    Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        pages_dir: temp_path.join("md"),
        images_dir: temp_path.join("images"),
        audio_dir: temp_path.join("audio"),
        videos_dir: temp_path.join("videos"),
        page_strip_extension: true,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "http://localhost/build".into(),
        webhook_secret: "secret".into(),
        port: 3000,
        nginx_media_prefixes: false,
    })
}

pub struct TestOptions {
    pub pages_dir: PathBuf,
    pub images_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub videos_dir: PathBuf,
}

impl Default for TestOptions {
    fn default() -> Self {
        let content_dir = PathBuf::from("/content");
        Self {
            pages_dir: content_dir.clone(),
            images_dir: content_dir.clone(),
            audio_dir: content_dir.clone(),
            videos_dir: content_dir,
        }
    }
}

pub async fn setup_service_with_options(opts: TestOptions) -> (
    Arc<SyncService>,
    MockContentReader,
    MockBuildNotifier,
    Arc<ChasquiConfig>,
    SqliteRepository,
) {
    let repo = chasqui_db::testutil::create_test_repository().await;
    let reader = MockContentReader::new();
    let notifier = MockBuildNotifier::new();
    let config = Arc::new(ChasquiConfig {
        database_url: "".into(),
        max_connections: 1,
        pages_dir: opts.pages_dir,
        images_dir: opts.images_dir,
        audio_dir: opts.audio_dir,
        videos_dir: opts.videos_dir,
        page_strip_extension: true,
        asset_strip_extension: false,
        serve_home: true,
        home_identifier: "index".into(),
        webhook_url: "".into(),
        webhook_secret: "".into(),
        port: 3000,
        nginx_media_prefixes: false,
    });

    let service = SyncService::new(
        repo.clone(),
        Arc::new(reader.clone()),
        Box::new(notifier.clone()),
        config.clone(),
    )
    .await
    .unwrap();

    (Arc::new(service), reader, notifier, config, repo)
}

pub async fn setup_service() -> (
    Arc<SyncService>,
    MockContentReader,
    MockBuildNotifier,
    Arc<ChasquiConfig>,
    SqliteRepository,
) {
    setup_service_with_options(TestOptions::default()).await
}