pub mod namespace;
pub mod config;
pub mod migration_state;
pub mod watch;

use axum::{Router, middleware as axum_mw, routing::get};
use common_jwt::JWKSCache;
use jwt_ctx::PrincipalStateProvider;
use std::sync::Arc;
use crate::services::ConfigService;
use prometheus::{Encoder, TextEncoder};

pub type AppState = ConfigService;

impl PrincipalStateProvider for ConfigService {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/config", namespace::routes(state.clone()))
        .nest("/config", config::routes(state.clone()))
        .nest("/config", watch::routes(state.clone()))
        .merge(migration_state::routes(state.clone()))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route_layer(axum_mw::from_fn(common_telemetry_axum::trace_request_middleware))
        .with_state(state)
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

pub fn create_state(pool: sqlx::MySqlPool, jwks: Arc<JWKSCache>) -> AppState {
    ConfigService::new(pool, jwks)
}
