//! prue_backend 的方言无关 DB 层（MySQL / PostgreSQL / SQLite 三方言）。
//!
//! marker 探测统一走 `digua_common_db::migration_state::probe_marker`
//! （内部按后端选 `information_schema` / `sqlite_master` / `current_schema()`）。
//!
//! ⚠️ 执行器是「先按 ASCII 分号切分、再剥离 `--` 注释行」，
//! 因此**迁移文件的注释里不能出现 ASCII 分号**——切分点会落在注释中间，
//! 剩余片段不以 `--` 开头，会被当成可执行 SQL。

use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, migration_state, DbPool};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

const INIT_SQL_MYSQL: &str = include_str!("../migrations/mysql/V001__init.sql");
const INIT_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V001__init.sql");
const INIT_SQL_SQLITE: &str = include_str!("../migrations/sqlite/V001__init.sql");
const SCHEMA_MARKER_TABLE: &str = "prue_menu_configs";

/// 取目标方言的初始化脚本。
fn script_for(dialect: DialectKind) -> &'static str {
    match dialect {
        DialectKind::Postgres => INIT_SQL_POSTGRES,
        DialectKind::Sqlite => INIT_SQL_SQLITE,
        _ => INIT_SQL_MYSQL,
    }
}

pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    let dialect = pool.dialect();
    let exists = migration_state::probe_marker(pool, SCHEMA_MARKER_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("probe marker {SCHEMA_MARKER_TABLE}: {e}"))?;

    if !exists {
        info!(
            marker = SCHEMA_MARKER_TABLE,
            dialect = ?dialect,
            "prue_backend schema missing, applying initial migration"
        );
        for s in split_sql(script_for(dialect)) {
            // 借用而非搬移：`s` 之后还要用于日志。
            // （满足 `db_execute!` 的 `$sql` 生命周期要求：指向**存活的局部变量**的 `&str` ✅）
            let stmt: &str = s.as_str();
            if let Err(e) = db_execute!(pool, stmt) {
                warn!(statement = %stmt, error = %e, "prue_backend migration statement failed");
                return Err(anyhow::anyhow!("apply prue_backend migration: {e}"));
            }
        }
        info!("prue_backend initial migration applied");
    } else {
        info!(marker = SCHEMA_MARKER_TABLE, "prue_backend schema present");
    }

    Ok(())
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
