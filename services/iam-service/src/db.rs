use common_db::{init_pool, DbPool};
use crate::config::DatabaseConfig;

pub async fn init_pool(config: &DatabaseConfig) -> anyhow::Result<DbPool> {
    let pool = init_pool(&config.url, config.max_connections, config.min_connections).await?;
    Ok(pool)
}
