pub mod service;

use chasqui_core::features::model::FeatureType;
use axum::{Json, Router, extract::State, routing::get, http::StatusCode};
use crate::app::AppState;
use chasqui_core::features::pages::model::JsonPage;

pub fn pages_router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_pages_handler))
        .route("/{*identifier}", get(get_page_handler))
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

async fn get_page_handler(
    State(state): State<AppState>,
    axum::extract::Path(identifier): axum::extract::Path<String>,
) -> Result<Json<JsonPage>, StatusCode> {
    if let Some(chasqui_core::features::model::Feature::Page(p)) =
        state.sync_service.get_feature_by_identifier(&identifier).await
    {
        Ok(Json((&p).into()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}