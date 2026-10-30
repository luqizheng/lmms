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
