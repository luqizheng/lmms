use axum::{extract::State, extract::Query, http::StatusCode};
use crate::routes::AppState;
use serde::Deserialize;
use std::time::Duration;
use utoipa::IntoParams;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/watch", axum::routing::get(watch_config))
        .with_state(state)
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct WatchParams {
    pub namespace_id: Option<String>,
    pub timeout: Option<u64>,
}

#[utoipa::path(
    get,
    path = "/config/watch",
    params(WatchParams),
    responses(
        (status = 200, description = "Long-poll result", body = serde_json::Value),
    ),
    tag = "watch"
)]
pub async fn watch_config(
    State(_state): State<AppState>,
    Query(params): Query<WatchParams>,
) -> Result<axum::response::Json<serde_json::Value>, (StatusCode, &'static str)> {
    let namespace_id = params.namespace_id.unwrap_or_default();
    let timeout = params.timeout.unwrap_or(30);
    
    tokio::time::sleep(Duration::from_secs(timeout)).await;
    
    Ok(axum::response::Json(serde_json::json!({
        "namespace_id": namespace_id,
        "changed": false,
        "data": []
    })))
}
