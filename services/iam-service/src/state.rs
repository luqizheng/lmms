use sqlx::mysql::MySqlPool;
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use common_jwt::{JWKSCache, ServiceJwtCache};
use jwt_ctx::PrincipalStateProvider;
use std::sync::Arc;
use crate::config::Config;
use crate::service::TokenForwardClient;

#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub cache: Arc<MemoryCache>,
    pub config: Arc<Config>,
    pub redis: Option<redis::aio::ConnectionManager>,
    pub redis_prefix: String,
    pub perm_epoch_ttl_sec: u64,
    pub outbox_repo: Arc<MySqlOutboxRepo>,
    /// Phase 5-B: JWKS cache for verifying incoming user-jwt (and service-jwt
    /// for /iam/internal/*).
    pub jwks: Arc<JWKSCache>,
    /// Phase 5-B: service-jwt cache for outbound calls (none today; future).
    pub service_jwt: Arc<ServiceJwtCache>,
    /// Forwards user-JWT issuance to token-service (single source of signing keys).
    pub token_forward: Arc<TokenForwardClient>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: MySqlPool,
        cache: MemoryCache,
        config: Config,
        redis: Option<redis::aio::ConnectionManager>,
        outbox_repo: Arc<MySqlOutboxRepo>,
        jwks: Arc<JWKSCache>,
        service_jwt: Arc<ServiceJwtCache>,
        token_forward: Arc<TokenForwardClient>,
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
            jwks,
            service_jwt,
            token_forward,
        }
    }
}

impl PrincipalStateProvider for AppState {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}

pub type State = axum::extract::State<AppState>;