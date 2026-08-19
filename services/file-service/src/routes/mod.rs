pub mod file;
pub mod migration_state;
pub mod presign;

use axum::{Router, middleware as axum_mw, routing::get};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::PrincipalStateProvider;
use std::sync::Arc;
use crate::services::FileService;
use prometheus::{Encoder, TextEncoder};

pub type AppState = FileService;

impl PrincipalStateProvider for FileService {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

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
        .merge(
            migration_state::routes(state.clone())
                .layer(axum_mw::from_fn_with_state(
                    state.clone(),
                    digua_authz::verify_aud_principal_middleware::<AppState>("file-service"),
                )),
        )
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route_layer(axum_mw::from_fn(digua_common_telemetry_axum::trace_request_middleware))
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

pub fn create_state(
    pool: sqlx::MySqlPool,
    storage: Arc<dyn crate::storage::object_store::ObjectStore>,
    base_url: String,
    max_file_size: u64,
    jwks: Arc<JWKSCache>,
) -> AppState {
    FileService::new(pool, storage, base_url, max_file_size, jwks)
}
