use crate::routes::AppState;
use axum::{extract::Query, extract::State, http::StatusCode};
use digua_jwt_ctx::TenantScope;
use serde::Deserialize;
use std::time::Duration;
use utoipa::IntoParams;

pub fn routes(state: AppState) -> axum::Router<AppState> {
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
    summary = "Watch config changes (long-poll)",
    params(WatchParams),
    responses(
        (status = 200, description = "Long-poll result", body = serde_json::Value),
    ),
    tag = "watch"
)]
// check-tenant-scope: delegated (long-poll stub, no DB query — auth enforced via TenantScope extractor only)
pub async fn watch_config(
    State(_state): State<AppState>,
    Query(params): Query<WatchParams>,
    _scope: TenantScope,
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
