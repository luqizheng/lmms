use common_db::DbPool;
use common_jwt::JWKSCache;
use jwt_ctx::PrincipalStateProvider;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DbPool>,
    pub jwks: Arc<JWKSCache>,
}

impl AppState {
    pub fn new(db: DbPool, jwks: Arc<JWKSCache>) -> Self {
        Self { db: Arc::new(db), jwks }
    }
}

impl PrincipalStateProvider for AppState {
    fn jwks(&self) -> Arc<JWKSCache> {
        self.jwks.clone()
    }
}
