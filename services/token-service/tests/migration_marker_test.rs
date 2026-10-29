//! V002 迁移哨兵表：存量库（V001 已建表）上 `ensure_schema` 仍必须补跑 V002。
//!
//! 回归场景：V002 是 ALTER 型迁移。若沿用 `token_oauth_clients` 当 marker，
//! 存量部署上 V002 会被整段跳过，`user_id` / `expires_at` 列不存在，
//! 所有 oauth client 查询直接报 `Unknown column 'user_id'`。
//! 因此 V002 必须有自己的哨兵表 `token_migration_v002_applied`：
//! 哨兵与 ALTER 在同一份 SQL 里落地（all-or-nothing），哨兵缺失 = V002 未应用。

mod common;

use token_service::db::DbPool;

/// V002 的专用哨兵表（与 `src/db.rs::MIGRATIONS` 一致）。
const V002_SENTINEL_TABLE: &str = "token_migration_v002_applied";

async fn table_exists(pool: &sqlx::MySqlPool, table: &str) -> bool {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.tables \
         WHERE table_schema = DATABASE() AND table_name = ?",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("probe tables");
    row.0 > 0
}

async fn column_exists(pool: &sqlx::MySqlPool, table: &str, column: &str) -> bool {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("probe columns");
    row.0 > 0
}

#[tokio::test]
async fn v002_is_applied_to_an_existing_v001_database() {
    let pool = common::spawn_mysql().await;
    let DbPool::Mysql(p) = &pool;

    // 模拟存量部署：V001 的表在，V002 的列与哨兵都不在。
    sqlx::query("ALTER TABLE token_oauth_clients DROP COLUMN user_id, DROP COLUMN expires_at")
        .execute(p)
        .await
        .expect("drop V002 columns");
    sqlx::query(&format!("DROP TABLE IF EXISTS {V002_SENTINEL_TABLE}"))
        .execute(p)
        .await
        .expect("drop V002 sentinel");

    assert!(
        table_exists(p, "token_oauth_clients").await,
        "V001 的 marker 表应当存在"
    );
    assert!(!column_exists(p, "token_oauth_clients", "user_id").await);
    assert!(!table_exists(p, V002_SENTINEL_TABLE).await);

    // V001 的 marker 在，V002 仍必须执行。
    token_service::db::ensure_schema(&pool).await.expect("ensure schema");

    assert!(
        column_exists(p, "token_oauth_clients", "user_id").await,
        "V002 未被执行：user_id 列缺失"
    );
    assert!(column_exists(p, "token_oauth_clients", "expires_at").await);
    assert!(
        table_exists(p, V002_SENTINEL_TABLE).await,
        "V002 哨兵表缺失"
    );

    // 幂等：哨兵在位时再跑一次不得报错。
    token_service::db::ensure_schema(&pool).await.expect("ensure schema again");
}
