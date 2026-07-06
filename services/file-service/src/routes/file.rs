use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::models::file_info::{FileInfo, PagedFiles, QueryFiles, UploadFileMeta};
use crate::routes::AppState;
use bytes::Bytes;
use jwt_ctx::TenantScope;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/upload", axum::routing::post(upload_file))
        .route("/list", axum::routing::get(query_files))
        .route("/{id}", axum::routing::get(download_file))
        .route("/{id}", axum::routing::delete(delete_file))
        .route("/{id}/info", axum::routing::get(get_file_info))
        .with_state(state)
}

/// Multipart upload:
///   - `metadata` 字段：JSON 字符串，类型 UploadFileMeta
///   - `file` 字段：原始字节（大小受 services/file-service/src/config.rs:max_file_size 限制）
#[utoipa::path(
    post,
    path = "/file/upload",
    request_body(content = String, content_type = "multipart/form-data",
        description = "multipart: 'metadata' (json UploadFileMeta) + 'file' (bytes)"),
    responses(
        (status = 200, description = "Uploaded file metadata", body = FileInfo),
        (status = 413, description = "Payload too large"),
        (status = 400, description = "Invalid multipart body or metadata"),
    ),
    tag = "file"
)]
// check-tenant-scope: delegated (tenant_id from scope applied by service::upload)
pub async fn upload_file(
    State(state): State<AppState>,
    scope: TenantScope,
    mut multipart: Multipart,
) -> Result<Json<FileInfo>, (StatusCode, &'static str)> {
    let mut meta: Option<UploadFileMeta> = None;
    let mut data: Option<Bytes> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid multipart body"))?
    {
        match field.name() {
            Some("metadata") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| (StatusCode::BAD_REQUEST, "metadata read failed"))?;
                meta = Some(
                    serde_json::from_str::<UploadFileMeta>(&text)
                        .map_err(|_| (StatusCode::BAD_REQUEST, "invalid metadata json"))?,
                );
            }
            Some("file") => {
                data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| (StatusCode::BAD_REQUEST, "file read failed"))?,
                );
            }
            _ => {
                // 忽略其他字段
            }
        }
    }

    let meta = meta.ok_or((StatusCode::BAD_REQUEST, "missing metadata field"))?;
    let data = data.ok_or((StatusCode::BAD_REQUEST, "missing file field"))?;

    let info = state.upload(data, meta, &scope).await.map_err(|e| {
        if e.code() == "FILE_UPLOAD_2002" {
            (StatusCode::PAYLOAD_TOO_LARGE, "file too large")
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "upload failed")
        }
    })?;
    Ok(Json(info))
}

#[utoipa::path(
    get,
    path = "/file/list",
    params(QueryFiles),
    responses(
        (status = 200, description = "Paged file list", body = PagedFiles),
    ),
    tag = "file"
)]
// check-tenant-scope: delegated (TenantScope applied by service::query_files)
pub async fn query_files(
    State(state): State<AppState>,
    Query(query): Query<QueryFiles>,
    scope: TenantScope,
) -> Result<Json<PagedFiles>, (StatusCode, &'static str)> {
    let result = state.query_files(query, &scope).await.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "query failed"))?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/file/{id}",
    params(("id" = String, Path, description = "File id")),
    responses(
        (status = 200, description = "File bytes (octet-stream or content-type)", body = String),
        (status = 404, description = "Not found"),
    ),
    tag = "file"
)]
// check-tenant-scope: delegated (TenantScope applied by service::download)
pub async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    scope: TenantScope,
) -> Result<(axum::http::HeaderMap, Vec<u8>), (StatusCode, &'static str)> {
    let data = state.download(&id, &scope).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "download failed") }
    })?;

    let info = state.get_file_info(&id, &scope).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get file info failed") }
    })?;

    let content_type = info.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, content_type.parse().unwrap());

    Ok((headers, data.to_vec()))
}

#[utoipa::path(
    delete,
    path = "/file/{id}",
    params(("id" = String, Path, description = "File id")),
    responses(
        (status = 200, description = "Deleted", body = serde_json::Value),
        (status = 404, description = "Not found"),
    ),
    tag = "file"
)]
// check-tenant-scope: delegated (TenantScope applied by service::delete)
pub async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    scope: TenantScope,
) -> Result<Json<serde_json::Value>, (StatusCode, &'static str)> {
    state.delete(&id, &scope).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "delete failed") }
    })?;
    Ok(Json(serde_json::json!({ "success": true })))
}

#[utoipa::path(
    get,
    path = "/file/{id}/info",
    params(("id" = String, Path, description = "File id")),
    responses(
        (status = 200, description = "File info", body = FileInfo),
        (status = 404, description = "Not found"),
    ),
    tag = "file"
)]
// check-tenant-scope: delegated (TenantScope applied by service::get_file_info)
pub async fn get_file_info(
    State(state): State<AppState>,
    Path(id): Path<String>,
    scope: TenantScope,
) -> Result<Json<FileInfo>, (StatusCode, &'static str)> {
    let info = state.get_file_info(&id, &scope).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") } else { (StatusCode::INTERNAL_SERVER_ERROR, "get file info failed") }
    })?;
    Ok(Json(info))
}