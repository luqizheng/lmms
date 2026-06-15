pub mod compliance;
pub mod operation;
pub mod login;
pub mod retention;

use std::sync::Arc;
use crate::services::{AuditService, CleanupService};
use crate::models::RetentionPolicy;
use crate::error::AppResult;
use axum::Router;
use sqlx::mysql::MySqlPool;

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

        Ok(Self {
            audit,
            cleanup,
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
        self.audit.query_logs(user_id, action, resource, start_date, end_date, 10000, 0).await
    }
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
        .nest("/audit", compliance::routes(state.clone()))
        .nest("/audit", retention::routes(state.clone()))
}

pub fn create_state(pool: MySqlPool) -> AppState {
    Arc::new(AppServices {
        audit: AuditService::new(&pool),
        cleanup: CleanupService::new(&pool, RetentionPolicy::default()),
    })
}
