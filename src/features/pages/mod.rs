pub mod model;
pub mod repo;

use crate::AppState;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};

pub fn pages_router() -> Router<AppState> {
    Router::new()
        .route("/{slug}", get(get_page_handler))
        .route("/", get(list_pages_handler))
}

async fn get_page_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<model::JsonPage>, StatusCode> {
    let page_option = state.sync_service.get_page_by_identifier(&slug).await;

    match page_option {
        None => Err(StatusCode::NOT_FOUND),
        Some(page) => {
            let json_page: model::JsonPage = (&page).into();
            Ok(Json(json_page))
        }
    }
}

async fn list_pages_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<model::JsonPage>>, StatusCode> {
    let db_pages = state.sync_service.get_all_pages().await;

    let json_pages: Vec<model::JsonPage> = db_pages.iter().map(|p| p.into()).collect();

    Ok(Json(json_pages))
}
