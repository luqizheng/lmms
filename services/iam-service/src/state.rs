use common_db::DbPool;
use common_cache::RedisClient;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub redis: RedisClient,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: DbPool, redis: RedisClient, config: Config) -> Self {
        Self {
            db,
            redis,
            config: Arc::new(config),
        }
    }
}

pub type State = axum::extract::State<AppState>;
