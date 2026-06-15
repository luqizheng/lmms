use axum::{extract::State, extract::Query, http::StatusCode};
use crate::routes::AppState;
use std::time::Duration;

pub fn routes(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/watch", axum::routing::get(watch_config))
        .with_state(state)
}

async fn watch_config(
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

#[derive(Debug, serde::Deserialize)]
struct WatchParams {
    namespace_id: Option<String>,
    timeout: Option<u64>,
}
