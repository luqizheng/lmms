pub mod namespace;
pub mod config;
pub mod watch;

use axum::{Router, routing::get};
use std::sync::Arc;
use crate::services::ConfigService;
use prometheus::{Encoder, TextEncoder};

pub type AppState = Arc<ConfigService>;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/config", namespace::routes(state.clone()))
        .nest("/config", config::routes(state.clone()))
        .nest("/config", watch::routes(state.clone()))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
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

pub fn create_state(pool: common_db::DbPool) -> AppState {
    Arc::new(ConfigService::new(pool))
}
