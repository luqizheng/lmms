//! WP-LMMS-SERVICE-JWT-AUDIENCE-02-R1/R2：grant 表唯一性 / 并发 / 升级 /
//! 中断恢复验证（PostgreSQL）。
//!
//! 覆盖实施任务书测试矩阵：
//! 1. 相同 owner+audience 只能插入一条
//! 2. 不同 audience 可以分别授权
//! 3. 并发 ensure_system_grants 不产生重复行
//! 4. 重复 seed 后结果幂等
//! 5. 已禁用的系统 grant 不被启动 seed 复活
//! 6. 已有 V004（首版无主键）数据库可以通过 V005 升级
//! 7. PG 半边：主键必须恰为 (owner, audience)
//!    （MySQL 半边在 service_jwt_grant_test.rs::mysql_grant_pk_is_owner_audience）
//! R2 中断恢复（真实执行器逐条语义，无迁移级事务掩盖）：
//! 8. 主键缺失时重试 → 建立正确主键
//! 9. 主键定义错误 → 明确失败并报告当前定义，不删除未知约束
//! 10. 主键已正确存在时重试 → no-op，数据不变
//! 11. DDL 后 / marker 建立前中断 → 重试识别状态并安全完成
//! 12. 重复数据 fail-closed：错误含 owner/audience/数量、数据完整保留、
//!     marker 不建立；人工处理后重试成功
//! 13. R1 版 guard 表残留被 V005 SQL 清理
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用）。
//! 无该变量时打印 `[skip]` 并直接返回，不失败。

#![cfg(feature = "postgres")]

use digua_common_db::db_execute;
use digua_common_db::dialect::DialectKind;
use token_service::db::{self, is_duplicate_key, DbPool, RustMigrationStep};
use token_service::models::service_jwt_grant::{self, SYSTEM_GRANTS};

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[pg_grant_unique] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 从 `postgres://u:p@h:p/dbname` 里替换库名。
fn with_db(url: &str, db_name: &str) -> String {
    let (base, _old) = url
        .rsplit_once('/')
        .expect("TEST_PG_URL must contain a /db");
    format!("{base}/{db_name}")
}

/// 建一个唯一数据库并返回连到它的 `DbPool`（**未**跑迁移）。
async fn fresh_pg_pool() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_token_uq_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin = sqlx::PgPool::connect(&url).await.expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    Some(
        digua_common_db::init_pool(&with_db(&url, &db_name))
            .await
            .expect("init_pool on fresh pg db"),
    )
}

/// 建库 + 跑完 migrations（生产 marker 路径）。
async fn fresh_pool() -> Option<DbPool> {
    let pool = fresh_pg_pool().await?;
    db::ensure_schema(&pool).await.expect("ensure_schema on pg");
    Some(pool)
}

/// 与生产执行器（db::apply_sql_script）同口径：剥注释行 + 分号切分，
/// 逐条自动提交——不打包迁移级事务，忠实模拟真实执行边界。
async fn apply_pg_script(pool: &sqlx::PgPool, script: &str) {
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
        sqlx::query(s)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("apply pg stmt `{}`: {e}", s.lines().next().unwrap_or("")));
    }
}

/// 驱动 V005 的 rust_step（生产同一函数）。
async fn v005_rust_step(pool: &DbPool) -> Result<(), anyhow::Error> {
    token_service::db::run_rust_step(pool, RustMigrationStep::V005GrantPk).await
}

/// 驱动 V005 的 SQL 脚本（残留清理 + 哨兵 marker）。
async fn v005_sql(pg: &sqlx::PgPool) {
    let v005 = db::MIGRATIONS
        .iter()
        .find(|m| m.version == "V005")
        .expect("V005 registered");
    apply_pg_script(pg, v005.sql_for(DialectKind::Postgres)).await;
}

async fn grant_count(pool: &DbPool, owner: &str, audience: &str) -> i64 {
    service_jwt_grant::find_by_owner_and_audience(pool, owner, audience)
        .await
        .expect("find grant")
        .map(|_| 1)
        .unwrap_or(0)
}

async fn grant_total(pool: &DbPool) -> i64 {
    service_jwt_grant::list_all(pool)
        .await
        .expect("list grants")
        .len() as i64
}

async fn pk_defs(pg: &sqlx::PgPool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'token_service_jwt_grants'::regclass AND contype = 'p'",
    )
    .fetch_all(pg)
    .await
    .expect("probe pk");
    rows.into_iter().map(|r| r.0).collect()
}

/// 首版 V004（commit 1983c2bb）的 PostgreSQL 建表语句：**无** PRIMARY KEY。
/// 原样内联以模拟真实存量库；修正版 V004 与 V005 都不能改动这里的语义。
const LEGACY_V004_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_token_service_jwt_grants_audience
  ON token_service_jwt_grants (audience);
CREATE TABLE IF NOT EXISTS token_migration_v004_applied (
  applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
"#;

async fn insert_legacy_grant(pg: &sqlx::PgPool, owner: &str, audience: &str) {
    sqlx::query(
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES ($1, $2, 'active', TRUE)",
    )
    .bind(owner)
    .bind(audience)
    .execute(pg)
    .await
    .expect("insert legacy grant");
}

// ── #1 相同 owner+audience 只能插入一条 ────────────────────────────────────

#[tokio::test]
async fn pg_duplicate_owner_audience_insert_rejected() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    db_execute!(
        &pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        "svc-x",
        "aud-1"
    )
    .expect("first insert must succeed");

    let err = db_execute!(
        &pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        "svc-x",
        "aud-1"
    )
    .expect_err("duplicate (owner, audience) must violate the composite primary key");
    assert!(
        is_duplicate_key(&err),
        "must surface as a unique/pk violation, got: {err}"
    );

    assert_eq!(grant_count(&pool, "svc-x", "aud-1").await, 1);
    assert_eq!(grant_total(&pool).await, 1);
}

// ── #2 不同 audience 可以分别授权 ──────────────────────────────────────────

#[tokio::test]
async fn pg_different_audiences_independently_granted() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    for audience in ["aud-x", "aud-y"] {
        db_execute!(
            &pool,
            "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
             VALUES (?, ?, 'active', FALSE)",
            "svc-a",
            audience
        )
        .unwrap_or_else(|e| panic!("insert (svc-a, {audience}) must succeed: {e}"));
    }

    assert_eq!(grant_total(&pool).await, 2);
    assert!(service_jwt_grant::is_allowed(&pool, "svc-a", "aud-x")
        .await
        .expect("is_allowed x"));
    assert!(service_jwt_grant::is_allowed(&pool, "svc-a", "aud-y")
        .await
        .expect("is_allowed y"));
    assert!(!service_jwt_grant::is_allowed(&pool, "svc-a", "aud-z")
        .await
        .expect("is_allowed z"));
}

// ── #3 并发 ensure_system_grants 不产生重复行 ──────────────────────────────

#[tokio::test]
async fn pg_concurrent_ensure_system_grants_no_duplicates() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    // 8 个并发副本模拟多实例同时启动 seed。
    let mut handles = Vec::new();
    for _ in 0..8 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            service_jwt_grant::ensure_system_grants(&p).await
        }));
    }
    for h in handles {
        let inserted = h.await.expect("join").expect("ensure must not fail");
        assert!(inserted <= SYSTEM_GRANTS.len() as u64);
    }

    for (owner, audience) in SYSTEM_GRANTS {
        assert_eq!(
            grant_count(&pool, owner, audience).await,
            1,
            "concurrent seed must leave exactly one row for ({owner}, {audience})"
        );
    }
    assert_eq!(
        grant_total(&pool).await as usize,
        SYSTEM_GRANTS.len(),
        "no duplicate rows may exist after concurrent seeding"
    );
}

// ── #4 重复 seed 后结果幂等 ────────────────────────────────────────────────

#[tokio::test]
async fn pg_repeated_seed_is_idempotent() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    let first = service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("first seed");
    assert_eq!(first as usize, SYSTEM_GRANTS.len(), "fresh table seeds all");

    let second = service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("second seed");
    assert_eq!(second, 0, "re-seeding must insert nothing");

    assert_eq!(
        grant_total(&pool).await as usize,
        SYSTEM_GRANTS.len(),
        "row count must stay unchanged across repeated seeding"
    );
}

// ── #5 已禁用的系统 grant 不被启动 seed 复活 ───────────────────────────────

#[tokio::test]
async fn pg_disabled_system_grant_not_revived() {
    let Some(pool) = fresh_pool().await else {
        return;
    };

    let (owner, audience) = SYSTEM_GRANTS[0];
    db_execute!(
        &pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'disabled', TRUE)",
        owner,
        audience
    )
    .expect("insert disabled system grant");

    let inserted = service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("seed over disabled grant");
    assert_eq!(
        inserted as usize,
        SYSTEM_GRANTS.len() - 1,
        "only the missing grants may be seeded, the disabled row must stay untouched"
    );

    let row = service_jwt_grant::find_by_owner_and_audience(&pool, owner, audience)
        .await
        .expect("find")
        .expect("disabled row must still exist");
    assert_eq!(row.status, "disabled", "disabled grant must not be revived");
    assert!(row.system_managed);

    assert!(
        !service_jwt_grant::is_allowed(&pool, owner, audience)
            .await
            .expect("is_allowed on disabled grant"),
        "disabled grant must keep failing closed"
    );
}

// ── #6 已有 V004（首版无主键）数据库可以通过 V005 升级 ─────────────────────

#[tokio::test]
async fn pg_legacy_v004_database_upgrades_via_v005() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    // 1. 复原存量库形态：首版 V004 表（无主键）+ 两条合法数据
    //    + R1 版中断残留的 guard 表（含临时数据）
    apply_pg_script(pg, LEGACY_V004_SQL).await;
    for (owner, audience) in SYSTEM_GRANTS {
        insert_legacy_grant(pg, owner, audience).await;
    }
    apply_pg_script(
        pg,
        r#"
CREATE TABLE IF NOT EXISTS token_v005_grant_dup_guard (
  owner    VARCHAR(64) NOT NULL,
  audience VARCHAR(64) NOT NULL,
  UNIQUE (owner, audience)
);
INSERT INTO token_v005_grant_dup_guard (owner, audience)
SELECT owner, audience FROM token_service_jwt_grants;
"#,
    )
    .await;

    // 2. 执行 V005：rust_step（状态感知）+ SQL（残留清理 + marker）
    v005_rust_step(&pool)
        .await
        .expect("rust step must succeed on a clean legacy table");
    v005_sql(pg).await;

    // 3. 主键存在且恰为 (owner, audience)
    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner, audience)".to_string()],
        "V005 must establish the composite primary key on the legacy table"
    );

    // 4. 数据完整保留
    assert_eq!(grant_total(&pool).await as usize, SYSTEM_GRANTS.len());

    // 5. V005 哨兵表就位（marker 探测语义）
    let sentinels: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_migration_v005_applied'",
    )
    .fetch_all(pg)
    .await
    .expect("probe sentinel");
    assert_eq!(sentinels.len(), 1);

    // 6. R1 版 guard 残留表被 V005 SQL 清理
    let guards: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_v005_grant_dup_guard'",
    )
    .fetch_all(pg)
    .await
    .expect("probe guard");
    assert!(guards.is_empty(), "legacy guard table must be dropped");

    // 7. 重跑 V005 幂等（迁移失败重试路径）
    v005_rust_step(&pool).await.expect("re-run rust step");
    v005_sql(pg).await;
    assert_eq!(
        grant_total(&pool).await as usize,
        SYSTEM_GRANTS.len(),
        "re-applying V005 must be a no-op on data"
    );
}

// ── #8 主键缺失时重试 → 建立正确主键 ──────────────────────────────────────

/// R1 版 V005 中断残留状态之一：主键已被 DROP 但尚未重新 ADD。
/// 重试必须识别「主键不存在」并安全建立，不得因上次中断而永久失败。
#[tokio::test]
async fn pg_v005_pk_missing_rebuilds_on_retry() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    apply_pg_script(pg, LEGACY_V004_SQL).await;
    for (owner, audience) in SYSTEM_GRANTS {
        insert_legacy_grant(pg, owner, audience).await;
    }

    assert!(
        pk_defs(pg).await.is_empty(),
        "legacy table must start without a pk"
    );

    v005_rust_step(&pool)
        .await
        .expect("rust step must add the pk");
    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner, audience)".to_string()],
        "retry must establish the composite primary key"
    );
    assert_eq!(grant_total(&pool).await as usize, SYSTEM_GRANTS.len());
}

// ── #9 主键定义错误 → 明确失败并报告当前定义 ──────────────────────────────

#[tokio::test]
async fn pg_v005_unexpected_pk_fails_without_dropping() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    apply_pg_script(
        pg,
        r#"
CREATE TABLE IF NOT EXISTS token_service_jwt_grants (
  owner          VARCHAR(64)  NOT NULL,
  audience       VARCHAR(64)  NOT NULL,
  status         VARCHAR(16)  NOT NULL DEFAULT 'active',
  system_managed BOOLEAN      NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at     TIMESTAMPTZ  NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (owner)
);
"#,
    )
    .await;

    let err = v005_rust_step(&pool)
        .await
        .expect_err("wrong pk definition must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("PRIMARY KEY (owner)") && msg.contains("refusing"),
        "error must report the current definition, got: {msg}"
    );
    // 未知约束未被删除
    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner)".to_string()],
        "unknown constraint must be left untouched"
    );
}

// ── #10 主键已正确存在时重试 → no-op，数据不变 ────────────────────────────

#[tokio::test]
async fn pg_v005_rerun_is_noop_when_pk_correct() {
    let Some(pool) = fresh_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    let before: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT owner, audience, status FROM token_service_jwt_grants ORDER BY owner, audience",
    )
    .fetch_all(pg)
    .await
    .expect("list before");
    v005_rust_step(&pool)
        .await
        .expect("rust step on a correct pk must be a no-op");

    let after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT owner, audience, status FROM token_service_jwt_grants ORDER BY owner, audience",
    )
    .fetch_all(pg)
    .await
    .expect("list after");
    assert_eq!(before, after, "re-run must not modify any grant row");
    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner, audience)".to_string()],
    );
}

// ── #11 DDL 后 / marker 建立前中断 → 重试识别状态并安全完成 ───────────────

#[tokio::test]
async fn pg_v005_retry_when_marker_missing() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    apply_pg_script(pg, LEGACY_V004_SQL).await;
    for (owner, audience) in SYSTEM_GRANTS {
        insert_legacy_grant(pg, owner, audience).await;
    }

    // 模拟中断点：rust_step（DDL）已完成，但 SQL 脚本（哨兵 marker）未执行。
    v005_rust_step(&pool).await.expect("rust step");

    let sentinels: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_migration_v005_applied'",
    )
    .fetch_all(pg)
    .await
    .expect("probe sentinel");
    assert!(
        sentinels.is_empty(),
        "marker must not exist before the SQL script runs"
    );

    // 重启重试：rust_step 重新评估（no-op）+ SQL 建立 marker → 安全完成。
    v005_rust_step(&pool).await.expect("retry rust step");
    v005_sql(pg).await;

    let sentinels: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_migration_v005_applied'",
    )
    .fetch_all(pg)
    .await
    .expect("probe sentinel");
    assert_eq!(sentinels.len(), 1, "marker must be established on retry");
    assert_eq!(grant_total(&pool).await as usize, SYSTEM_GRANTS.len());
}

// ── #12 重复数据 fail-closed + 人工处理后重试成功 ─────────────────────────

#[tokio::test]
async fn pg_v005_fails_closed_on_duplicate_rows() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    apply_pg_script(pg, LEGACY_V004_SQL).await;
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
             VALUES ('svc-dup', 'aud-dup', 'active', TRUE)",
        )
        .execute(pg)
        .await
        .expect("insert legacy duplicate rows");
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
    let kept: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = 'svc-dup'")
            .fetch_one(pg)
            .await
            .expect("count rows");
    assert_eq!(kept.0, 2, "duplicate rows must not be silently removed");

    // 失败迁移不得留下成功 marker。
    let sentinels: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_migration_v005_applied'",
    )
    .fetch_all(pg)
    .await
    .expect("probe sentinel");
    assert!(
        sentinels.is_empty(),
        "a failed migration must not record a success marker"
    );

    // 人工裁决：保留一行（此处模拟人工 DELETE 决策）→ 重试成功。
    sqlx::query(
        "DELETE FROM token_service_jwt_grants WHERE owner = 'svc-dup' AND ctid IN \
                 (SELECT ctid FROM token_service_jwt_grants WHERE owner = 'svc-dup' LIMIT 1)",
    )
    .execute(pg)
    .await
    .expect("manual resolution");

    v005_rust_step(&pool).await.expect("retry after manual fix");
    v005_sql(pg).await;
    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner, audience)".to_string()],
        "retry after manual resolution must establish the pk"
    );
}

// ── #13 PG 半边：主键恰为 (owner, audience) ────────────────────────────────

#[tokio::test]
async fn pg_grant_pk_is_owner_audience() {
    let Some(pool) = fresh_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    assert_eq!(
        pk_defs(pg).await,
        vec!["PRIMARY KEY (owner, audience)".to_string()],
        "grant table pk must be exactly (owner, audience) — mirror of the MySQL schema"
    );
}
