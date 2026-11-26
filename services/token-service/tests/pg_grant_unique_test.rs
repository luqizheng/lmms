//! WP-LMMS-SERVICE-JWT-AUDIENCE-02-R1：grant 表唯一性 / 并发 / 升级验证（PostgreSQL）。
//!
//! 覆盖实施任务书测试矩阵 1-6 与 #7 的 PG 半边：
//! 1. 相同 owner+audience 只能插入一条
//! 2. 不同 audience 可以分别授权
//! 3. 并发 ensure_system_grants 不产生重复行
//! 4. 重复 seed 后结果幂等
//! 5. 已禁用的系统 grant 不被启动 seed 复活
//! 6. 已有 V004（首版无主键）数据库可以通过 V005 升级
//! 7. PG 半边：主键必须恰为 (owner, audience)
//!    （MySQL 半边在 service_jwt_grant_test.rs::mysql_grant_pk_is_owner_audience，
//!    双方言脚本静态一致性由 db.rs::grant_pk_is_owner_audience_in_both_dialects 把关）
//!
//! 走 **`TEST_PG_URL` 外部容器模式**（本环境 testcontainers 不可用）。
//! 无该变量时打印 `[skip]` 并直接返回，不失败。

#![cfg(feature = "postgres")]

use digua_common_db::db_execute;
use token_service::db::{self, is_duplicate_key, DbPool};
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

/// 与生产执行器（db::apply_sql_script）同口径：剥注释行 + 分号切分，逐条执行。
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

#[tokio::test]
async fn pg_legacy_v004_database_upgrades_via_v005() {
    let Some(pool) = fresh_pg_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    // 1. 复原存量库形态：首版 V004 表（无主键）+ 两条合法数据
    apply_pg_script(pg, LEGACY_V004_SQL).await;
    for (owner, audience) in SYSTEM_GRANTS {
        sqlx::query(
            "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
             VALUES ($1, $2, 'active', TRUE)",
        )
        .bind(owner)
        .bind(audience)
        .execute(pg)
        .await
        .expect("seed legacy row");
    }

    // 2. 执行 V005（生产脚本，从 MIGRATIONS 取，避免测试漂移）
    let v005 = db::MIGRATIONS
        .iter()
        .find(|m| m.version == "V005")
        .expect("V005 registered");
    apply_pg_script(
        pg,
        v005.sql_for(digua_common_db::dialect::DialectKind::Postgres),
    )
    .await;

    // 3. 主键存在且恰为 (owner, audience)
    let defs: Vec<(String,)> = sqlx::query_as(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'token_service_jwt_grants'::regclass AND contype = 'p'",
    )
    .fetch_all(pg)
    .await
    .expect("probe pk");
    assert_eq!(
        defs,
        vec![("PRIMARY KEY (owner, audience)".to_string(),)],
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

    // 6. dup-guard 临时表已清理
    let guards: Vec<(String,)> = sqlx::query_as(
        "SELECT tablename FROM pg_tables \
         WHERE schemaname = current_schema() AND tablename = 'token_v005_grant_dup_guard'",
    )
    .fetch_all(pg)
    .await
    .expect("probe guard");
    assert!(guards.is_empty(), "dup-guard table must be dropped");

    // 7. 重跑 V005 幂等（迁移失败重试路径）
    apply_pg_script(
        pg,
        v005.sql_for(digua_common_db::dialect::DialectKind::Postgres),
    )
    .await;
    assert_eq!(
        grant_total(&pool).await as usize,
        SYSTEM_GRANTS.len(),
        "re-applying V005 must be a no-op on data"
    );
}

/// 存量库带重复行时，V005 必须 fail-closed：迁移失败停机、错误含唯一约束语义、
/// 绝不静默删行。
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

    let v005 = db::MIGRATIONS
        .iter()
        .find(|m| m.version == "V005")
        .expect("V005 registered");
    let script = v005.sql_for(digua_common_db::dialect::DialectKind::Postgres);

    // 与生产 apply_sql_script 同口径执行：任何一条语句报错都必须向上传播。
    let mut failed = None;
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
        if let Err(e) = sqlx::query(s).execute(pg).await {
            failed = Some(e);
            break;
        }
    }
    let err = failed.expect("V005 must fail on duplicate (owner, audience) rows");
    assert!(
        is_duplicate_key(&err),
        "dup-guard violation must surface as unique violation, got: {err}"
    );

    // 重复行原样保留，交由人工裁决——绝不静默删行。
    let kept: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = 'svc-dup'")
            .fetch_one(pg)
            .await
            .expect("count rows");
    assert_eq!(kept.0, 2, "duplicate rows must not be silently removed");
}

// ── #7 PG 半边：主键恰为 (owner, audience) ─────────────────────────────────

#[tokio::test]
async fn pg_grant_pk_is_owner_audience() {
    let Some(pool) = fresh_pool().await else {
        return;
    };
    let pg = pool.as_postgres().expect("pg pool");

    let defs: Vec<(String,)> = sqlx::query_as(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid = 'token_service_jwt_grants'::regclass AND contype = 'p'",
    )
    .fetch_all(pg)
    .await
    .expect("probe pk");
    assert_eq!(
        defs,
        vec![("PRIMARY KEY (owner, audience)".to_string(),)],
        "grant table pk must be exactly (owner, audience) — mirror of the MySQL schema"
    );
}
