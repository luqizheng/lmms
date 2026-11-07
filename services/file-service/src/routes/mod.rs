pub mod file;
pub mod migration_state;
pub mod presign;

use crate::services::FileService;
use axum::{middleware as axum_mw, routing::get, Router};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::PrincipalStateProvider;
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;

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

    // 2026-11-07 裁决：file-service 定位为纯内部服务，/file/* 只接受
    // service-jwt（aud=file-service）。用户 JWT（aud=coder-micro-services）
    // 在此 401；用户面路径走网关→业务服务→本服务（ServiceJwtCache），
    // 归属身份由受信调用方在 metadata 中显式声明（见 contracts/http-conventions.md）。
    let file = axum_mw::from_fn_with_state(
        state.clone(),
        digua_authz::verify_aud_principal_middleware::<AppState>("file-service"),
    );
    Router::new()
        .nest("/file", file::routes(state.clone()).layer(file.clone()))
        .nest(
            "/file",
            presign::routes(state.clone()).layer(file.clone()),
        )
        .merge(
            migration_state::routes(state.clone()).layer(axum_mw::from_fn_with_state(
                state.clone(),
                digua_authz::verify_aud_principal_middleware::<AppState>("file-service"),
            )),
        )
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

#[allow(dead_code)] // kept for future use; suppression avoids dead_code lint until wired
async fn metrics() -> axum::response::Html<String> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    axum::response::Html(String::from_utf8(buffer).unwrap())
}

pub fn create_state(
    pool: digua_common_db::DbPool,
    storage: Arc<dyn crate::storage::object_store::ObjectStore>,
    base_url: String,
    max_file_size: u64,
    jwks: Arc<JWKSCache>,
) -> AppState {
    FileService::new(pool, storage, base_url, max_file_size, jwks)
}
