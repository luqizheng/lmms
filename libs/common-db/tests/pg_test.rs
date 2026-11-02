//! PostgreSQL 集成测试——需要真实 PG（共享容器 coder-test-pg，端口 5434）。
//!
//! 整个文件仅在 `postgres` feature 下编译；无 `TEST_PG_URL` 时用例打印 skip
//! 说明并直接通过：CI 无 service 的 job 与本地无容器的场景都依赖这一点（不得失败）。

#![cfg(feature = "postgres")]

use digua_common_db::dialect::DialectKind;
use digua_common_db::{init_pool, DbPool};

fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!("[pg_test] TEST_PG_URL not set — skipped");
            None
        }
    }
}

#[test]
fn pg_dialect_url_maps_to_postgres() {
    // 纯枚举断言，不依赖数据库
    assert_eq!(
        DialectKind::from_url("postgres://user:pwd@host:5432/db"),
        DialectKind::Postgres
    );
}

#[tokio::test]
async fn pg_pool_connects_and_reports_dialect() {
    let Some(url) = test_pg_url() else { return };
    let pool = init_pool(&url).await.expect("init_pool against coder-test-pg");
    assert_eq!(pool.dialect(), DialectKind::Postgres);
    match &pool {
        DbPool::Postgres(p) => {
            let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(p).await.expect("SELECT 1");
            assert_eq!(row.0, 1);
        }
        _ => panic!("expected a postgres pool"),
    }
}

#[tokio::test]
async fn pg_now_expr_is_valid_sql() {
    let Some(url) = test_pg_url() else { return };
    let d = DialectKind::Postgres;
    let pool = init_pool(&url).await.expect("init_pool");
    match &pool {
        DbPool::Postgres(p) => {
            let row: (chrono::NaiveDateTime,) =
                sqlx::query_as(&format!("SELECT {}", d.now_expr()))
                    .fetch_one(p)
                    .await
                    .expect("now_expr must be valid SQL");
            // 2020-01-01 之后即认为取到了合理的当前时间
            assert!(row.0
                > chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap());
        }
        _ => panic!("expected a postgres pool"),
    }
}

/// `probe_marker_postgres` / `probe_marker` 必须能在真实 PG 上判定 marker table 是否存在。
#[tokio::test]
async fn pg_marker_probe_detects_existing_table() {
    let Some(url) = test_pg_url() else { return };
    let pool = init_pool(&url).await.expect("init_pool");

    // 建一张专用 marker 表，再探测它。
    sqlx::query("CREATE TABLE IF NOT EXISTS pg_probe_marker (id BIGINT PRIMARY KEY)")
        .execute(pool.as_postgres().expect("pg pool"))
        .await
        .expect("create marker table");

    let found =
        digua_common_db::migration_state::probe_marker(&pool, "pg_probe_marker")
            .await
            .expect("probe_marker");
    assert!(found, "刚建的 marker table 必须被探测到");

    let missing =
        digua_common_db::migration_state::probe_marker(&pool, "pg_no_such_marker_table")
            .await
            .expect("probe_marker");
    assert!(!missing, "不存在的 marker table 必须返回 false");
}

/// `init_pool` 的 PG 分支必须把会话时区锁死为 UTC（Q2-bis）。
#[tokio::test]
async fn pg_session_timezone_is_utc() {
    let Some(url) = test_pg_url() else { return };
    let pool = init_pool(&url).await.expect("init_pool");
    let pg = pool.as_postgres().expect("pg pool");
    let tz: (String,) = sqlx::query_as("SHOW TIMEZONE")
        .fetch_one(pg)
        .await
        .expect("SHOW TIMEZONE");
    assert_eq!(tz.0, "UTC", "PG 会话时区必须被 after_connect 锁定为 UTC");
}

/// `placeholder_style` 必须与后端匹配（PG 是 `$N`，其余是 `?`）。
#[tokio::test]
async fn pg_placeholder_style_is_numbered() {
    let Some(url) = test_pg_url() else { return };
    let pool = init_pool(&url).await.expect("init_pool");
    assert_eq!(
        pool.placeholder_style(),
        digua_common_db::PlaceholderStyle::Numbered
    );
    assert_eq!(pool.dialect(), DialectKind::Postgres);
}

/// `ping()` 必须能在真实 PG 上成功返回。
#[tokio::test]
async fn pg_ping_succeeds() {
    let Some(url) = test_pg_url() else { return };
    let pool = init_pool(&url).await.expect("init_pool");
    pool.ping().await.expect("ping");
}
