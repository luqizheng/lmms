use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

use crate::config::{DatabaseCfg, DbType};

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
}

impl DbPool {
    pub fn db_type(&self) -> DbType {
        match self {
            Self::Mysql(_) => DbType::Mysql,
        }
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.url)
        .await
        .with_context(|| format!("connect mysql {}", cfg.url))?;
    Ok(DbPool::Mysql(pool))
}
