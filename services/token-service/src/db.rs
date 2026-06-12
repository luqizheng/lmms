use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

use crate::config::DatabaseCfg;

pub const INIT_SQL: &str = include_str!("../migrations/20260611000001_init.sql");

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

/// 启动时跑 migrations/20260611000001_init.sql；表已存在则忽略，保证重启幂等。
pub async fn init_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let p = match pool {
        DbPool::Mysql(p) => p,
    };
    for stmt in INIT_SQL.split(';') {
        // 剥掉段内的纯行注释，再 trim 判空（注释行后可能紧跟 DDL）
        let cleaned: String = stmt
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = cleaned.trim();
        if s.is_empty() {
            continue;
        }
        if let Err(e) = sqlx::query(s).execute(p).await {
            if let Some(db) = e.as_database_error() {
                if db.code().as_deref() == Some("42S01") {
                    continue;
                }
            }
            return Err(e);
        }
    }
    Ok(())
}
