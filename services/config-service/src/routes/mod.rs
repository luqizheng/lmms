pub mod namespace;
pub mod config;
pub mod watch;

use axum::Router;
use std::sync::Arc;
use crate::services::ConfigService;

pub type AppState = Arc<ConfigService>;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/config", namespace::routes(state.clone()))
        .nest("/config", config::routes(state.clone()))
        .nest("/config", watch::routes(state.clone()))
}

pub fn create_state(pool: common_db::DbPool) -> AppState {
    Arc::new(ConfigService::new(pool))
}
