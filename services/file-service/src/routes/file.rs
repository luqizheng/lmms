use axum::{extract::State, Json, extract::Path, extract::Query, http::StatusCode};
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
) -> Result<Json<FileInfo>, (StatusCode, &'static str)> {
    let data = Bytes::new();
    let info = state.upload(data, meta).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "upload failed"))?;
    Ok(Json(info))
}

async fn query_files(
    State(state): State<AppState>,
    Query(query): Query<QueryFiles>,
) -> Result<Json<PagedFiles>, (StatusCode, &'static str)> {
    let result = state.query_files(query).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "query failed"))?;
    Ok(Json(result))
}

async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(axum::http::HeaderMap, Vec<u8>), (StatusCode, &'static str)> {
    let data = state.download(&id).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "download failed") }
    })?;
    
    let info = state.get_file_info(&id).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get file info failed") }
    })?;
    
    let content_type = info.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
    
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, content_type.parse().unwrap());
    
    Ok((headers, data.to_vec()))
}

async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    state.delete(&id).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "delete failed") }
    })?;
    Ok(Json(serde_json::json!({ "success": true })))
}

async fn get_file_info(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FileInfo>, (StatusCode, &'static str)> {
    let info = state.get_file_info(&id).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get file info failed") }
    })?;
    Ok(Json(info))
}
