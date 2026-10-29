use anyhow::{Context, Result};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::DatabaseCfg;

/// 单个迁移：版本号 + SQL + marker table。
pub struct Migration {
    /// 版本号，`V001` / `V002` / ...
    pub version: &'static str,
    /// 该迁移的完整 SQL 脚本。
    pub sql: &'static str,
    /// 判定该迁移是否已应用的 marker table。
    pub marker_table: &'static str,
}

/// 按序执行的迁移列表。新增迁移只能追加到末尾。
///
/// marker table 的选择规则（与 iam-service 一致）：
///   - V001 建了 `token_oauth_clients`，用它当 marker；
///   - V002 是 ALTER 型迁移，MySQL 8.0 不支持 `ALTER TABLE ... ADD COLUMN IF NOT EXISTS`
///     （那是 MariaDB 扩展），改用专用哨兵表 `token_migration_v002_applied`。
///     哨兵表与 ALTER 在同一份 SQL 里落地（all-or-nothing），哨兵缺失 = V002 未应用。
///     不能用 `token_oauth_clients` 当 V002 的 marker：V001 已经在每个已部署库里
///     建好它，用它做 marker 会让存量库永远跳过 V002，列根本不会出现。
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "V001",
        sql: include_str!("../migrations/mysql/V001__init.sql"),
        marker_table: "token_oauth_clients",
    },
    Migration {
        version: "V002",
        sql: include_str!("../migrations/mysql/V002__add_client_user_and_expiry.sql"),
        marker_table: "token_migration_v002_applied",
    },
];

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

/// 按序执行 migrations/mysql/*.sql。
///
/// 每个迁移按自己的 marker table 独立判定是否已应用，因此存量库
/// （V001 的 marker 已在、V002 的哨兵表还不在）上 V002 仍会被执行。
pub async fn init_schema(pool: &DbPool) -> Result<(), sqlx::Error> {
    let DbPool::Mysql(p) = pool;
    for m in MIGRATIONS {
        if table_exists(p, m.marker_table).await? {
            info!(
                version = m.version,
                marker = m.marker_table,
                "migration already applied, skip"
            );
            continue;
        }
        info!(
            version = m.version,
            marker = m.marker_table,
            "applying migration"
        );
        apply_sql_script(p, m.sql).await?;
    }
    Ok(())
}

/// 启动时确保 schema 就位；迁移失败则启动失败（不做静默降级）。
pub async fn ensure_schema(pool: &DbPool) -> Result<()> {
    if let Err(e) = init_schema(pool).await {
        warn!(error = %e, "migration failed");
        return Err(anyhow::Error::from(e).context("apply migrations"));
    }
    Ok(())
}

/// 探测一张表在当前库里是否存在。
async fn table_exists(p: &MySqlPool, table: &str) -> Result<bool, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(p)
    .await?;
    Ok(row.0 > 0)
}

/// 执行一份迁移脚本；表已存在（错误码 42S01）则忽略，保证重启幂等。
async fn apply_sql_script(p: &MySqlPool, script: &str) -> Result<(), sqlx::Error> {
    for stmt in script.split(';') {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_sql_contains_all_marker_tables() {
        let upper = MIGRATIONS[0].sql.to_uppercase();
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

    #[test]
    fn migrations_are_versioned_in_order() {
        let versions: Vec<&str> = MIGRATIONS.iter().map(|m| m.version).collect();
        assert_eq!(versions, vec!["V001", "V002"]);
    }

    #[test]
    fn v002_has_its_own_sentinel_marker() {
        let v002 = MIGRATIONS.iter().find(|m| m.version == "V002").unwrap();
        assert_eq!(v002.marker_table, "token_migration_v002_applied");
        // 不能复用 V001 的 marker：存量库里 token_oauth_clients 早已存在。
        assert_ne!(v002.marker_table, MIGRATIONS[0].marker_table);
    }

    #[test]
    fn v002_sql_creates_its_sentinel_table() {
        let v002 = MIGRATIONS.iter().find(|m| m.version == "V002").unwrap();
        assert!(v002.sql.contains("CREATE TABLE IF NOT EXISTS token_migration_v002_applied"));
    }
}
