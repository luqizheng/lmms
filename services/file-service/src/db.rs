//! file-service 的方言无关 DB 层。
//!
//! 迁移脚本按方言选择（`migrations/mysql/` vs `migrations/postgres/`），
//! marker 探测统一走 `digua_common_db::migration_state::probe_marker`
//! （内部对 PG 用 `current_schema()`、MySQL 用 `DATABASE()`）。
//!
//! ⚠️ 执行器是「先按 ASCII 分号切分、再剥离 `--` 注释行」，
//! 因此**迁移文件的注释里不能出现 ASCII 分号**——切分点会落在注释中间，
//! 剩余片段不以 `--` 开头，会被当成可执行 SQL。

use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, migration_state, DbPool};
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const INIT_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V001__init.sql");
const V002_SQL_MYSQL: &str = include_str!("../migrations/mysql/V002__file_tenant_reconcile.sql");
const V002_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V002__file_tenant_reconcile.sql");
const V003_SQL_MYSQL: &str = include_str!("../migrations/mysql/V003__align_bigint_signed.sql");
const V003_SQL_POSTGRES: &str =
    include_str!("../migrations/postgres/V003__align_bigint_signed.sql");

const SCHEMA_MARKER_TABLE: &str = "file_files";
const V002_MARKER_TABLE: &str = "file_migration_v002_applied";
const V003_MARKER_TABLE: &str = "file_migration_v003_applied";

/// 按 URL 判定方言并建池（PG 分支由 `init_pool_with` 锁定会话时区为 UTC）。
pub async fn init_pool(database_url: &str) -> anyhow::Result<DbPool> {
    digua_common_db::init_pool_with(database_url, 10)
        .await
        .map_err(|e| anyhow::anyhow!("DB_CONNECT: {e}"))
}

/// 取目标方言的迁移脚本。
fn script_for(dialect: DialectKind, mysql: &'static str, postgres: &'static str) -> &'static str {
    match dialect {
        DialectKind::Postgres => postgres,
        _ => mysql,
    }
}

/// marker 哨兵表的 DDL（方言相关：PG 的时间列必须 TIMESTAMPTZ）。
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

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    let dialect = pool.dialect();

    if !table_exists(pool, SCHEMA_MARKER_TABLE).await? {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            dialect = ?dialect,
            "file-service schema missing, applying V001"
        );
        apply_script(
            pool,
            script_for(dialect, INIT_SQL_MYSQL, INIT_SQL_POSTGRES),
            "V001",
        )
        .await?;
        info!("file-service V001 applied");
    } else {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            "file-service schema present, skip V001"
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
        db_execute!(pool, marker_ddl(dialect, V002_MARKER_TABLE))?;
        info!("file-service V002 applied");
    } else {
        info!(marker = V002_MARKER_TABLE, "V002 already applied, skip");
    }

    // V003：MySQL 侧把 tenant_id 改为**有符号** BIGINT（PG 侧是同版本号的空操作文件）。
    // 两侧都建哨兵表，保证 marker 语义一致（PG 侧虽是空操作，仍应记录已应用）。
    if !table_exists(pool, V003_MARKER_TABLE).await? {
        info!(marker = V003_MARKER_TABLE, "applying V003");
        apply_script(
            pool,
            script_for(dialect, V003_SQL_MYSQL, V003_SQL_POSTGRES),
            "V003",
        )
        .await?;
        db_execute!(pool, marker_ddl(dialect, V003_MARKER_TABLE))?;
        info!("file-service V003 applied");
    } else {
        info!(marker = V003_MARKER_TABLE, "V003 already applied, skip");
    }

    Ok(())
}

/// 逐条执行迁移脚本（与历史行为一致：先按 `;` 切分，再剥离注释行）。
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
            warn!(statement = %first_line(s), error = %e, "{} statement failed", label);
            return Err(anyhow::anyhow!("apply {label} ({}): {e}", first_line(s)));
        }
    }
    Ok(())
}

async fn table_exists(pool: &DbPool, table: &str) -> anyhow::Result<bool> {
    // 统一走基座层入口：PG 用 current_schema()，MySQL 用 DATABASE()。
    migration_state::probe_marker(pool, table)
        .await
        .map_err(|e| anyhow::anyhow!("probe marker {table}: {e}"))
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}
