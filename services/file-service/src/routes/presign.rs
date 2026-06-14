use axum::{extract::State, Json, extract::Path};
use crate::error::AppResult;
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
) -> AppResult<Json<PresignUrlResponse>> {
    let result = state.create_presign_url(req).await?;
    Ok(Json(result))
}