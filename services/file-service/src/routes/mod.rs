pub mod file;
pub mod presign;

use axum::Router;
use std::sync::Arc;
use crate::services::FileService;

pub type AppState = Arc<FileService>;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/file", file::routes(state.clone()))
        .nest("/file", presign::routes(state.clone()))
}

pub fn create_state(pool: common_db::DbPool, storage: Arc<dyn crate::storage::ObjectStore>, base_url: String) -> AppState {
    Arc::new(FileService::new(pool, storage, base_url))
}