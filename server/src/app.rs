use chasqui_core::config::ChasquiConfig;
use crate::services::sync::SyncService;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub sync_service: Arc<SyncService>,
    pub config: Arc<ChasquiConfig>,
}