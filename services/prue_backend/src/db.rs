use common_db::DbPool;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const INIT_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V001__init.sql");
const V002_SQL_MYSQL: &str = include_str!("../migrations/mysql/V002__routes_etag.sql");
const V002_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V002__routes_etag.sql");
const SCHEMA_MARKER_TABLE: &str = "menu_configs";
const ETAG_COLUMN: &str = "routes_etag";

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    match pool {
        DbPool::Mysql(p) => {
            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM information_schema.tables \
                 WHERE table_schema = DATABASE() AND table_name = ?",
            )
            .bind(SCHEMA_MARKER_TABLE)
            .fetch_one(p)
            .await?;
            if exists.0 == 0 {
                info!("prue_backend schema missing, applying initial migration");
                for s in split_sql(INIT_SQL_MYSQL) {
                    if let Err(e) = sqlx::query(&s).execute(p).await {
                        warn!(error = %e, "prue_backend migration statement failed");
                        return Err(anyhow::anyhow!("apply prue_backend migration: {e}"));
                    }
                }
                info!("prue_backend initial migration applied");
            } else {
                info!(marker = SCHEMA_MARKER_TABLE, "prue_backend schema present");
            }
            apply_v002_mysql(p).await?;
            Ok(())
        }
        DbPool::Sqlite(p) => {
            let exists: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(SCHEMA_MARKER_TABLE)
            .fetch_one(p)
            .await?;
            if exists.0 == 0 {
                info!("prue_backend schema missing, applying initial migration");
                for s in split_sql(INIT_SQL_SQLITE) {
                    if let Err(e) = sqlx::query(&s).execute(p).await {
                        warn!(error = %e, "prue_backend migration statement failed");
                        return Err(anyhow::anyhow!("apply prue_backend migration: {e}"));
                    }
                }
                info!("prue_backend initial migration applied");
            } else {
                info!(marker = SCHEMA_MARKER_TABLE, "prue_backend schema present");
            }
            apply_v002_sqlite(p).await?;
            Ok(())
        }
    }
}

async fn apply_v002_mysql(p: &sqlx::MySqlPool) -> anyhow::Result<()> {
    let exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
    )
    .bind(SCHEMA_MARKER_TABLE)
    .bind(ETAG_COLUMN)
    .fetch_one(p)
    .await?;
    if exists.0 > 0 {
        return Ok(());
    }
    info!("prue_backend V002 missing, applying routes_etag column");
    for s in split_sql(V002_SQL_MYSQL) {
        if let Err(e) = sqlx::query(&s).execute(p).await {
            return Err(anyhow::anyhow!("apply prue_backend V002: {e}"));
        }
    }
    info!("prue_backend V002 applied");
    Ok(())
}

async fn apply_v002_sqlite(p: &sqlx::SqlitePool) -> anyhow::Result<()> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('menu_configs') WHERE name = ?")
            .bind(ETAG_COLUMN)
            .fetch_optional(p)
            .await?;
    if row.is_none() {
        info!("prue_backend V002 missing, applying routes_etag column (sqlite)");
        for s in split_sql(V002_SQL_SQLITE) {
            if let Err(e) = sqlx::query(&s).execute(p).await {
                return Err(anyhow::anyhow!("apply prue_backend V002 (sqlite): {e}"));
            }
        }
    }

    let pending: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, routes_json FROM menu_configs WHERE deleted_at IS NULL AND routes_etag = ''",
    )
    .fetch_all(p)
    .await?;
    if pending.is_empty() {
        return Ok(());
    }
    info!(count = pending.len(), "prue_backend backfilling routes_etag");
    for (id, routes_json) in pending {
        let etag = compute_etag_from_string(&routes_json)?;
        sqlx::query("UPDATE menu_configs SET routes_etag = ? WHERE id = ?")
            .bind(&etag)
            .bind(&id)
            .execute(p)
            .await?;
    }
    Ok(())
}

fn compute_etag_from_string(raw: &str) -> anyhow::Result<String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| anyhow::anyhow!("invalid routes_json during backfill: {e}"))?;
    Ok(crate::service::etag::compute(&value))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest.iter() {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

fn split_sql(sql: &str) -> Vec<String> {
    sql.split(';')
        .filter_map(|stmt| {
            let cleaned: String = stmt
                .lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            let s = cleaned.trim().to_string();
            if s.is_empty() {
                return None;
            }
            let upper = s.to_ascii_uppercase();
            if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
                return None;
            }
            Some(s)
        })
        .collect()
}
