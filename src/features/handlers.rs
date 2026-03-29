use crate::AppState;
use crate::features::model::JsonFeature;
use crate::features::routing::{path_to_identifier, get_identifier_variants};
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};

pub async fn metadata_handler(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Result<Json<JsonFeature>, StatusCode> {
    let base_id = path_to_identifier(&state.config, &path);
    let variants = get_identifier_variants(&base_id);

    for id in variants {
        if let Some(f) = state.sync_service.get_feature_by_identifier(&id).await {
            return Ok(Json(JsonFeature::from(f)));
        }
    }

    Err(StatusCode::NOT_FOUND)
}