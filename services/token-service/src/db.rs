use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

use crate::config::DatabaseCfg;

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
}

impl DbPool {
    pub async fn ping(&self) -> Result<()> {
        match self {
            Self::Mysql(p) => {
                sqlx::query("SELECT 1").execute(p).await.context("ping")?;
            }
        }
        Ok(())
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // 强制 UTC session，确保 NOW() 与 expires_at 一致
                sqlx::query("SET time_zone = '+00:00'")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&cfg.url)
        .await
        .with_context(|| format!("connect mysql {}", cfg.url))?;
    Ok(DbPool::Mysql(pool))
}
