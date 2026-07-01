use sqlx::mysql::MySqlPool;
use crate::config::DatabaseConfig;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V002_SQL: &str = include_str!("../migrations/mysql/V002__outbox_archive.sql");
const V003_SQL: &str = include_str!("../migrations/mysql/V003__tenants.sql");
const SCHEMA_MARKER_TABLE: &str = "iam_users";
const V002_MARKER_TABLE: &str = "iam_outbox_events_archive";
const V003_MARKER_TABLE: &str = "iam_tenants";

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let pool = common::db::connect(&config.url, config.max_connections).await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

pub async fn ensure_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!("iam schema missing, applying initial migration");
        apply_sql_script(pool, INIT_SQL).await?;
        info!("iam initial migration applied");
    } else {
        info!(marker = SCHEMA_MARKER_TABLE, "iam schema present, skip V001");
    }

    if !table_exists(pool, V002_MARKER_TABLE).await? {
        info!("iam outbox archive table missing, applying V002");
        apply_sql_script(pool, V002_SQL).await?;
        info!("iam V002 (outbox archive) applied");
    }

    if !table_exists(pool, V003_MARKER_TABLE).await? {
        info!("iam tenants table missing, applying V003");
        apply_sql_script(pool, V003_SQL).await?;
        info!("iam V003 (tenants) applied");
    }

    Ok(())
}

async fn table_exists(pool: &MySqlPool, table: &str) -> anyhow::Result<bool> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

async fn apply_sql_script(pool: &MySqlPool, script: &str) -> anyhow::Result<()> {
    for stmt in script.split(';') {
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
    Ok(())
}
