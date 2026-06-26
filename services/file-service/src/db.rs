use common_db::DbPool;
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const INIT_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "file_files";

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    match pool {
        DbPool::Mysql(p) => ensure_mysql(p).await,
        DbPool::Sqlite(p) => ensure_sqlite(p).await,
    }
}

async fn ensure_mysql(pool: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .fetch_one(pool)
    .await?;
    if row.0 > 0 {
        info!(marker = SCHEMA_MARKER_TABLE, "file-service schema present, skip migration");
        return Ok(());
    }

    info!("file-service schema missing, applying initial migration");
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
            warn!(error = %e, "file-service migration statement failed");
            return Err(anyhow::anyhow!("apply file migration: {e}"));
        }
    }
    info!("file-service initial migration applied");
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
        info!(marker = SCHEMA_MARKER_TABLE, "file-service schema present, skip migration");
        return Ok(());
    }

    info!("file-service schema missing, applying initial migration");
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
            warn!(error = %e, "file-service migration statement failed");
            return Err(anyhow::anyhow!("apply file migration: {e}"));
        }
    }
    info!("file-service initial migration applied");
    Ok(())
}