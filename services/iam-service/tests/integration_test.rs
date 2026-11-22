//! AppState 构造的冒烟测试。
//!
//! 注意：不能直接调 `load_config()`——seeded yaml 故意把 `database.url` /
//! `redis.url` 留空并要求来自 env（fail-fast 校验），裸环境（CI runner / 本机）
//! 没有这些 env 时必然校验失败。这里用 `load_config_with_env` 注入占位值，
//! 与生产「env 优先」路径保持同一套合并逻辑，也不受并发测试的 env 竞争影响。
#[cfg(test)]
mod tests {
    use digua_common_cache::MemoryCache;
    use digua_common_event::MySqlOutboxRepo;
    use iam_service::config::DEFAULT_CONFIG_PATH;
    use iam_service::service::token_forward::TokenForwardClient;
    use iam_service::state::AppState;
    use iam_service::{load_config_with_env, Config};
    use std::sync::Arc;

    fn load_test_config() -> Config {
        load_config_with_env(DEFAULT_CONFIG_PATH, |k| match k {
            "DATABASE_URL" => Some("mysql://root@localhost:3306/iam_test".into()),
            "REDIS_URL" => Some("redis://localhost:6379/".into()),
            _ => std::env::var(k).ok(),
        })
        .expect("load config with injected env")
    }

    #[tokio::test]
    async fn test_config_loading() {
        // 测试配置加载（env 注入版，见模块注释）
        let config = load_test_config();
        assert!(!config.database.url.is_empty());
        assert!(!config.redis.url.is_empty());
    }

    #[tokio::test]
    async fn test_state_creation() {
        // 测试 AppState 创建
        let config = load_test_config();
        let cache = MemoryCache::new(10000);
        let pool =
            sqlx::mysql::MySqlPool::connect_lazy("mysql://root:password@localhost:3306/test")
                .unwrap();
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
        let token_forward = Arc::new(TokenForwardClient::new(service_jwt.clone(), String::new()));
        let state = AppState::new(
            digua_common_db::DbPool::from(pool.clone()),
            cache,
            config.clone(),
            None,
            outbox_repo,
            jwks,
            service_jwt,
            token_forward,
        );
        assert!(state.config.app.name == config.app.name);
    }
}
