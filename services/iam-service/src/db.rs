use crate::config::DatabaseConfig;
use sqlx::mysql::MySqlPool;
use tracing::{info, warn};

const INIT_SQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V002_SQL: &str = include_str!("../migrations/mysql/V002__outbox_archive.sql");
const V003_SQL: &str = include_str!("../migrations/mysql/V003__tenants.sql");
const V004_SQL: &str = include_str!("../migrations/mysql/V004__tenant_business_tables.sql");
const V005_SQL: &str = include_str!("../migrations/mysql/V005__platform_admin_role.sql");
const V006_SQL: &str = include_str!("../migrations/mysql/V006__org_unit_path.sql");
const V007_SQL: &str = include_str!("../migrations/mysql/V007__system_menu_permission.sql");
const SCHEMA_MARKER_TABLE: &str = "iam_users";
const V002_MARKER_TABLE: &str = "iam_outbox_events_archive";
const V003_MARKER_TABLE: &str = "iam_tenants";
const V004_MARKER_TABLE: &str = "iam_migration_v004_applied";
const V005_MARKER_TABLE: &str = "iam_migration_v005_applied";
const V006_MARKER_TABLE: &str = "iam_migration_v006_applied";
const V007_MARKER_TABLE: &str = "iam_migration_v007_applied";

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let pool = digua_common::db::connect(&config.url, config.max_connections).await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

pub async fn ensure_schema(pool: &MySqlPool) -> anyhow::Result<()> {
    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!("iam schema missing, applying initial migration");
        apply_sql_script(pool, INIT_SQL).await?;
        info!("iam initial migration applied");
    } else {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "iam schema present, skip V001"
        );
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

    if !table_exists(pool, V004_MARKER_TABLE).await? {
        info!("iam V004 marker missing, applying V004");
        apply_sql_script(pool, V004_SQL).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS iam_migration_v004_applied (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("iam V004 (tenant business tables) applied");
    }

    if !table_exists(pool, V005_MARKER_TABLE).await? {
        info!("iam V005 marker missing, applying V005");
        apply_sql_script(pool, V005_SQL).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS iam_migration_v005_applied (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("iam V005 (platform_admin role) applied");
    }

    if !table_exists(pool, V006_MARKER_TABLE).await? {
        info!("iam V006 marker missing, applying V006");
        apply_sql_script(pool, V006_SQL).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS iam_migration_v006_applied (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("iam V006 (org unit path) applied");
    }

    if !table_exists(pool, V007_MARKER_TABLE).await? {
        info!("iam V007 marker missing, applying V007");
        apply_sql_script(pool, V007_SQL).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS iam_migration_v007_applied (applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        info!("iam V007 (system menu permission) applied");
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
