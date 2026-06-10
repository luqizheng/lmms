use anyhow::Result;
use sqlx::mysql::MySqlPool;

pub async fn init_pool(url: &str, max: u32) -> Result<MySqlPool> {
    common::db::connect(url, max).await
}
