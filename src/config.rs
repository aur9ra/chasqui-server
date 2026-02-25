use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ChasquiConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub frontend_path: PathBuf,
    pub content_dir: PathBuf,
    pub strip_extensions: bool,
    pub webhook_url: String,
    pub webhook_secret: String,
}

impl ChasquiConfig {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL")
            .expect("Failed to determine DATABASE_URL from environment variables");

        let max_connections = std::env::var("MAX_CONNECTIONS")
            .ok()
            .and_then(|val| val.parse::<u32>().ok())
            .unwrap_or(15);

        let frontend_path = PathBuf::from(
            std::env::var("FRONTEND_DIST_PATH")
                .expect("Failed to determine FRONTEND_DIST_PATH from environment variables"),
        );

        let content_dir = std::fs::canonicalize(
            std::env::var("CONTENT_DIR").unwrap_or_else(|_| "./content/md".to_string()),
        )
        .expect("Failed to resolve CONTENT_DIR to an absolute path. Does the directory exist?");

        let strip_extensions = std::env::var("DEFAULT_IDENTIFIER_STRIP_EXTENSION")
            .unwrap_or_else(|_| "false".to_string())
            == "true";

        let webhook_url = std::env::var("FRONTEND_WEBHOOK_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:4000/build".to_string());

        let webhook_secret = std::env::var("WEBHOOK_SECRET").unwrap_or_default();

        Self {
            database_url,
            max_connections,
            frontend_path,
            content_dir,
            strip_extensions,
            webhook_url,
            webhook_secret,
        }
    }
}
