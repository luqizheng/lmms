pub mod compliance;
pub mod login;
pub mod migration_state;
pub mod operation;
pub mod retention;

use crate::error::AppResult;
use crate::models::RetentionPolicy;
use crate::services::{AuditService, CleanupService};
use axum::{middleware as axum_mw, routing::get, Router};
use digua_common_db::DbPool;
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::PrincipalStateProvider;
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub audit: AuditService,
    pub cleanup: CleanupService,
    /// Phase 5-B: JWKS cache for verifying inbound user-jwt.
    pub jwks: Arc<JWKSCache>,
}

impl AppServices {
    pub async fn new(pool: DbPool, jwks: Arc<JWKSCache>) -> AppResult<Self> {
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
    // 2026-11-05 review (P0-4): the four business nests below were mounted
    // with NO authentication at all — any authenticated user could dump the
    // full audit trail and two-step-delete it via retention policy+cleanup.
    // Every business route now requires a valid user-jwt
    // (aud == "coder-micro-services", the audience stamped on user tokens);
    // the retention write endpoints additionally require an admin principal
    // (see retention.rs).
    let user_jwt_gate = axum_mw::from_fn_with_state(
        state.clone(),
        digua_authz::verify_aud_principal_middleware::<AppState>("coder-micro-services"),
    );
    Router::new()
        .nest("/audit", operation::routes(state.clone()).layer(user_jwt_gate.clone()))
        .nest("/audit", login::routes(state.clone()).layer(user_jwt_gate.clone()))
        .nest(
            "/audit",
            compliance::routes(state.clone()).layer(user_jwt_gate.clone()),
        )
        .nest("/audit", retention::routes(state.clone()).layer(user_jwt_gate))
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

pub fn create_state(pool: DbPool, jwks: Arc<JWKSCache>) -> AppState {
    Arc::new(AppServices {
        audit: AuditService::new(&pool),
        cleanup: CleanupService::new(&pool, RetentionPolicy::default()),
        jwks,
    })
}
