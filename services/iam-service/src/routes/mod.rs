pub mod auth;
pub mod org_units;
pub mod permissions;
pub mod roles;
pub mod tenants;
pub mod users;

use axum::{Router, middleware as axum_mw, routing::get};
use crate::state::AppState;
use prometheus::{Encoder, TextEncoder};

pub fn build_router(state: AppState) -> Router {
    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    Router::new()
        .merge(auth::routes(state.clone()))
        .merge(users::routes(state.clone()))
        .merge(roles::routes(state.clone()))
        .merge(permissions::routes(state.clone()))
        .merge(org_units::routes(state.clone()))
        .merge(tenants::routes(state.clone()))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))
}

async fn health() -> &'static str {
    "OK"
}
