pub mod model;
pub mod repo;
pub mod service;

use crate::AppState;
use crate::features::model::FeatureType;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::get,
};

pub fn pages_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_pages_handler))
        .route("/{*identifier}", get(get_page_handler))
}

async fn list_pages_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<model::JsonPage>>, StatusCode> {
    let features = state.sync_service.get_all_features_by_type(FeatureType::Page).await;

    let json_pages: Vec<model::JsonPage> = features.into_iter().filter_map(|f| {
        if let crate::features::model::Feature::Page(p) = f {
            Some(model::JsonPage::from(&p))
        } else {
            None
        }
    }).collect();

    Ok(Json(json_pages))
}

async fn get_page_handler(
    State(state): State<AppState>,
    axum::extract::Path(identifier): axum::extract::Path<String>,
) -> Result<Json<model::JsonPage>, StatusCode> {
    // Note: The universal router handles path sanitization usually, 
    // but here we just look it up.
    if let Some(crate::features::model::Feature::Page(p)) = state.sync_service.get_feature_by_identifier(&identifier).await {
        Ok(Json(model::JsonPage::from(&p)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
