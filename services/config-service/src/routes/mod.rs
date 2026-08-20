pub mod config;
pub mod migration_state;
pub mod namespace;
pub mod watch;

use crate::services::ConfigService;
use axum::{middleware as axum_mw, routing::get, Router};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::PrincipalStateProvider;
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;

pub type AppState = ConfigService;

impl PrincipalStateProvider for ConfigService {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

pub fn routes(state: AppState) -> Router {
    let migration_state_with_authz =
        migration_state::routes(state.clone()).layer(axum_mw::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<AppState>("config-service"),
        ));
    Router::new()
        .nest("/config", namespace::routes(state.clone()))
        .nest("/config", config::routes(state.clone()))
        .nest("/config", watch::routes(state.clone()))
        .merge(migration_state_with_authz)
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route_layer(axum_mw::from_fn(
            digua_common_telemetry_axum::trace_request_middleware,
        ))
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
