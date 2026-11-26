//! token-service 的方言无关 DB 层（MySQL / PostgreSQL）。
//!
//! # 本轮改造要点
//!
//! 1. **删掉本 crate 自建的 `DbPool` 枚举**（历史遗留的重复定义，只有 `Mysql` 一个变体），
//!    统一使用 `digua_common_db::DbPool`。为不打断调用方（含 5 个测试文件）的
//!    `use crate::db::DbPool` / `token_service::db::DbPool` 路径，这里用 `pub use` 重导出
//!    同一类型——**类型只有一份**，只是路径保持稳定。
//! 2. 迁移脚本按方言选择（`migrations/mysql/` vs `migrations/postgres/`），
//!    marker 探测统一走 `migration_state::probe_marker`（内部按后端选
//!    `information_schema` / `current_schema()`）。
//! 3. 语句执行走 `db_execute!`（宏负责 `?` → `$N` 的占位符改写与 bind 计数校验）。
//!
//! ⚠️ 执行器是「先按 ASCII 分号切分、再剥离 `--` 注释行」，
//! 因此**迁移文件的注释里不能出现 ASCII 分号**。

use anyhow::{Context, Result};
use digua_common_db::dialect::DialectKind;
use digua_common_db::{db_execute, db_fetch_all, migration_state};

/// 重导出基座层的方言无关连接池。
///
/// 本 crate 曾自建过一个只有 `Mysql` 变体的同名枚举，本轮删除并统一到基座层。
/// 这里用 `pub use` 而不是让调用方改用 `digua_common_db::DbPool`，是为了让
/// `crate::db::DbPool` / `token_service::db::DbPool` 这些既有路径继续可用——
/// **类型只有一份**，只是导入路径保持稳定。
pub use digua_common_db::DbPool;

use std::time::Duration;
use tracing::{info, warn};

use crate::config::DatabaseCfg;

// 会话时区锁 UTC 只对 MySQL 分支需要（PG 分支由基座层的 after_connect 负责）。
#[cfg(feature = "mysql")]
use sqlx::mysql::MySqlPoolOptions;

/// 单个迁移：版本号 + 双方言 SQL + marker table + 可选 Rust 迁移步。
///
/// `rust_step`（WP-02-R2）：在 SQL 脚本**之前**执行的 Rust 迁移逻辑，用于
/// 纯 SQL 无法在「分号切分 + 逐条自动提交」执行语义下表达的状态感知逻辑
/// （条件 DDL、明确错误信息、事务性 DDL）。约定：
/// - 失败必须返回 `Err`：迁移停机、哨兵 marker 不建立、正式数据不动；
/// - 重试安全：任意中断后重新执行，函数必须识别当前状态并安全继续，
///   不允许留下影响重试的持久残留（WP-02-R2 移除了 guard 表方案，
///   从根本上消除了临时对象残留）。
pub struct Migration {
    /// 版本号，`V001` / `V002` / ...
    pub version: &'static str,
    /// 该迁移的 MySQL 脚本（`migrations/mysql/`）。
    pub sql_mysql: &'static str,
    /// 该迁移的 PostgreSQL 脚本（`migrations/postgres/`）。
    pub sql_postgres: &'static str,
    /// 判定该迁移是否已应用的 marker table。
    pub marker_table: &'static str,
    /// 可选 Rust 迁移步（先于 SQL 脚本执行）。
    pub rust_step: Option<RustMigrationStep>,
}

impl Migration {
    /// 按方言取脚本。
    pub fn sql_for(&self, dialect: DialectKind) -> &'static str {
        match dialect {
            DialectKind::Postgres => self.sql_postgres,
            _ => self.sql_mysql,
        }
    }
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
        sql_mysql: include_str!("../migrations/mysql/V001__init.sql"),
        sql_postgres: include_str!("../migrations/postgres/V001__init.sql"),
        marker_table: "token_oauth_clients",
        rust_step: None,
    },
    Migration {
        version: "V002",
        sql_mysql: include_str!("../migrations/mysql/V002__add_client_user_and_expiry.sql"),
        sql_postgres: include_str!("../migrations/postgres/V002__add_client_user_and_expiry.sql"),
        marker_table: "token_migration_v002_applied",
        rust_step: None,
    },
    Migration {
        version: "V003",
        sql_mysql: include_str!("../migrations/mysql/V003__bootstrap_tokens.sql"),
        sql_postgres: include_str!("../migrations/postgres/V003__bootstrap_tokens.sql"),
        marker_table: "token_migration_v003_applied",
        rust_step: None,
    },
    Migration {
        version: "V004",
        sql_mysql: include_str!("../migrations/mysql/V004__service_jwt_grants.sql"),
        sql_postgres: include_str!("../migrations/postgres/V004__service_jwt_grants.sql"),
        marker_table: "token_migration_v004_applied",
        rust_step: None,
    },
    // V005 给 token_service_jwt_grants 兜底复合唯一约束（WP-02-R1/R2）：
    // V004 首版（1983c2bb）的 PG 脚本漏掉主键。状态感知逻辑（重复检测
    // fail-closed + 主键三分支判定 + 事务性 DDL）在 rust_step 中实现；
    // SQL 文件只做旧版 guard 残留清理与 marker 哨兵表。R2 修复：移除
    // guard 表复制方案（中断残留会使重试永久失败）与无条件 DROP PRIMARY KEY
    // （主键已删未建时中断会使重试永久失败）。
    Migration {
        version: "V005",
        sql_mysql: include_str!("../migrations/mysql/V005__service_jwt_grants_unique.sql"),
        sql_postgres: include_str!("../migrations/postgres/V005__service_jwt_grants_unique.sql"),
        marker_table: "token_migration_v005_applied",
        rust_step: Some(RustMigrationStep::V005GrantPk),
    },
    // V006（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）：grant 管理审计表。
    // 纯 CREATE TABLE IF NOT EXISTS 幂等脚本，无需 rust_step。
    Migration {
        version: "V006",
        sql_mysql: include_str!("../migrations/mysql/V006__service_jwt_grant_audit.sql"),
        sql_postgres: include_str!("../migrations/postgres/V006__service_jwt_grant_audit.sql"),
        marker_table: "token_migration_v006_applied",
        rust_step: None,
    },
];

/// 需要状态感知逻辑的迁移步（WP-02-R2）。纯 SQL 无法在「分号切分 +
/// 逐条自动提交」的执行语义下表达条件 DDL、明确错误信息与事务性 DDL。
#[derive(Debug, Clone, Copy)]
pub enum RustMigrationStep {
    /// V005：重复检测 fail-closed + grant 主键三分支判定（双方言）。
    V005GrantPk,
}

/// 分发 Rust 迁移步。约定：失败即 `Err`（迁移停机、marker 不建立、正式数据
/// 不动、可重试）；成功不建立 marker（marker 由后续 SQL 脚本的哨兵表建立）。
///
/// 公开给集成测试复用：中断恢复测试按生产语义单独驱动某一步。
pub async fn run_rust_step(pool: &DbPool, step: RustMigrationStep) -> Result<()> {
    match step {
        RustMigrationStep::V005GrantPk => v005_grant_pk_migration(pool).await,
    }
}

/// WP-02-R2：V005 状态感知迁移逻辑（双方言）。
///
/// 第一步·重复检测（fail-closed）：
///   按 `(owner, audience)` 分组，存在 `COUNT(*) > 1` → `Err`，错误信息含
///   每一组的 owner / audience / 数量。正式数据一律不动（不删除、不合并、
///   不覆盖），由人工裁决；marker 不建立。
///
/// 第二步·主键状态判定：
///   - 已正确存在 `PRIMARY KEY (owner, audience)` → no-op，不执行破坏性 DDL；
///   - 主键不存在 → 建立复合主键（PG 走事务性 DDL；MySQL 8.0 单条 DDL 原子）；
///   - 主键存在但定义错误 → `Err` 并报告当前定义，不删除未知约束。
///
/// 成功路径无持久残留（R1 的 guard 表方案已移除），任意中断点重试安全。
pub async fn v005_grant_pk_migration(pool: &DbPool) -> Result<()> {
    let dups: Vec<GrantDupGroup> = db_fetch_all!(
        pool,
        GrantDupGroup,
        "SELECT owner, audience, COUNT(*) AS cnt FROM token_service_jwt_grants \
         GROUP BY owner, audience HAVING COUNT(*) > 1"
    )?;
    if !dups.is_empty() {
        let detail = dups
            .iter()
            .map(|d| format!("({}, {}) x{}", d.owner, d.audience, d.cnt))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow::anyhow!(
            "token_service_jwt_grants contains duplicate (owner, audience) rows: {detail}; \
             migration aborted fail-closed — no rows were modified, \
             resolve the duplicates manually and retry"
        ));
    }
    match pool {
        #[cfg(feature = "mysql")]
        DbPool::Mysql(p) => v005_rebuild_pk_mysql(p).await,
        #[cfg(feature = "postgres")]
        DbPool::Postgres(p) => v005_rebuild_pk_postgres(p).await,
        #[allow(unreachable_patterns)]
        _ => Err(anyhow::anyhow!("V005 unsupported dialect")),
    }
}

/// 重复检测行（双方言：PG `COUNT(*)` 为 int8、MySQL 为 bigint，均解 i64）。
#[derive(Debug, sqlx::FromRow)]
struct GrantDupGroup {
    owner: String,
    audience: String,
    cnt: i64,
}

/// V005 期望的主键定义（PG `pg_get_constraintdef` 的规范化输出）。
#[cfg(feature = "postgres")]
const V005_PK_DEF: &str = "PRIMARY KEY (owner, audience)";

/// PG 分支：按 `pg_constraint` 当前主键判定，事务性 DDL 建立缺失主键。
#[cfg(feature = "postgres")]
async fn v005_rebuild_pk_postgres(pg: &sqlx::PgPool) -> Result<()> {
    let pk: Option<(String,)> = sqlx::query_as(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'token_service_jwt_grants'::regclass AND contype = 'p' LIMIT 1",
    )
    .fetch_optional(pg)
    .await?;
    match pk {
        // 主键不存在 → 建立（事务性 DDL：ALTER 原子提交，中断即回滚）
        None => {
            let mut tx = pg.begin().await?;
            sqlx::query("ALTER TABLE token_service_jwt_grants ADD PRIMARY KEY (owner, audience)")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        // 已正确存在 → 不做任何破坏性 DDL
        Some((def,)) if def == V005_PK_DEF => {
            tracing::info!("V005: grant pk already correct, no DDL executed");
        }
        // 定义错误 → 明确失败并报告当前定义，不删除未知约束
        Some((def,)) => {
            return Err(anyhow::anyhow!(
                "token_service_jwt_grants has an unexpected primary key `{def}`; \
                 expected `{V005_PK_DEF}` — refusing to drop an unknown constraint"
            ));
        }
    }
    Ok(())
}

/// MySQL 分支：按 `information_schema.STATISTICS` 当前主键列判定。
/// MySQL DDL 隐式提交，不依赖迁移级事务；建立主键是单条原子 DDL，
/// 中断不产生中间态，重试按当前状态重新判定。
#[cfg(feature = "mysql")]
async fn v005_rebuild_pk_mysql(p: &sqlx::MySqlPool) -> Result<()> {
    #[derive(Debug, sqlx::FromRow)]
    struct PkCol {
        column_name: String,
    }
    let cols: Vec<PkCol> = sqlx::query_as::<_, PkCol>(
        "SELECT COLUMN_NAME AS column_name FROM information_schema.STATISTICS \
             WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'token_service_jwt_grants' \
               AND INDEX_NAME = 'PRIMARY' ORDER BY SEQ_IN_INDEX",
    )
    .fetch_all(p)
    .await?;
    let names: Vec<&str> = cols.iter().map(|c| c.column_name.as_str()).collect();
    match names.as_slice() {
        // 主键不存在（含 R1 版 DROP 后中断的存量库）→ 建立复合主键
        [] => {
            sqlx::query("ALTER TABLE token_service_jwt_grants ADD PRIMARY KEY (owner, audience)")
                .execute(p)
                .await?;
        }
        // 已正确存在 → 不做任何破坏性 DDL（不再无条件 DROP PRIMARY KEY）
        ["owner", "audience"] => {
            tracing::info!("V005: grant pk already correct, no DDL executed");
        }
        // 定义错误 → 明确失败并报告当前定义，不删除未知约束
        other => {
            return Err(anyhow::anyhow!(
                "token_service_jwt_grants has an unexpected primary key ({}); \
                 expected (owner, audience) — refusing to drop an unknown constraint",
                other.join(", ")
            ));
        }
    }
    Ok(())
}

/// 建池。
///
/// - **PostgreSQL**：直接委托 `digua_common_db::init_pool_with`——它的 PG 分支已用
///   `after_connect` 把会话时区锁成 UTC。
/// - **MySQL**：保留本服务历史上的 `SET time_zone = '+00:00'`（基座层的 MySQL 分支
///   没有 `after_connect` 钩子）。不锁 UTC 会让 `TIMESTAMP` 列的编解码随服务器时区漂移，
///   `expires_at > NOW()` 这类判断虽然自洽，但 `DateTime<Utc>` 解出来会整体偏移。
pub async fn init_pool(cfg: &DatabaseCfg) -> Result<DbPool> {
    let dialect = DialectKind::from_url(&cfg.url)
        .ok_or_else(|| anyhow::anyhow!("unsupported database URL scheme: {}", cfg.url))?;
    match dialect {
        #[cfg(feature = "mysql")]
        DialectKind::Mysql => {
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
        DialectKind::Postgres => digua_common_db::init_pool_with(&cfg.url, cfg.max_connections)
            .await
            .map_err(|e| anyhow::anyhow!("connect postgres {}: {e}", cfg.url)),
        other => anyhow::bail!("unsupported database dialect: {other:?}"),
    }
}

/// 按序执行 migrations，每个迁移按自己的 marker table 独立判定是否已应用，
/// 因此存量库（V001 的 marker 已在、V002 的哨兵表还不在）上 V002 仍会被执行。
pub async fn init_schema(pool: &DbPool) -> Result<()> {
    let dialect = pool.dialect();
    for m in MIGRATIONS {
        let exists = migration_state::probe_marker(pool, m.marker_table)
            .await
            .map_err(|e| anyhow::anyhow!("probe marker {}: {e}", m.marker_table))?;
        if exists {
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
            dialect = ?dialect,
            "applying migration"
        );
        // Rust 迁移步先于 SQL 脚本执行（WP-02-R2）：失败即停机，哨兵 marker
        // 未建立；重试时 marker 仍缺失 → 从头重新评估当前状态。
        if let Some(step) = m.rust_step {
            info!(version = m.version, step = ?step, "running rust migration step");
            run_rust_step(pool, step)
                .await
                .map_err(|e| anyhow::anyhow!("rust migration step {}: {e}", m.version))?;
        }
        apply_sql_script(pool, m.version, m.sql_for(dialect)).await?;
    }
    Ok(())
}

/// 启动时确保 schema 就位；迁移失败则启动失败（不做静默降级）。
pub async fn ensure_schema(pool: &DbPool) -> Result<()> {
    if let Err(e) = init_schema(pool).await {
        warn!(error = %e, "migration failed");
        return Err(e.context("apply migrations"));
    }
    Ok(())
}

/// 执行一份迁移脚本。
///
/// 「表已存在」按方言忽略，保证重启幂等：MySQL `42S01`、PostgreSQL `42P07`。
async fn apply_sql_script(pool: &DbPool, version: &str, script: &str) -> Result<()> {
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
        if let Err(e) = db_execute!(pool, s) {
            if let Some(db) = e.as_database_error() {
                // 42S01 = MySQL "table already exists" / 42P07 = PostgreSQL "duplicate_table"
                if matches!(db.code().as_deref(), Some("42S01") | Some("42P07")) {
                    continue;
                }
            }
            return Err(anyhow::anyhow!("apply {version} ({}): {e}", first_line(s)));
        }
    }
    Ok(())
}

/// 判断一个 `sqlx::Error` 是否为**唯一键/主键冲突**。
///
/// 用于把「幂等插入」写成一份跨方言 SQL：
/// - MySQL 的 `INSERT IGNORE` 在 PG 没有对应语法（`ON CONFLICT` 又是 PG 专有），
///   与其写两份 SQL，不如统一写裸 `INSERT` 并在此处把重复键错误吞掉；
/// - SQLSTATE：MySQL `23000`（integrity constraint violation）、
///   PostgreSQL `23505`（unique_violation）。
pub fn is_duplicate_key(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .and_then(|d| d.code())
        .map(|c| c == "23000" || c == "23505")
        .unwrap_or(false)
}

fn first_line(s: &str) -> String {
    s.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .unwrap_or_else(|| "<empty>".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_sql_contains_all_marker_tables() {
        for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
            let upper = MIGRATIONS[0].sql_for(dialect).to_uppercase();
            for tbl in [
                "TOKEN_OAUTH_CLIENTS",
                "TOKEN_AUTH_CODES",
                "TOKEN_REVOKED_JTIS",
                "TOKEN_REFRESH_MARKERS",
            ] {
                assert!(
                    upper.contains(tbl),
                    "missing CREATE TABLE for {tbl} in {dialect:?} script"
                );
            }
        }
    }

    #[test]
    fn migrations_are_versioned_in_order() {
        let versions: Vec<&str> = MIGRATIONS.iter().map(|m| m.version).collect();
        assert_eq!(versions, vec!["V001", "V002", "V003", "V004", "V005", "V006"]);
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
        for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
            assert!(
                v002.sql_for(dialect)
                    .contains("CREATE TABLE IF NOT EXISTS token_migration_v002_applied"),
                "{dialect:?} script must create the V002 sentinel table"
            );
        }
    }

    /// 与 `apply_script` 同口径：剥离 `--` 注释行后再检查——注释永远不会到达
    /// PG 执行器，迁移文件头部的转换规则说明（含 "ON UPDATE CURRENT_TIMESTAMP"
    /// 等字样）不构成残留。
    fn executable_sql(script: &str) -> String {
        script
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_uppercase()
    }

    /// PG 脚本必须用 TIMESTAMPTZ / JSONB，且不得残留 MySQL 专有语法。
    #[test]
    fn postgres_scripts_use_pg_types_only() {
        for m in MIGRATIONS {
            let sql = executable_sql(m.sql_for(DialectKind::Postgres));
            for forbidden in [
                "ON UPDATE CURRENT_TIMESTAMP",
                "ENGINE=",
                "AUTO_INCREMENT",
                "TINYINT",
            ] {
                assert!(
                    !sql.contains(forbidden),
                    "{} PG script still contains `{forbidden}`",
                    m.version
                );
            }
            assert!(
                sql.contains("TIMESTAMPTZ"),
                "{} PG script must use TIMESTAMPTZ",
                m.version
            );
        }
        // 含 json 列的那一份必须是 JSONB（sqlx 的 Json<T> 以 JSONB 绑定）。
        let v001 = MIGRATIONS[0].sql_for(DialectKind::Postgres).to_uppercase();
        assert!(v001.contains("JSONB"), "V001 PG 的 json 列必须是 JSONB");
    }

    /// WP-02-R1/R2：grant 主键契约 —— V004 双方言脚本必须声明
    /// `PRIMARY KEY (owner, audience)`；V005 的约束修复走 rust_step
    /// （状态感知），其 SQL 文件不得再包含 guard 复制方案、无条件
    /// DROP PRIMARY KEY 或任何数据复制语句。
    #[test]
    fn grant_pk_contract_holds_across_dialects_and_v005_is_state_aware() {
        for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
            let v004 = MIGRATIONS
                .iter()
                .find(|m| m.version == "V004")
                .unwrap_or_else(|| panic!("migration V004 missing"));
            assert!(
                executable_sql(v004.sql_for(dialect)).contains("PRIMARY KEY (OWNER, AUDIENCE)"),
                "V004 {dialect:?} script must declare PRIMARY KEY (owner, audience)"
            );

            let v005 = MIGRATIONS
                .iter()
                .find(|m| m.version == "V005")
                .unwrap_or_else(|| panic!("migration V005 missing"));
            assert!(
                matches!(v005.rust_step, Some(RustMigrationStep::V005GrantPk)),
                "V005 must carry the state-aware rust migration step"
            );
            let sql = executable_sql(v005.sql_for(dialect));
            assert!(
                !sql.contains("INSERT INTO"),
                "V005 {dialect:?} SQL must not copy grant rows (guard scheme removed)"
            );
            assert!(
                !sql.contains("DROP PRIMARY KEY"),
                "V005 {dialect:?} SQL must not unconditionally drop the pk"
            );
            assert!(
                sql.contains("TOKEN_MIGRATION_V005_APPLIED"),
                "V005 {dialect:?} SQL must create its sentinel marker"
            );
        }
    }

    /// 迁移文件的注释里不能有 ASCII 分号，否则切分器会把注释切成可执行 SQL。
    #[test]
    fn migration_comments_have_no_ascii_semicolons() {
        for m in MIGRATIONS {
            for dialect in [DialectKind::Mysql, DialectKind::Postgres] {
                for line in m.sql_for(dialect).lines() {
                    if line.trim_start().starts_with("--") {
                        assert!(
                            !line.contains(';'),
                            "{} {:?} comment has an ASCII semicolon: {line}",
                            m.version,
                            dialect
                        );
                    }
                }
            }
        }
    }
}
