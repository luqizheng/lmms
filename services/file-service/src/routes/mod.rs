pub mod file;
pub mod presign;

use axum::{Router, routing::get};
use std::sync::Arc;
use crate::services::FileService;
use prometheus::{Encoder, TextEncoder};

pub type AppState = Arc<FileService>;

pub fn routes(state: AppState) -> Router {
    async fn metrics() -> axum::response::Html<String> {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        axum::response::Html(String::from_utf8(buffer).unwrap())
    }

    Router::new()
        .nest("/file", file::routes(state.clone()))
        .nest("/file", presign::routes(state.clone()))
        .route("/health", get(health))
        .route("/metrics", get(metrics))
}

async fn health() -> &'static str {
    "OK"
}

pub fn create_state(pool: common_db::DbPool, storage: Arc<crate::storage::local::LocalStorage>, base_url: String) -> AppState {
    Arc::new(FileService::new(pool, storage, base_url))
}
