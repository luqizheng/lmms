//! `db_*!` / `db_qb_*!` 宏的回归测试。
//!
//! 覆盖三件必须成立的事：
//! 1. **宏能展开**（每个启用方言的分支都能编译，且 `$bind` 顺序正确）；
//! 2. **bind 个数契约在构造期生效**——SQL 里的 `?` 个数与 bind 实参个数不符时，
//!    **MySQL / SQLite 分支同样报错**（这是「MySQL 静默接受编号错位」的唯一封堵点）；
//! 3. **同一份 SQL 在 PG 下真的能跑**（`?` → `$N` 改写后的语句被真实 PG 接受）。
//!
//! # 调用形态
//!
//! 宏内部**已经**完成 `.await`，因此宏表达式本身求值为 `Result<.., sqlx::Error>`
//! （不是 Future）。宏内部用 `?` 传播渲染错误，因此**外层函数必须返回
//! `Result<_, E>` 且 `E: From<sqlx::Error>`**——`sqlx::Error` 与 `AppError` 都满足，
//! 调用点不需要改错误类型。

use digua_common_db::{
    db_execute, db_fetch_all, db_fetch_one, db_fetch_optional, db_fetch_scalar, db_qb_execute,
    db_qb_fetch_all, db_qb_fetch_one, db_qb_fetch_optional,
};

#[derive(Debug, sqlx::FromRow, PartialEq)]
struct Row {
    id: i64,
    name: String,
}

/// 建一张测试表并插入若干行（走 `db_execute!`，顺带验证写宏）。
async fn seed(pool: &digua_common_db::DbPool) -> Result<(), sqlx::Error> {
    db_execute!(
        pool,
        "CREATE TABLE IF NOT EXISTS macro_users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)"
    )?;
    db_execute!(pool, "DELETE FROM macro_users")?;
    db_execute!(
        pool,
        "INSERT INTO macro_users (id, name) VALUES (?, ?)",
        1i64,
        "alice"
    )?;
    db_execute!(
        pool,
        "INSERT INTO macro_users (id, name) VALUES (?, ?)",
        2i64,
        "bob"
    )?;
    Ok(())
}

#[cfg(feature = "sqlite")]
mod sqlite_backend {
    use super::*;

    async fn memory_pool() -> digua_common_db::DbPool {
        // `max_connections(1)`：内存库每个连接独立，锁到单连接才能保证表可见。
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("open in-memory sqlite");
        digua_common_db::DbPool::Sqlite(pool)
    }

    #[tokio::test]
    async fn fetch_all_returns_rows_in_insert_order() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await?;

        let rows: Vec<Row> =
            db_fetch_all!(&pool, Row, "SELECT id, name FROM macro_users ORDER BY id")?;

        assert_eq!(
            rows,
            vec![
                Row {
                    id: 1,
                    name: "alice".into()
                },
                Row {
                    id: 2,
                    name: "bob".into()
                },
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn fetch_one_and_optional_differ_on_missing_row() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await?;

        let one: Row =
            db_fetch_one!(&pool, Row, "SELECT id, name FROM macro_users WHERE id = ?", 1i64)?;
        assert_eq!(one.name, "alice");

        let some: Option<Row> =
            db_fetch_optional!(&pool, Row, "SELECT id, name FROM macro_users WHERE id = ?", 1i64)?;
        assert_eq!(some.map(|r| r.id), Some(1));

        let none: Option<Row> =
            db_fetch_optional!(&pool, Row, "SELECT id, name FROM macro_users WHERE id = ?", 999i64)?;
        assert!(none.is_none(), "缺失行必须是 Ok(None)，而不是 Err");
        Ok(())
    }

    #[tokio::test]
    async fn fetch_scalar_returns_count() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await?;

        let count: i64 = db_fetch_scalar!(
            &pool,
            i64,
            "SELECT COUNT(*) FROM macro_users WHERE name = ?",
            "alice"
        )?;
        assert_eq!(count, 1);
        Ok(())
    }

    #[tokio::test]
    async fn execute_reports_rows_affected() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await?;

        let affected = db_execute!(
            &pool,
            "UPDATE macro_users SET name = ? WHERE id = ?",
            "carol",
            1i64
        )?;
        assert_eq!(affected, 1);
        Ok(())
    }

    /// **顺序契约的核心防线**：bind 个数与 `?` 个数不符时，**在构造期**报错——
    /// 即使后端是 SQLite / MySQL（它们本身不会因为编号错位而报错）。
    #[tokio::test]
    async fn bind_count_mismatch_is_rejected_at_construction_time() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await.expect("seed");

        // SQL 只有 1 个 `?`，却传了 2 个 bind —— 必须报错，而不是静默执行。
        let err = db_fetch_all!(
            &pool,
            Row,
            "SELECT id, name FROM macro_users WHERE id = ?",
            1i64,
            2i64
        )
        .expect_err("bind 个数不符必须报错");
        let message = err.to_string();
        assert!(
            message.contains("bind count mismatch"),
            "错误信息应说明 bind 个数不符，实际: {message}"
        );
        Ok(())
    }

    /// 反向：bind 个数**少于** `?` 个数，同样必须报错。
    #[tokio::test]
    async fn too_few_binds_is_rejected_at_construction_time() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await.expect("seed");

        let err =
            db_execute!(&pool, "INSERT INTO macro_users (id, name) VALUES (?, ?)", 3i64)
                .expect_err("bind 个数不足必须报错");
        assert!(
            err.to_string().contains("bind count mismatch"),
            "错误信息应说明 bind 个数不符，实际: {err}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn qb_macros_work() -> Result<(), sqlx::Error> {
        let pool = memory_pool().await;
        seed(&pool).await?;

        let rows: Vec<Row> = db_qb_fetch_all!(&pool, Row, |qb| {
            qb.push("SELECT id, name FROM macro_users WHERE id >= ")
                .push_bind(1i64);
        })?;
        assert_eq!(rows.len(), 2);

        let one: Row = db_qb_fetch_one!(&pool, Row, |qb| {
            qb.push("SELECT id, name FROM macro_users WHERE id = ")
                .push_bind(2i64);
        })?;
        assert_eq!(one.name, "bob");

        let none: Option<Row> = db_qb_fetch_optional!(&pool, Row, |qb| {
            qb.push("SELECT id, name FROM macro_users WHERE id = ")
                .push_bind(999i64);
        })?;
        assert!(none.is_none());

        let affected = db_qb_execute!(&pool, |qb| {
            qb.push("DELETE FROM macro_users WHERE id = ").push_bind(2i64);
        })?;
        assert_eq!(affected, 1);
        Ok(())
    }
}

/// PostgreSQL：同一份 `?` SQL 改写成 `$N` 后必须被真实 PG 接受并返回正确数据。
#[cfg(feature = "postgres")]
mod postgres_backend {
    use super::*;

    fn test_pg_url() -> Option<String> {
        match std::env::var("TEST_PG_URL") {
            Ok(url) if !url.trim().is_empty() => Some(url),
            _ => {
                eprintln!("[query_macro_test] TEST_PG_URL not set — skipped");
                None
            }
        }
    }

    #[tokio::test]
    async fn pg_macros_execute_rewritten_sql() -> Result<(), sqlx::Error> {
        let Some(url) = test_pg_url() else { return Ok(()) };
        let pool = digua_common_db::init_pool(&url).await.expect("init_pool");
        assert_eq!(
            pool.dialect(),
            digua_common_db::dialect::DialectKind::Postgres
        );

        db_execute!(
            &pool,
            "CREATE TABLE IF NOT EXISTS macro_users (id BIGINT PRIMARY KEY, name TEXT NOT NULL)"
        )?;
        db_execute!(&pool, "DELETE FROM macro_users")?;
        db_execute!(
            &pool,
            "INSERT INTO macro_users (id, name) VALUES (?, ?)",
            1i64,
            "alice"
        )?;

        // 多个占位符：验证 $1 / $2 的编号与 bind 顺序严格对应。
        let rows: Vec<Row> = db_fetch_all!(
            &pool,
            Row,
            "SELECT id, name FROM macro_users WHERE id >= ? AND name <> ?",
            1i64,
            "nobody"
        )?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "alice");

        let count: i64 = db_fetch_scalar!(
            &pool,
            i64,
            "SELECT COUNT(*) FROM macro_users WHERE name = ?",
            "alice"
        )?;
        assert_eq!(count, 1);

        // bind 个数不符在 PG 分支同样报错。
        let err = db_fetch_all!(
            &pool,
            Row,
            "SELECT id, name FROM macro_users WHERE id = ?",
            1i64,
            2i64
        )
        .expect_err("PG 分支也必须校验 bind 个数");
        assert!(err.to_string().contains("bind count mismatch"));
        Ok(())
    }
}
