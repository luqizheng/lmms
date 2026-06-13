use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use sqlx::Row;
use std::time::Duration;
use tracing::{info, warn};

use crate::config::{DatabaseCfg, DbType};

const INIT_SQL: &str = include_str!("../migrations/0001_init.sql");
const SCHEMA_MARKER_TABLE: &str = "users";

#[derive(Clone)]
pub enum DbPool {
    Mysql(MySqlPool),
}

impl DbPool {
    pub fn db_type(&self) -> DbType {
        match self {
            Self::Mysql(_) => DbType::Mysql,
        }
    }
}

pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.url)
        .await
        .with_context(|| format!("connect mysql {}", cfg.url))?;
    Ok(DbPool::Mysql(pool))
}

/// Apply initial schema migration if the database is empty.
///
/// Strategy: probe `information_schema.tables` for a marker table (`users`).
/// If it exists, skip; otherwise execute the embedded SQL statements one by one.
/// Designed for a single-node first-boot deployment; it is not a full migration
/// framework — every statement is idempotent (`CREATE TABLE IF NOT EXISTS`).
pub async fn ensure_schema(pool: &DbPool) -> Result<()> {
    match pool {
        DbPool::Mysql(p) => {
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
                info!(marker = SCHEMA_MARKER_TABLE, "schema already present, skip migration");
                return Ok(());
            }

            info!(marker = SCHEMA_MARKER_TABLE, "schema missing, applying initial migration");
            for stmt in split_sql(INIT_SQL) {
                let s = stmt.trim();
                if s.is_empty() || s.starts_with("--") {
                    continue;
                }
                if let Err(e) = sqlx::query(s).execute(p).await {
                    warn!(
                        statement = %first_line(s),
                        error = %e,
                        "migration statement failed"
                    );
                    return Err(anyhow::Error::from(e)
                        .context(format!("apply migration: {}", first_line(s))));
                }
            }
            info!("initial migration applied");
            Ok(())
        }
    }
}

fn split_sql(sql: &str) -> Vec<&str> {
    sql.split(';').collect()
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sql_returns_non_empty_statements() {
        let sql = "CREATE TABLE a (id INT);\n\n-- comment\nCREATE TABLE b (id INT);\n";
        let parts: Vec<&str> = split_sql(sql)
            .into_iter()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains("CREATE TABLE a"));
        assert!(parts[1].contains("CREATE TABLE b"));
    }

    #[test]
    fn first_line_skips_comments_and_blanks() {
        assert_eq!(first_line("\n  -- hi\n  CREATE TABLE x (id INT);"), "CREATE TABLE x (id INT);");
        assert_eq!(first_line("-- only comment"), "<empty>");
    }

    #[test]
    fn init_sql_is_non_empty_and_has_all_tables() {
        let upper = INIT_SQL.to_uppercase();
        for tbl in [
            "USERS",
            "ROLES",
            "PERMISSIONS",
            "ROLE_PERMISSIONS",
            "USER_ROLES",
            "ORG_UNITS",
            "POSITIONS",
            "USER_POSITIONS",
            "POSITION_ROLES",
            "USER_LOGIN_LOG",
        ] {
            assert!(
                upper.contains(&format!("CREATE TABLE IF NOT EXISTS {tbl}")),
                "missing CREATE TABLE for {tbl}"
            );
        }
    }
}