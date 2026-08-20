pub mod menu_configs;
pub mod migration_state;

use crate::state::AppState;
use axum::{middleware as axum_mw, routing::get, Router};
use prometheus::{Encoder, TextEncoder};

pub fn routes(state: AppState) -> Router {
    let migration_state_with_authz =
        migration_state::routes(state.clone()).layer(axum_mw::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<AppState>("prue_backend"),
        ));
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .merge(menu_configs::routes(state.clone()))
        .merge(migration_state_with_authz)
        .route_layer(axum_mw::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ))
}

async fn health() -> &'static str {
    "OK"
}

async fn metrics() -> axum::response::Html<String> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    axum::response::Html(String::from_utf8(buffer).unwrap())
}
