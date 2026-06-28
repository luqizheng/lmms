use sqlx::mysql::MySqlPool;
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use std::sync::Arc;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub cache: Arc<MemoryCache>,
    pub config: Arc<Config>,
    pub redis: Option<redis::aio::ConnectionManager>,
    pub redis_prefix: String,
    pub perm_epoch_ttl_sec: u64,
    pub outbox_repo: Arc<MySqlOutboxRepo>,
}

impl AppState {
    pub fn new(
        db: MySqlPool,
        cache: MemoryCache,
        config: Config,
        redis: Option<redis::aio::ConnectionManager>,
        outbox_repo: Arc<MySqlOutboxRepo>,
    ) -> Self {
        let redis_prefix = config.redis.prefix.clone();
        let perm_epoch_ttl_sec = config.jwt.access_token_ttl_minutes * 60;
        Self {
            db,
            cache: Arc::new(cache),
            config: Arc::new(config),
            redis,
            redis_prefix,
            perm_epoch_ttl_sec,
            outbox_repo,
        }
    }
}

pub type State = axum::extract::State<AppState>;