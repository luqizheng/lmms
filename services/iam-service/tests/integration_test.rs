#[cfg(test)]
mod tests {
    use iam_service::{config::load_config, db, state::AppState};
    use common_cache::MemoryCache;

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
        let state = AppState::new(
            sqlx::mysql::MySqlPool::connect_lazy("mysql://root:password@localhost:3306/test").unwrap(),
            cache,
            config.clone(),
        );
        assert!(state.config.app.name == config.app.name);
    }
}