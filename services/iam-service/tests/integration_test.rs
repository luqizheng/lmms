#[cfg(test)]
mod tests {
    use digua_common_jwt;
    use iam_service::{config::load_config, db, state::AppState};
    use digua_common_cache::MemoryCache;
    use digua_common_event::MySqlOutboxRepo;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_config_loading() {
        // 测试配置加载
        let result = load_config();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_state_creation() {
        // 测试 AppState 创建
        let config = load_config().unwrap();
        let cache = MemoryCache::new(10000);
        let pool = sqlx::mysql::MySqlPool::connect_lazy("mysql://root:password@localhost:3306/test").unwrap();
        let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
        let jwks = Arc::new(digua_common_jwt::JWKSCache::new(
            digua_common_jwt::JwksConfig {
                url: "http://localhost:0/.well-known/jwks.json".into(),
                refresh_interval: std::time::Duration::from_secs(3600),
                request_timeout: std::time::Duration::from_secs(5),
            },
            reqwest::Client::new(),
        ));
        let service_jwt = Arc::new(digua_common_jwt::ServiceJwtCache::new(
            digua_common_jwt::ServiceJwtConfig {
                token_service_url: "http://localhost:0".into(),
                bootstrap_token: "test-bt".into(),
                caller_service_name: "iam-service".into(),
                caller_tenant: 1,
                refresh_skew: std::time::Duration::from_secs(60),
                request_timeout: std::time::Duration::from_secs(5),
            },
            reqwest::Client::new(),
            String::new(),
        ));
        let state = AppState::new(
            pool, cache, config.clone(), None, outbox_repo, jwks, service_jwt,
        );
        assert!(state.config.app.name == config.app.name);
    }
}
