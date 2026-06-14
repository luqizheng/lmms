use axum::{extract::State, Json, extract::Path, extract::Query, body::Body};
use crate::error::AppResult;
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::routes::AppState;
use bytes::Bytes;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/upload", axum::routing::post(upload_file))
        .route("/list", axum::routing::get(query_files))
        .route("/:id", axum::routing::get(download_file))
        .route("/:id", axum::routing::delete(delete_file))
        .route("/:id/info", axum::routing::get(get_file_info))
        .with_state(state)
}

async fn upload_file(
    State(state): State<AppState>,
    Json(meta): Json<UploadFileMeta>,
    body: Body,
) -> AppResult<Json<FileInfo>> {
    let data = axum::body::to_bytes(body, 100 * 1024 * 1024)
        .await
        .map_err(|e| crate::error::AppError::file_upload_failed(e.to_string()))?;
    
    let info = state.upload(data, meta).await?;
    Ok(Json(info))
}

async fn query_files(
    State(state): State<AppState>,
    Query(query): Query<QueryFiles>,
) -> AppResult<Json<PagedFiles>> {
    let result = state.query_files(query).await?;
    Ok(Json(result))
}

async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<axum::response::Response> {
    let data = state.download(&id).await?;
    
    let info = state.get_file_info(&id).await?;
    
    let content_type = info.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
    
    axum::response::IntoResponse::into_response((
        axum::http::header::CONTENT_TYPE,
        content_type,
        data,
    ))
}

async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    state.delete(&id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn get_file_info(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<FileInfo>> {
    let info = state.get_file_info(&id).await?;
    Ok(Json(info))
}