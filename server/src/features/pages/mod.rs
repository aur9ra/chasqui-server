pub mod service;

use chasqui_core::features::model::FeatureType;
use axum::{Json, Router, extract::State, routing::get};
use crate::app::AppState;
use chasqui_core::features::pages::model::JsonPage;

pub fn pages_router() -> Router<AppState> {
    Router::new().route("/", get(list_pages_handler))
}

async fn list_pages_handler(State(state): State<AppState>) -> Json<Vec<JsonPage>> {
    let features = state.sync_service.get_all_features_by_type(FeatureType::Page).await;
    let pages: Vec<JsonPage> = features
        .into_iter()
        .filter_map(|f| {
            if let chasqui_core::features::model::Feature::Page(p) = f {
                Some((&p).into())
            } else {
                None
            }
        })
        .collect();
    Json(pages)
}