//! WP-LMMS-SERVICE-JWT-AUDIENCE-02-R2：V005 迁移中断恢复验证（MySQL）。
//!
//! 与 pg_grant_unique_test.rs 对等，覆盖任务书测试矩阵的 MySQL 半边：
//! - V004 → V005 正常执行（含 R1 版 guard 残留清理）
//! - V005 重复执行 → no-op，不破坏数据
//! - R1 版中断残留态（主键已 DROP 未 ADD）→ 重试建立正确主键
//! - 主键定义错误 → 明确失败并报告当前定义，不删除未知约束
//! - 重复数据 fail-closed：错误含 owner/audience/数量、数据完整保留、
//!   marker 不建立；人工处理后重试成功
//! - DDL 后 / marker 建立前中断 → 重试识别状态并安全完成
//!
//! 全部走真实执行器语义：rust_step（生产函数）+ 逐条 SQL 自动提交
//! （无迁移级事务掩盖中断问题）。`TEST_MYSQL_HOST/PORT` 未设置时回退
//! testcontainers（本环境不可用，本地跑共享容器模式）。

#![cfg(feature = "mysql")]

use token_service::db::{run_rust_step, DbPool, RustMigrationStep, MIGRATIONS};

mod common;

/// 首版 V004（commit 1983c2bb）的 MySQL 建表语句：**无** PRIMARY KEY。
const LEGACY_V004_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  INDEX idx_token_service_jwt_grants_audience (audience)
);
CREATE TABLE IF NOT EXISTS token_migration_v004_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;

/// 带单列主键（定义错误）的建表语句，用于验证 fail-closed。
const WRONG_PK_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (owner)
);
"#;

/// R1 版 V005 中断后可能遗留的 guard 表。
const LEGACY_GUARD_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS token_v005_grant_dup_guard (
  owner    VARCHAR(64) NOT NULL,
  audience VARCHAR(64) NOT NULL,
  UNIQUE KEY uq_token_v005_dup_guard (owner, audience)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
INSERT INTO token_v005_grant_dup_guard (owner, audience)
SELECT owner, audience FROM token_service_jwt_grants;
"#;

fn mysql(pool: &DbPool) -> &sqlx::MySqlPool {
    pool.as_mysql().expect("mysql pool")
}

/// 手工跑 V001-V004（rust_step + SQL，与生产同序），供构造迁移前状态。
async fn run_migrations_except_v005(pool: &DbPool) {
    let p = mysql(pool);
    for m in MIGRATIONS {
        if m.version == "V005" {
            continue;
        }
        if let Some(step) = m.rust_step {
            run_rust_step(pool, step)
                .await
                .expect("rust migration step");
        }
        common_apply_script(p, m.sql_mysql).await;
    }
}

/// 与生产 `apply_sql_script` 同口径：剥注释 + 分号切分、逐条自动提交。
async fn common_apply_script(p: &sqlx::MySqlPool, script: &str) {
    let cleaned = script
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n");
    for stmt in cleaned.split(';') {
        let s = stmt.trim();
        if s.is_empty() {
            continue;
        }
        sqlx::query(s).execute(p).await.unwrap_or_else(|e| {
            panic!("apply mysql stmt `{}`: {e}", s.lines().next().unwrap_or(""))
        });
    }
}

async fn v005_rust_step(pool: &DbPool) -> Result<(), anyhow::Error> {
    run_rust_step(pool, RustMigrationStep::V005GrantPk).await
}

async fn v005_sql(pool: &DbPool) {
    let v005 = MIGRATIONS
        .iter()
        .find(|m| m.version == "V005")
        .expect("V005 registered");
    common_apply_script(mysql(pool), v005.sql_mysql).await;
}

async fn insert_legacy_grant(pool: &DbPool, owner: &str, audience: &str) {
    sqlx::query(
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', TRUE)",
    )
    .bind(owner)
    .bind(audience)
    .execute(mysql(pool))
    .await
    .expect("insert legacy grant");
}

async fn pk_columns(pool: &DbPool) -> Vec<String> {
    let cols: Vec<(String,)> = sqlx::query_as(
        "SELECT COLUMN_NAME FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'token_service_jwt_grants' \
           AND INDEX_NAME = 'PRIMARY' ORDER BY SEQ_IN_INDEX",
    )
    .fetch_all(mysql(pool))
    .await
    .expect("query pk columns");
    cols.into_iter().map(|c| c.0).collect()
}

async fn grant_total(pool: &DbPool) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM token_service_jwt_grants")
        .fetch_one(mysql(pool))
        .await
        .expect("count grants");
    n
}

async fn sentinel_exists(pool: &DbPool) -> bool {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = 'token_migration_v005_applied'",
    )
    .fetch_one(mysql(pool))
    .await
    .expect("probe sentinel");
    n > 0
}

async fn guard_exists(pool: &DbPool) -> bool {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = 'token_v005_grant_dup_guard'",
    )
    .fetch_one(mysql(pool))
    .await
    .expect("probe guard");
    n > 0
}

// ── V004 → V005 正常执行（含 R1 guard 残留清理）───────────────────────────

#[tokio::test]
async fn mysql_v004_to_v005_normal_with_guard_residue() {
    let pool = common::spawn_mysql_unmigrated().await;
    run_migrations_except_v005(&pool).await;
    for (owner, audience) in [
        ("iam-service", "token-service"),
        ("notification-service", "iam-service"),
    ] {
        insert_legacy_grant(&pool, owner, audience).await;
    }
    // R1 版中断残留：guard 表已建且已复制数据
    common_apply_script(mysql(&pool), LEGACY_GUARD_SQL).await;
    assert!(guard_exists(&pool).await);

    v005_rust_step(&pool)
        .await
        .expect("rust step must succeed on a clean legacy table");
    v005_sql(&pool).await;

    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string(), "audience".to_string()],
        "V005 must establish the composite primary key"
    );
    assert_eq!(grant_total(&pool).await, 2, "data must be preserved");
    assert!(sentinel_exists(&pool).await, "marker must be established");
    assert!(
        !guard_exists(&pool).await,
        "legacy guard table must be dropped"
    );

    // 重跑 V005 幂等
    v005_rust_step(&pool).await.expect("re-run rust step");
    v005_sql(&pool).await;
    assert_eq!(grant_total(&pool).await, 2);
}

// ── V005 重复执行 → no-op，不破坏数据 ─────────────────────────────────────

#[tokio::test]
async fn mysql_v005_rerun_is_noop_when_pk_correct() {
    let pool = common::spawn_mysql().await;

    let before: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT owner, audience, status FROM token_service_jwt_grants ORDER BY owner, audience",
    )
    .fetch_all(mysql(&pool))
    .await
    .expect("list before");

    v005_rust_step(&pool)
        .await
        .expect("rust step on a correct pk must be a no-op");

    let after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT owner, audience, status FROM token_service_jwt_grants ORDER BY owner, audience",
    )
    .fetch_all(mysql(&pool))
    .await
    .expect("list after");
    assert_eq!(before, after, "re-run must not modify any grant row");
    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string(), "audience".to_string()]
    );
}

// ── R1 版中断残留态（主键已 DROP 未 ADD）→ 重试建立正确主键 ───────────────

#[tokio::test]
async fn mysql_v005_pk_missing_rebuilds_on_retry() {
    let pool = common::spawn_mysql_unmigrated().await;
    common_apply_script(mysql(&pool), LEGACY_V004_SQL).await;
    insert_legacy_grant(&pool, "iam-service", "token-service").await;
    insert_legacy_grant(&pool, "notification-service", "iam-service").await;

    assert!(
        pk_columns(&pool).await.is_empty(),
        "legacy table must start without a pk"
    );

    v005_rust_step(&pool)
        .await
        .expect("rust step must add the pk");
    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string(), "audience".to_string()],
        "retry must establish the composite primary key"
    );
    assert_eq!(grant_total(&pool).await, 2);
}

// ── 主键定义错误 → 明确失败并报告当前定义 ─────────────────────────────────

#[tokio::test]
async fn mysql_v005_unexpected_pk_fails_without_dropping() {
    let pool = common::spawn_mysql_unmigrated().await;
    common_apply_script(mysql(&pool), WRONG_PK_SQL).await;

    let err = v005_rust_step(&pool)
        .await
        .expect_err("wrong pk definition must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("owner") && msg.contains("refusing"),
        "error must report the current definition, got: {msg}"
    );
    // 未知约束未被删除
    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string()],
        "unknown constraint must be left untouched"
    );
}

// ── 重复数据 fail-closed + 人工处理后重试成功 ─────────────────────────────

#[tokio::test]
async fn mysql_v005_fails_closed_on_duplicate_rows() {
    let pool = common::spawn_mysql_unmigrated().await;
    common_apply_script(mysql(&pool), LEGACY_V004_SQL).await;
    for _ in 0..2 {
        insert_legacy_grant(&pool, "svc-dup", "aud-dup").await;
    }

    let err = v005_rust_step(&pool)
        .await
        .expect_err("V005 must fail on duplicate (owner, audience) rows");
    let msg = err.to_string();
    assert!(
        msg.contains("(svc-dup, aud-dup) x2"),
        "error must contain the duplicate owner, audience and count, got: {msg}"
    );

    // 重复行原样保留，交由人工裁决——绝不静默删行。
    let (kept,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = 'svc-dup'")
            .fetch_one(mysql(&pool))
            .await
            .expect("count rows");
    assert_eq!(kept, 2, "duplicate rows must not be silently removed");

    // 失败迁移不得留下成功 marker。
    assert!(
        !sentinel_exists(&pool).await,
        "a failed migration must not record a success marker"
    );

    // 人工裁决：保留一行（模拟人工 DELETE 决策）→ 重试成功。
    sqlx::query("DELETE FROM token_service_jwt_grants WHERE owner = 'svc-dup' LIMIT 1")
        .execute(mysql(&pool))
        .await
        .expect("manual resolution");

    v005_rust_step(&pool).await.expect("retry after manual fix");
    v005_sql(&pool).await;
    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string(), "audience".to_string()],
        "retry after manual resolution must establish the pk"
    );
}

// ── DDL 后 / marker 建立前中断 → 重试识别状态并安全完成 ──────────────────

#[tokio::test]
async fn mysql_v005_retry_when_marker_missing() {
    let pool = common::spawn_mysql_unmigrated().await;
    common_apply_script(mysql(&pool), LEGACY_V004_SQL).await;
    insert_legacy_grant(&pool, "iam-service", "token-service").await;
    insert_legacy_grant(&pool, "notification-service", "iam-service").await;

    // 模拟中断点：rust_step（DDL）已完成，但 SQL 脚本（哨兵 marker）未执行。
    v005_rust_step(&pool).await.expect("rust step");
    assert!(
        !sentinel_exists(&pool).await,
        "marker must not exist before the SQL script runs"
    );

    // 重启重试：rust_step 重新评估（no-op）+ SQL 建立 marker → 安全完成。
    v005_rust_step(&pool).await.expect("retry rust step");
    v005_sql(&pool).await;

    assert!(
        sentinel_exists(&pool).await,
        "marker must be established on retry"
    );
    assert_eq!(grant_total(&pool).await, 2);
    assert_eq!(
        pk_columns(&pool).await,
        vec!["owner".to_string(), "audience".to_string()]
    );
}
