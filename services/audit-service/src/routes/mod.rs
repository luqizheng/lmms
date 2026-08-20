pub mod compliance;
pub mod login;
pub mod migration_state;
pub mod operation;
pub mod retention;

use crate::error::AppResult;
use crate::models::RetentionPolicy;
use crate::services::{AuditService, CleanupService};
use axum::{middleware as axum_mw, routing::get, Router};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::PrincipalStateProvider;
use prometheus::{Encoder, TextEncoder};
use sqlx::mysql::MySqlPool;
use std::sync::Arc;

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub audit: AuditService,
    pub cleanup: CleanupService,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
    pub jwks: Arc<JWKSCache>,
}

impl AppServices {
    pub async fn new(pool: MySqlPool, jwks: Arc<JWKSCache>) -> AppResult<Self> {
        let policy = RetentionPolicy::default();
        let audit = AuditService::new(&pool);
        let cleanup = CleanupService::new(&pool, policy);

        Ok(Self {
            audit,
            cleanup,
            jwks,
        })
    }

    pub async fn get_operation_logs_by_filter(
        &self,
        user_id: Option<&str>,
        action: Option<&str>,
        resource: Option<&str>,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> AppResult<Vec<crate::models::OperationLog>> {
        self.audit
            .query_logs(user_id, action, resource, start_date, end_date, 10000, 0)
            .await
    }
}

impl PrincipalStateProvider for AppServices {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

pub fn routes(state: AppState) -> Router {
    let migration_state_with_authz =
        migration_state::routes(state.clone()).layer(axum_mw::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<AppState>("audit-service"),
        ));
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
        .nest("/audit", compliance::routes(state.clone()))
        .nest("/audit", retention::routes(state.clone()))
        .merge(migration_state_with_authz)
        .route("/health", get(health))
        .route("/audit/health", get(health))
        .route("/metrics", get(metrics))
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

pub fn create_state(pool: MySqlPool, jwks: Arc<JWKSCache>) -> AppState {
    Arc::new(AppServices {
        audit: AuditService::new(&pool),
        cleanup: CleanupService::new(&pool, RetentionPolicy::default()),
        jwks,
    })
}
