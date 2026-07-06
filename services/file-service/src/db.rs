use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V002_SQL: &str = include_str!("../migrations/mysql/V002__file_tenant_reconcile.sql");
const SCHEMA_MARKER_TABLE: &str = "file_files";
const V002_MARKER_TABLE: &str = "file_migration_v002_applied";

pub async fn init_pool(database_url: &str) -> anyhow::Result<MySqlPool> {
    MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|e| anyhow::anyhow!("DB_CONNECT: {e}"))
}

pub async fn ensure_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "file-service schema missing, applying V001"
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
        info!("file-service V001 applied");
    } else {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "file-service schema present, skip V001"
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
            "CREATE TABLE IF NOT EXISTS file_migration_v002_applied \
             (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("file-service V002 applied");
    } else {
        info!(marker = V002_MARKER_TABLE, "V002 already applied, skip");
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

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}