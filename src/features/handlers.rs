use crate::AppState;
use crate::features::model::{Feature, JsonFeature};
use crate::features::routing::{path_to_identifier, get_identifier_variants};
use axum::{
    body::Body,
    extract::{State, Request},
    http::{Response, StatusCode},
    Json,
};
use tower_http::services::ServeDir;
use tower::ServiceExt;

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

pub async fn universal_dispatch_handler(
    State(state): State<AppState>,
    req: Request,
) -> Result<Response<Body>, StatusCode> {
    let (parts, _body) = req.into_parts();
    let uri_path = parts.uri.path().to_string();
    
    // 1. Primary Priority: Serve directly from the frontend dist directory (Astro Output)
    let serve_dir = ServeDir::new(&state.config.frontend_path).append_index_html_on_directories(true);
    
    // Reconstruct request for ServeDir
    let sd_req = Request::from_parts(parts.clone(), Body::empty());
    let res = serve_dir.oneshot(sd_req).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If the file was found in dist, return it immediately.
    if res.status() != StatusCode::NOT_FOUND {
        return Ok(res.map(Body::new));
    }

    // 2. Secondary Priority: Consult the Feature Registry for assets in content_dir
    let base_id = path_to_identifier(&state.config, &uri_path);
    let variants = get_identifier_variants(&base_id);

    for id in variants {
        if let Some(f) = state.sync_service.get_feature_by_identifier(&id).await {
            match f {
                Feature::Page(_) => {
                    // Pages MUST be served from dist. If we're here, it's missing.
                    return Err(StatusCode::NOT_FOUND);
                }
                Feature::Image(i) => return serve_physical_file(i.metadata.new_path.as_ref().unwrap_or(&i.metadata.file_path)).await,
                Feature::Audio(a) => return serve_physical_file(a.metadata.new_path.as_ref().unwrap_or(&a.metadata.file_path)).await,
                Feature::Video(v) => return serve_physical_file(v.metadata.new_path.as_ref().unwrap_or(&v.metadata.file_path)).await,
            }
        }
    }

    // 3. Final Fallback: The original 404 from ServeDir
    Ok(res.map(Body::new))
}

async fn serve_physical_file(path: &std::path::Path) -> Result<Response<Body>, StatusCode> {
    if path.exists() {
        let serve_file = tower_http::services::ServeFile::new(path);
        let req = http::Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = serve_file.oneshot(req).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(res.map(Body::new))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
