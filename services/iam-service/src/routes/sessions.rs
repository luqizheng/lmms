use axum::{Router, routing::{get, delete}, extract::{Path, State, Json}};
use crate::{state::AppState, dto::SessionInfo};
use jwt_ctx::Principal;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/sessions", get(list_sessions))
        .route("/iam/sessions/:session_id", delete(force_logout))
        .with_state(state)
}

async fn list_sessions(
    State(state): State<AppState>,
    _principal: Principal,
) -> Result<Json<Vec<SessionInfo>>, axum::http::StatusCode> {
    Ok(Json(vec![]))
}

async fn force_logout(
    State(state): State<AppState>,
    _principal: Principal,
    Path(session_id): Path<String>,
) -> Result<(), axum::http::StatusCode> {
    Ok(())
}
