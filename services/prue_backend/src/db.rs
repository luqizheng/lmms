use digua_common_db::DbPool;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const INIT_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "prue_menu_configs";

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
            Ok(())
        }
    }
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
