pub mod compliance;
pub mod operation;
pub mod login;
pub mod retention;

use std::sync::Arc;
use crate::services::{AuditService, CleanupService};
use crate::models::RetentionPolicy;
use crate::error::AppResult;
use axum::{Router, middleware as axum_mw, routing::get};
use sqlx::mysql::MySqlPool;
use prometheus::{Encoder, TextEncoder};

pub type AppState = Arc<AppServices>;

pub struct AppServices {
    pub audit: AuditService,
    pub cleanup: CleanupService,
}

impl AppServices {
    pub async fn new(pool: MySqlPool) -> AppResult<Self> {
        let policy = RetentionPolicy::default();
        let audit = AuditService::new(&pool);
        let cleanup = CleanupService::new(&pool, policy);

        Ok(Self { audit, cleanup })
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

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
        .nest("/audit", compliance::routes(state.clone()))
        .nest("/audit", retention::routes(state.clone()))
        .route("/health", get(health))
        .route("/audit/health", get(health))
        .route("/metrics", get(metrics))
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

pub fn create_state(pool: MySqlPool) -> AppState {
    Arc::new(AppServices {
        audit: AuditService::new(&pool),
        cleanup: CleanupService::new(&pool, RetentionPolicy::default()),
    })
}
