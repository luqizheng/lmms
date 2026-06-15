use axum::{extract::State, Json, http::StatusCode};
use crate::models::presign_url::{CreatePresignUrl, PresignUrlResponse};
use crate::routes::AppState;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/presign", axum::routing::post(create_presign))
        .with_state(state)
}

async fn create_presign(
    State(state): State<AppState>,
    Json(req): Json<CreatePresignUrl>,
) -> Result<Json<PresignUrlResponse>, (StatusCode, &'static str)> {
    let result = state.create_presign_url(req).await.map_err(|e| {
        if e.code() == "FILE_ITEM_1001" { (StatusCode::NOT_FOUND, "file not found") }
        else if e.code() == "FILE_PRESIGN_3001" { (StatusCode::UNAUTHORIZED, "presign expired") }
        else { (StatusCode::INTERNAL_SERVER_ERROR, "create presign failed") }
    })?;
    Ok(Json(result))
}
