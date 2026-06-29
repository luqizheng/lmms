pub mod menu_configs;

use crate::state::AppState;
use axum::{middleware as axum_mw, routing::get, Router};
use prometheus::{Encoder, TextEncoder};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .merge(menu_configs::routes(state))
        .route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))
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
