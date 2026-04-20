use axum::extract::{State, Path};
use axum::http::StatusCode;
use axum::Json;
use chasqui_core::features::model::JsonFeature;
use crate::app::AppState;
use crate::features::routing::{path_to_identifier, get_identifier_variants};

pub async fn metadata_handler(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Json<JsonFeature>, StatusCode> {
    let identifier = path_to_identifier(&state.config, &path);
    let variants = get_identifier_variants(&identifier);

    for variant in variants {
        if let Some(feature) = state.sync_service.get_feature_by_identifier(&variant).await {
            return Ok(Json(JsonFeature::from(feature)));
        }
    }

    Err(StatusCode::NOT_FOUND)
}