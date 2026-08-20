use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;
use std::time::Duration;
use tracing::{info, warn};

use crate::config::DatabaseCfg;

pub const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "token_oauth_clients";

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

/// 启动时跑 migrations/mysql/V001__init.sql；表已存在则忽略，保证重启幂等。
pub async fn init_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let DbPool::Mysql(p) = pool;
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

/// 首次启动探测：marker 表（token_oauth_clients）存在则跳过整个迁移；
/// 不存在则执行 init_schema 完成建表。
pub async fn ensure_schema(pool: &DbPool) -> Result<()> {
    let DbPool::Mysql(p) = pool;
    let row = sqlx::query(
        "SELECT COUNT(*) AS c FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .fetch_one(p)
    .await
    .with_context(|| format!("probe table {SCHEMA_MARKER_TABLE}"))?;
    let exists: i64 = row.try_get("c").unwrap_or(0);
    if exists > 0 {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "schema already present, skip migration"
        );
        return Ok(());
    }
    info!(
        marker = SCHEMA_MARKER_TABLE,
        "schema missing, applying initial migration"
    );
    if let Err(e) = init_schema(pool).await {
        warn!(error = %e, "initial migration failed");
        return Err(anyhow::Error::from(e).context("apply initial migration"));
    }
    info!("initial migration applied");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_sql_contains_all_marker_tables() {
        let upper = INIT_SQL.to_uppercase();
        for tbl in [
            "TOKEN_OAUTH_CLIENTS",
            "TOKEN_AUTH_CODES",
            "TOKEN_REVOKED_JTIS",
            "TOKEN_REFRESH_MARKERS",
        ] {
            assert!(
                upper.contains(&format!("CREATE TABLE {tbl}")),
                "missing CREATE TABLE for {tbl}"
            );
        }
    }
}
