use sqlx::mysql::MySqlPool;
use common_cache::MemoryCache;
use std::sync::Arc;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub cache: Arc<MemoryCache>,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: MySqlPool, cache: MemoryCache, config: Config) -> Self {
        Self {
            db,
            cache: Arc::new(cache),
            config: Arc::new(config),
        }
    }
}

pub type State = axum::extract::State<AppState>;