use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ChasquiConfig {
    pub database_url: String,
    pub max_connections: u32,

    // Mount Points
    pub pages_dir: PathBuf,
    pub images_dir: PathBuf,
    pub audio_dir: PathBuf,
    pub videos_dir: PathBuf,

    pub page_strip_extension: bool,
    pub asset_strip_extension: bool,
    pub serve_home: bool,
    pub home_identifier: String,
    pub webhook_url: String,
    pub webhook_secret: String,
    pub port: u16,
    pub nginx_media_prefixes: bool,
}

impl ChasquiConfig {
    pub fn from_env() -> Self {
        // Add the relevant functionality to provide fields such as PORT, etc.
        let database_url = std::env::var("DATABASE_URL")
            .expect("Failed to determine DATABASE_URL from environment variables");

        let max_connections = std::env::var("MAX_CONNECTIONS")
            .ok()
            .and_then(|val| val.parse::<u32>().ok())
            .unwrap_or(15);

        // Content Dir remains as a root, but we default mount points under it
        let content_root = std::env::var("CONTENT_DIR").unwrap_or_else(|_| "./content".to_string());

        let pages_dir = resolve_dir("PAGES_DIR", &format!("{}/md", content_root));
        let images_dir = resolve_dir("IMAGES_DIR", &format!("{}/images", content_root));
        let audio_dir = resolve_dir("AUDIO_DIR", &format!("{}/audio", content_root));
        let videos_dir = resolve_dir("VIDEOS_DIR", &format!("{}/videos", content_root));

        let page_strip_extension = std::env::var("DEFAULT_PAGE_IDENTIFIER_STRIP_EXTENSION")
            .unwrap_or_else(|_| "true".to_string())
            == "true";

        let asset_strip_extension = std::env::var("DEFAULT_ASSET_IDENTIFIER_STRIP_EXTENSION")
            .unwrap_or_else(|_| "false".to_string())
            == "true";

        let serve_home = std::env::var("ROUTER_SERVE_HOME_AT_DEFAULT")
            .unwrap_or_else(|_| "true".to_string())
            == "true";

        let home_identifier =
            std::env::var("HOME_IDENTIFIER").unwrap_or_else(|_| "index".to_string());

        let webhook_url = std::env::var("FRONTEND_WEBHOOK_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:4000/build".to_string());

        let webhook_secret = std::env::var("WEBHOOK_SECRET").unwrap_or_default();

        let port = std::env::var("PORT")
            .ok()
            .and_then(|val| val.parse::<u16>().ok())
            .unwrap_or(3000);

        let nginx_media_prefixes =
            std::env::var("NGINX_MEDIA_PREFIXES").unwrap_or_else(|_| "true".to_string()) == "true";

        Self {
            database_url,
            max_connections,
            pages_dir,
            images_dir,
            audio_dir,
            videos_dir,
            page_strip_extension,
            asset_strip_extension,
            serve_home,
            home_identifier,
            webhook_url,
            webhook_secret,
            port,
            nginx_media_prefixes,
        }
    }
}

fn resolve_dir(env_var: &str, default: &str) -> PathBuf {
    let path_str = std::env::var(env_var).unwrap_or_else(|_| default.to_string());
    std::fs::canonicalize(&path_str).unwrap_or_else(|_| PathBuf::from(path_str))
    // Fallback if folder doesn't exist yet
}
