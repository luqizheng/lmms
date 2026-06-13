use axum::{Router, routing::{get, delete}, extract::{Path, State}};
use crate::{state::AppState, dto::SessionInfo};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/sessions", get(list_sessions))
        .route("/iam/sessions/:session_id", delete(force_logout))
        .with_state(state)
}

async fn list_sessions(
    State(_state): State<AppState>,
    _principal: Principal,
) -> Result<axum::Json<Vec<SessionInfo>>, (axum::http::StatusCode, &'static str)> {
    Ok(axum::Json(vec![]))
}

async fn force_logout(
    State(_state): State<AppState>,
    _principal: Principal,
    Path(_session_id): Path<String>,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, &'static str)> {
    Ok(axum::http::StatusCode::NO_CONTENT)
}