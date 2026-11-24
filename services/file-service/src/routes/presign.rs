use crate::models::presign_url::{CreatePresignUrl, PresignUrlResponse};
use crate::routes::AppState;
use axum::{extract::State, http::StatusCode, Json};
use digua_jwt_ctx::TenantScope;

pub fn routes(state: AppState) -> axum::Router<AppState> {
    axum::Router::new()
        .route("/presign", axum::routing::post(create_presign))
        .with_state(state)
}

#[utoipa::path(
    post,
    path = "/file/presign",
    summary = "Create presigned URL",
    request_body = CreatePresignUrl,
    responses(
        (status = 200, description = "Presigned URL", body = PresignUrlResponse),
        (status = 401, description = "Presign expired"),
        (status = 400, description = "Invalid presign TTL"),
        (status = 404, description = "File not found"),
        (status = 501, description = "Storage backend does not support presigned URLs"),
    ),
    tag = "presign"
)]
// check-tenant-scope: delegated (TenantScope applied by service::create_presign_url via get_file_info)
pub async fn create_presign(
    State(state): State<AppState>,
    scope: TenantScope,
    Json(req): Json<CreatePresignUrl>,
) -> Result<Json<PresignUrlResponse>, (StatusCode, &'static str)> {
    let result = state.create_presign_url(req, &scope).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" {
            (StatusCode::NOT_FOUND, "file not found")
        } else if e.code() == "FILE_PRESIGN_3001" {
            (StatusCode::UNAUTHORIZED, "presign expired")
        } else if e.code() == "FILE_PRESIGN_3002" {
            (StatusCode::BAD_REQUEST, "invalid presign TTL")
        } else if e.code() == "FILE_PRESIGN_3003" {
            (
                StatusCode::NOT_IMPLEMENTED,
                "storage backend does not support presigned URLs",
            )
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "create presign failed")
        }
    })?;
    Ok(Json(result))
}
