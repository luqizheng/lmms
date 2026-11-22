//! iam-service 的方言无关 DB 层（MySQL / PostgreSQL）。
//!
//! 迁移脚本按方言选择（`migrations/mysql/` vs `migrations/postgres/`），
//! marker 探测统一走 `digua_common_db::migration_state::probe_marker`
//! （内部对 PG 用 `current_schema()`、MySQL 用 `DATABASE()`）。
//!
//! 与 `notification-service/src/db.rs` / `config-service/src/db.rs` 保持同一套结构：
//! 双方言 `include_str!` + `script_for` 方言选择 + `apply_script` 剥注释逐条执行。
//!
//! ⚠️ 执行器是「先按 ASCII 分号切分、再剥离 `--` 注释行」，
//! 因此**迁移文件的注释里不能出现 ASCII 分号**。

use crate::config::DatabaseConfig;
use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, migration_state, DbPool};
use tracing::{info, warn};

const V001_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const V001_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V001__init.sql");
const V002_SQL_MYSQL: &str = include_str!("../migrations/mysql/V002__outbox_archive.sql");
const V002_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V002__outbox_archive.sql");
const V003_SQL_MYSQL: &str = include_str!("../migrations/mysql/V003__tenants.sql");
const V003_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V003__tenants.sql");
const V004_SQL_MYSQL: &str = include_str!("../migrations/mysql/V004__tenant_business_tables.sql");
const V004_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V004__tenant_business_tables.sql");
const V005_SQL_MYSQL: &str = include_str!("../migrations/mysql/V005__platform_admin_role.sql");
const V005_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V005__platform_admin_role.sql");
const V006_SQL_MYSQL: &str = include_str!("../migrations/mysql/V006__org_unit_path.sql");
const V006_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V006__org_unit_path.sql");
const V007_SQL_MYSQL: &str = include_str!("../migrations/mysql/V007__system_menu_permission.sql");
const V007_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V007__system_menu_permission.sql");
const V008_SQL_MYSQL: &str =
    include_str!("../migrations/mysql/V008__rebind_tenant_admin_permissions.sql");
const V008_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V008__rebind_tenant_admin_permissions.sql");
const V009_SQL_MYSQL: &str = include_str!("../migrations/mysql/V009__align_bigint_signed.sql");
const V009_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V009__align_bigint_signed.sql");
const V010_SQL_MYSQL: &str = include_str!("../migrations/mysql/V010__iam_self_service.sql");
const V010_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V010__iam_self_service.sql");

/// V001 的 marker 表：它创建的第一张业务表（与 `migration_state` 端点自描述一致）。
const SCHEMA_MARKER_TABLE: &str = "iam_users";
/// V002 的 marker 表：归档表本身（V002 只建这一张表）。
const V002_MARKER_TABLE: &str = "iam_outbox_events_archive";
/// V003 的 marker 表：租户表本身。
const V003_MARKER_TABLE: &str = "iam_tenants";
/// V004~V008 是 ALTER / 种子修正型迁移，无天然业务表可探测，
/// 由 `ensure_schema` 在脚本执行后建同名哨兵表（与 MySQL 侧既有约定一致）。
const V004_MARKER_TABLE: &str = "iam_migration_v004_applied";
const V005_MARKER_TABLE: &str = "iam_migration_v005_applied";
const V006_MARKER_TABLE: &str = "iam_migration_v006_applied";
const V007_MARKER_TABLE: &str = "iam_migration_v007_applied";
const V008_MARKER_TABLE: &str = "iam_migration_v008_applied";
/// V009：MySQL 侧把 iam_tenants.id 与四张表的 tenant_id 改有符号 BIGINT
/// （sqlx i64 解码要求，notification V004 同款）；PG 侧空操作。
const V009_MARKER_TABLE: &str = "iam_migration_v009_applied";
/// V010: self-service one-time tokens (password reset + invitation).
const V010_MARKER_TABLE: &str = "iam_migration_v010_applied";

pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<DbPool> {
    // 按URL判方言建池（PG 分支由 `init_pool_with` 锁定会话时区为 UTC）。
    let pool = digua_common_db::init_pool_with(&config.url, config.max_connections).await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    let dialect = pool.dialect();

    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!(marker = SCHEMA_MARKER_TABLE, dialect = ?dialect, "iam schema missing, applying V001");
        apply_script(
            pool,
            script_for(dialect, V001_SQL_MYSQL, V001_SQL_POSTGRES),
            "V001",
        )
        .await?;
        info!("iam initial migration applied");
    } else {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "iam schema present, skip V001"
        );
    }

    if !table_exists(pool, V002_MARKER_TABLE).await? {
        info!(marker = V002_MARKER_TABLE, "applying V002");
        apply_script(
            pool,
            script_for(dialect, V002_SQL_MYSQL, V002_SQL_POSTGRES),
            "V002",
        )
        .await?;
        info!("iam V002 (outbox archive) applied");
    } else {
        info!(marker = V002_MARKER_TABLE, "iam V002 already applied, skip");
    }

    if !table_exists(pool, V003_MARKER_TABLE).await? {
        info!(marker = V003_MARKER_TABLE, "applying V003");
        apply_script(
            pool,
            script_for(dialect, V003_SQL_MYSQL, V003_SQL_POSTGRES),
            "V003",
        )
        .await?;
        info!("iam V003 (tenants) applied");
    } else {
        info!(marker = V003_MARKER_TABLE, "iam V003 already applied, skip");
    }

    apply_with_sentinel(
        pool,
        dialect,
        V004_MARKER_TABLE,
        "V004",
        "tenant business tables",
        V004_SQL_MYSQL,
        V004_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V005_MARKER_TABLE,
        "V005",
        "platform_admin role",
        V005_SQL_MYSQL,
        V005_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V006_MARKER_TABLE,
        "V006",
        "org unit path",
        V006_SQL_MYSQL,
        V006_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V007_MARKER_TABLE,
        "V007",
        "system menu permission",
        V007_SQL_MYSQL,
        V007_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V008_MARKER_TABLE,
        "V008",
        "rebind tenant_admin permissions",
        V008_SQL_MYSQL,
        V008_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V009_MARKER_TABLE,
        "V009",
        "align bigint signed",
        V009_SQL_MYSQL,
        V009_SQL_POSTGRES,
    )
    .await?;
    apply_with_sentinel(
        pool,
        dialect,
        V010_MARKER_TABLE,
        "V010",
        "self service one time tokens",
        V010_SQL_MYSQL,
        V010_SQL_POSTGRES,
    )
    .await?;

    Ok(())
}

/// V004~V008 共用流程：脚本执行 + 建哨兵表（PG 侧时间列 TIMESTAMPTZ）。
#[allow(clippy::too_many_arguments)]
async fn apply_with_sentinel(
    pool: &DbPool,
    dialect: DialectKind,
    marker_table: &str,
    version: &str,
    name: &str,
    sql_mysql: &'static str,
    sql_postgres: &'static str,
) -> anyhow::Result<()> {
    if !table_exists(pool, marker_table).await? {
        info!(marker = marker_table, "applying {version}");
        apply_script(pool, script_for(dialect, sql_mysql, sql_postgres), version).await?;
        db_execute!(pool, marker_ddl(dialect, marker_table))?;
        info!("iam {version} ({name}) applied");
    } else {
        info!(marker = marker_table, "iam {version} already applied, skip");
    }
    Ok(())
}

fn script_for(dialect: DialectKind, mysql: &'static str, postgres: &'static str) -> &'static str {
    match dialect {
        DialectKind::Postgres => postgres,
        _ => mysql,
    }
}

/// 哨兵表 DDL：MySQL `DATETIME`（与既有存量库一致），PG `TIMESTAMPTZ`。
fn marker_ddl(dialect: DialectKind, table: &str) -> String {
    let ts = match dialect {
        DialectKind::Postgres => "TIMESTAMPTZ",
        _ => "DATETIME",
    };
    format!(
        "CREATE TABLE IF NOT EXISTS {table} \
         (applied_at {ts} NOT NULL DEFAULT CURRENT_TIMESTAMP)"
    )
}

async fn table_exists(pool: &DbPool, table: &str) -> anyhow::Result<bool> {
    // 统一走基座层入口：PG 用 current_schema()，MySQL 用 DATABASE()。
    migration_state::probe_marker(pool, table)
        .await
        .map_err(|e| anyhow::anyhow!("probe marker {table}: {e}"))
}

async fn apply_script(pool: &DbPool, script: &str, label: &str) -> anyhow::Result<()> {
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
        // MySQL 侧迁移文件头部可能有 CREATE DATABASE / USE（历史遗留），跳过。
        if upper.starts_with("CREATE DATABASE") || upper.starts_with("USE ") {
            continue;
        }
        if let Err(e) = db_execute!(pool, s) {
            warn!(statement = %first_line(s), error = %e, "iam migration statement failed");
            return Err(anyhow::anyhow!(
                "apply iam {label} ({}): {e}",
                first_line(s)
            ));
        }
    }
    Ok(())
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty() && !l.trim().starts_with("--"))
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}
