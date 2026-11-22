//! 用真实 PG 校验 `migrations/postgres/*.sql` 能否**逐条**执行成功。
//!
//! 迁移执行器是 `script.split(';')`（各服务 `ensure_schema` 的实际做法），因此这里
//! 用**同样的切分方式**跑一遍——能直接暴露「PG 语法不合法」「`$$` 被切坏」
//! 「索引/唯一约束写法不对」等问题，而这些问题只在运行期才暴露。
//!
//! 无 `TEST_PG_URL` 时跳过（打印 `[skip]`），与既有 pg_test 约定一致。
//!
//! 每个服务的迁移在**独立 schema** 里跑，互不干扰，也不污染 `public`。

#![cfg(feature = "postgres")]

use sqlx::{Executor, PgPool};

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[pg_migration_test] TEST_PG_URL not set — skipped");
            None
        }
    }
}

/// 与各服务 `ensure_schema` 完全一致的切分 + 注释剥离逻辑。
fn split_sql(script: &str) -> Vec<String> {
    script
        .split(';')
        .map(|stmt| {
            stmt.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 在独立 schema 里依次执行给定迁移文件，返回执行的语句数。
async fn apply_migrations(pool: &PgPool, schema: &str, scripts: &[&str]) -> usize {
    // 关键：必须用**同一条连接**。连接池下每条 execute 可能落到不同连接，
    // SET search_path 不会跨连接生效（首版就踩了这个坑）。
    let mut conn = pool.acquire().await.expect("acquire conn");
    conn.execute(format!("DROP SCHEMA IF EXISTS {schema} CASCADE").as_str())
        .await
        .expect("drop schema");
    conn.execute(format!("CREATE SCHEMA {schema}").as_str())
        .await
        .expect("create schema");
    // 迁移里的表名不带 schema 前缀，靠 search_path 定位到本 schema。
    conn.execute(format!("SET search_path TO {schema}").as_str())
        .await
        .expect("set search_path");

    let mut executed = 0usize;
    for (idx, script) in scripts.iter().enumerate() {
        for stmt in split_sql(script) {
            conn.execute(stmt.as_str()).await.unwrap_or_else(|e| {
                panic!("migration #{idx} statement failed:\n{stmt}\n\nerror: {e}")
            });
            executed += 1;
        }
    }
    executed
}

#[tokio::test]
async fn file_service_postgres_migrations_apply() {
    let Some(url) = test_pg_url() else { return };
    let pool = PgPool::connect(&url).await.expect("connect pg");

    let executed = apply_migrations(
        &pool,
        "verify_file",
        &[
            include_str!("../../../services/file-service/migrations/postgres/V001__init.sql"),
            include_str!(
                "../../../services/file-service/migrations/postgres/V002__file_tenant_reconcile.sql"
            ),
        ],
    )
    .await;
    assert!(executed > 8, "应执行了多条语句，实际 {executed}");

    // V002 的关键效果：tenant_id 已从 VARCHAR 收紧为 BIGINT（PG: bigint）
    let ty: Vec<(String, String)> = sqlx::query_as(
        "SELECT data_type::text, column_default::text FROM information_schema.columns \
         WHERE table_schema = 'verify_file' AND table_name = 'file_files' AND column_name = 'tenant_id'",
    )
    .fetch_all(&pool)
    .await
    .expect("describe tenant_id");
    assert_eq!(ty.len(), 1, "tenant_id 必须存在");
    assert_eq!(ty[0].0, "bigint", "tenant_id 必须是 BIGINT（无 UNSIGNED）");

    let nullable: Vec<(String,)> = sqlx::query_as(
        "SELECT is_nullable::text FROM information_schema.columns \
         WHERE table_schema = 'verify_file' AND table_name = 'file_files' AND column_name = 'tenant_id'",
    )
    .fetch_all(&pool)
    .await
    .expect("nullable");
    assert_eq!(nullable[0].0, "NO", "tenant_id 必须是 NOT NULL");

    // is_temporary 必须是 BOOLEAN（MySQL TINYINT(1) → PG BOOLEAN，§3 #12）
    let bool_col: Vec<(String,)> = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = 'verify_file' AND table_name = 'file_files' AND column_name = 'is_temporary'",
    )
    .fetch_all(&pool)
    .await
    .expect("describe is_temporary");
    assert_eq!(bool_col[0].0, "boolean");
}

#[tokio::test]
async fn prue_backend_postgres_migration_applies() {
    let Some(url) = test_pg_url() else { return };
    let pool = PgPool::connect(&url).await.expect("connect pg");

    let executed = apply_migrations(
        &pool,
        "verify_prue",
        &[include_str!(
            "../../../services/prue_backend/migrations/postgres/V001__init.sql"
        )],
    )
    .await;
    assert!(executed > 3, "应执行了多条语句，实际 {executed}");

    // updated_at 不得残留 ON UPDATE 语义（PG 无此子句）——只校验列是 TIMESTAMPTZ
    let cols: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT column_name::text, data_type::text, column_default::text \
         FROM information_schema.columns \
         WHERE table_schema = 'verify_prue' AND table_name = 'prue_menu_configs' \
           AND column_name IN ('created_at','updated_at','published_at','deleted_at')",
    )
    .fetch_all(&pool)
    .await
    .expect("describe timestamp columns");
    assert_eq!(cols.len(), 4, "四个时间列都必须存在");
    for (col, ty, _) in &cols {
        assert_eq!(
            ty, "timestamp with time zone",
            "prue 列 {col} 必须是 TIMESTAMPTZ，实际 {ty}"
        );
    }

    // routes_json 必须是 json（§3 #5：PG 原生 json，不必 JSONB）
    let json_ty: Vec<(String,)> = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = 'verify_prue' AND table_name = 'prue_menu_configs' \
           AND column_name = 'routes_json'",
    )
    .fetch_all(&pool)
    .await
    .expect("describe routes_json");
    assert_eq!(json_ty[0].0, "json");

    // version 唯一索引存在（PG 的 upsert/幂等依赖它）
    let idx: Vec<(String,)> = sqlx::query_as(
        "SELECT indexname::text FROM pg_indexes \
         WHERE schemaname = 'verify_prue' AND tablename = 'prue_menu_configs'",
    )
    .fetch_all(&pool)
    .await
    .expect("list indexes");
    let names: Vec<&str> = idx.iter().map(|i| i.0.as_str()).collect();
    assert!(
        names.contains(&"uk_prue_menu_configs_version"),
        "version 唯一索引必须存在，实际: {names:?}"
    );
}

#[tokio::test]
async fn audit_service_postgres_migration_applies() {
    let Some(url) = test_pg_url() else { return };
    let pool = PgPool::connect(&url).await.expect("connect pg");

    let executed = apply_migrations(
        &pool,
        "verify_audit",
        &[include_str!(
            "../../../services/audit-service/migrations/postgres/V001__init.sql"
        )],
    )
    .await;
    assert!(executed > 10, "应执行了多条语句，实际 {executed}");

    // 四张表都在
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'verify_audit' ORDER BY table_name",
    )
    .fetch_all(&pool)
    .await
    .expect("list tables");
    let names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "audit_api_access_logs",
            "audit_events",
            "audit_login_logs",
            "audit_operation_logs"
        ]
    );

    // 时间列必须 TIMESTAMPTZ（§3 #3）
    let ts: Vec<(String, String)> = sqlx::query_as(
        "SELECT column_name::text, data_type::text FROM information_schema.columns \
         WHERE table_schema = 'verify_audit' AND data_type LIKE 'timestamp%'",
    )
    .fetch_all(&pool)
    .await
    .expect("list timestamp columns");
    assert!(!ts.is_empty(), "应存在时间列");
    for (col, ty) in &ts {
        assert_eq!(
            ty, "timestamp with time zone",
            "列 {col} 必须 TIMESTAMPTZ，实际 {ty}"
        );
    }

    // success 必须 BOOLEAN（MySQL TINYINT(1)）
    let bool_col: Vec<(String,)> = sqlx::query_as(
        "SELECT data_type::text FROM information_schema.columns \
         WHERE table_schema = 'verify_audit' AND table_name = 'audit_login_logs' \
           AND column_name = 'success'",
    )
    .fetch_all(&pool)
    .await
    .expect("describe success");
    assert_eq!(bool_col[0].0, "boolean");

    // audit_events.event_id 必须有唯一索引（ON CONFLICT (event_id) 的前提）
    let idx: Vec<(String,)> = sqlx::query_as(
        "SELECT indexname::text FROM pg_indexes \
         WHERE schemaname = 'verify_audit' AND tablename = 'audit_events'",
    )
    .fetch_all(&pool)
    .await
    .expect("list indexes");
    let inames: Vec<&str> = idx.iter().map(|i| i.0.as_str()).collect();
    assert!(
        inames.contains(&"uk_audit_events_event_id"),
        "event_id 唯一索引必须存在（PG 的 ON CONFLICT 依赖它），实际: {inames:?}"
    );

    // id 必须是 IDENTITY（AUTO_INCREMENT 的 PG 对应）
    let ident: Vec<(String,)> = sqlx::query_as(
        "SELECT is_identity::text FROM information_schema.columns \
         WHERE table_schema = 'verify_audit' AND table_name = 'audit_operation_logs' \
           AND column_name = 'id'",
    )
    .fetch_all(&pool)
    .await
    .expect("describe id");
    assert_eq!(ident[0].0, "YES", "id 必须是 GENERATED ... AS IDENTITY");
}

/// 迁移文件里不得出现执行器无法处理的块结构（`split(';')` 会切坏）。
#[test]
fn postgres_migrations_forbid_dollar_blocks() {
    let scripts = [
        include_str!("../../../services/file-service/migrations/postgres/V001__init.sql"),
        include_str!(
            "../../../services/file-service/migrations/postgres/V002__file_tenant_reconcile.sql"
        ),
        include_str!("../../../services/prue_backend/migrations/postgres/V001__init.sql"),
        include_str!("../../../services/audit-service/migrations/postgres/V001__init.sql"),
    ];
    // 只检查**可执行 SQL**：注释里解释 MySQL 原语法（如 `DATETIME -> TIMESTAMPTZ`）不算残留。
    let scripts: Vec<String> = scripts
        .iter()
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join(
                    "
",
                )
        })
        .collect();
    for script in scripts.iter() {
        // ⚠️ 执行器是「**先**按分号切分、**再**剥离注释行」。
        // 因此注释里出现 ASCII 分号时，切分点会落在注释中间，剩余片段不以 `--` 开头，
        // 会被当成可执行 SQL → 运行期报 `unterminated quoted string` 之类。
        // 这条同样适用于 MySQL 迁移（同一个 ensure_schema 逻辑），是既有隐患。
        for line in script.lines() {
            if line.trim_start().starts_with("--") {
                assert!(
                    !line.contains(';'),
                    "注释行不得含 ASCII 分号（执行器先切分后剥注释）: {line}"
                );
            }
        }
        assert!(
            !script.contains("$$"),
            "迁移里禁止 $$（split(';') 会切坏 dollar-quoted）"
        );
        let upper = script.to_uppercase();
        assert!(!upper.contains("DO $$"), "迁移里禁止 DO 块");
        assert!(
            !upper.contains("CREATE FUNCTION"),
            "迁移里禁止 CREATE FUNCTION"
        );
        assert!(
            !upper.contains("CREATE TRIGGER"),
            "迁移里禁止 CREATE TRIGGER"
        );
        // MySQL 专有语法残留检查
        assert!(
            !upper.contains("ENGINE=INNODB"),
            "PG 迁移不得残留 ENGINE=InnoDB"
        );
        assert!(!upper.contains("AUTO_INCREMENT"), "不得残留 AUTO_INCREMENT");
        assert!(
            !upper.contains("UNSIGNED"),
            "PG 无 UNSIGNED（§3 #23），应改用 BIGINT"
        );
        // DATETIME 是 MySQL 类型；PG 必须 TIMESTAMPTZ（§3 #3）
        assert!(
            !upper.contains("DATETIME"),
            "PG 迁移不得出现 DATETIME，必须 TIMESTAMPTZ"
        );
    }
}
