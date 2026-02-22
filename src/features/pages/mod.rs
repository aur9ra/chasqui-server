pub mod model;
pub mod repo;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use model::{DbPage, JsonPage};
use sqlx::{Pool, Sqlite};
use std::env::var;

pub fn pages_router() -> Router<Pool<Sqlite>> {
    Router::new()
        .route("/pages/{slug}", get(get_page_handler))
        .route("/pages", get(list_pages_handler))
}

async fn get_page_handler(
    State(pool): State<Pool<Sqlite>>,
    Path(slug): Path<String>,
) -> Result<Json<model::JsonPage>, StatusCode> {
    let page_option = repo::get_entry_by_identifier(&slug, &pool).await;

    match page_option {
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),

        Ok(None) => Err(StatusCode::NOT_FOUND),

        Ok(Some(page)) => Ok(Json(db_page_to_json_page(&page, "%Y-%m-%d %H:%M:%S"))), // Ok(Json(page))
    }
}

async fn list_pages_handler(
    State(pool): State<Pool<Sqlite>>,
) -> Result<Json<Vec<model::JsonPage>>, StatusCode> {
    let db_pages = repo::get_pages_from_db(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let json_pages: Vec<model::JsonPage> = db_pages
        .into_iter()
        .map(|p| db_page_to_json_page(&p, "%Y-%m-%d %H:%M:%S"))
        .collect();

    Ok(Json(json_pages))
}

fn db_page_to_json_page(dbpage: &DbPage, format: &str) -> JsonPage {
    let modified_datetime: Option<String> = match dbpage.modified_datetime {
        Some(val) => Some(val.format(format).to_string()),
        None => None,
    };
    let created_datetime: Option<String> = match dbpage.created_datetime {
        Some(val) => Some(val.format(format).to_string()),
        None => None,
    };

    JsonPage {
        identifier: dbpage.identifier.to_owned(),
        filename: dbpage.filename.to_owned(),
        name: dbpage.name.to_owned(),
        html_content: dbpage.html_content.to_owned(),
        md_content: dbpage.md_content.to_owned(),
        md_content_hash: dbpage.md_content_hash.to_owned(),
        tags: dbpage.tags.to_owned(),
        modified_datetime: modified_datetime,
        created_datetime: created_datetime,
    }
}
