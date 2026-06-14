pub mod operation;
pub mod login;
pub mod export;

use axum::Router;
use std::sync::Arc;
use crate::services::AuditService;

pub type AppState = Arc<AuditService>;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
        .nest("/audit", export::routes(state.clone()))
}

pub fn create_state(pool: common_db::DbPool) -> AppState {
    Arc::new(AuditService::new(pool))
}
