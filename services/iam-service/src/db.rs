use sqlx::mysql::MySqlPool;
use crate::config::DatabaseConfig;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "users";

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let pool = common::db::connect(&config.url, config.max_connections).await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

pub async fn ensure_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .fetch_one(pool)
    .await?;
    if row.0 > 0 {
        info!(marker = SCHEMA_MARKER_TABLE, "iam schema present, skip migration");
        return Ok(());
    }

    info!("iam schema missing, applying initial migration");
    for stmt in INIT_SQL.split(';') {
        let cleaned: String = stmt
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let s = cleaned.trim();
        if s.is_empty() {
            continue;
        }
        let upper = s.to_ascii_uppercase();
        if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
            continue;
        }
        if let Err(e) = sqlx::query(s).execute(pool).await {
            warn!(error = %e, "iam migration statement failed");
            return Err(anyhow::anyhow!("apply iam migration: {e}"));
        }
    }
    info!("iam initial migration applied");
    Ok(())
}
