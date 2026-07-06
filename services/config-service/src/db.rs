use common_db::DbPool;
use sqlx::MySqlPool;
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V002_SQL: &str = include_str!("../migrations/mysql/V002__tenant_cfg.sql");
const INIT_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "cfg_namespaces";
const V002_MARKER_TABLE: &str = "cfg_migration_v002_applied";

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    match pool {
        DbPool::Mysql(p) => ensure_mysql(p).await,
        DbPool::Sqlite(p) => ensure_sqlite(p).await,
    }
}

async fn ensure_mysql(pool: &MySqlPool) -> anyhow::Result<()> {
    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "config-service schema missing, applying V001"
        );
        for stmt in INIT_SQL_MYSQL.split(';') {
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
                warn!(statement = %first_line(s), error = %e, "V001 statement failed");
                return Err(anyhow::anyhow!("apply V001 ({}): {e}", first_line(s)));
            }
        }
        info!("config-service V001 applied");
    } else {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "config-service schema present, skip V001"
        );
    }

    if !table_exists(pool, V002_MARKER_TABLE).await? {
        info!(marker = V002_MARKER_TABLE, "applying V002");
        for stmt in V002_SQL.split(';') {
            let cleaned: String = stmt
                .lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            let s = cleaned.trim();
            if s.is_empty() {
                continue;
            }
            if let Err(e) = sqlx::query(s).execute(pool).await {
                warn!(statement = %first_line(s), error = %e, "V002 statement failed");
                return Err(anyhow::anyhow!("apply V002 ({}): {e}", first_line(s)));
            }
        }
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS cfg_migration_v002_applied \
             (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("config-service V002 applied");
    } else {
        info!(marker = V002_MARKER_TABLE, "V002 already applied, skip");
    }

    Ok(())
}

async fn ensure_sqlite(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .fetch_one(pool)
    .await?;
    if row.0 > 0 {
        info!(marker = SCHEMA_MARKER_TABLE, "config-service schema present, skip migration");
        return Ok(());
    }

    info!("config-service schema missing, applying initial migration");
    for stmt in INIT_SQL_SQLITE.split(';') {
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
            warn!(error = %e, "config-service migration statement failed");
            return Err(anyhow::anyhow!("apply cfg migration: {e}"));
        }
    }
    info!("config-service initial migration applied");
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

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}
