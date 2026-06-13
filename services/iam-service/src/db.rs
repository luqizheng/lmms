use sqlx::mysql::MySqlPool;
use crate::config::DatabaseConfig;

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    common::db::connect(&config.url, config.max_connections).await
}