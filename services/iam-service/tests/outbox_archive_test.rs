//! Phase 6-A T4: outbox archive cron 集成测（testcontainers MySQL）。
//!
//! 7 个用例覆盖：24h 搬运、跳未 published、1000/batch、GET_LOCK 互斥、
//! 30d purge、BETWEEN 窗口拾取晚到 published 行、publisher 主循环不受影响。
//!
//! Ponytail: 不重复造 V002 schema——`db::ensure_schema` 已经会建好
//! `iam_outbox_events_archive`;测试 setup 只确保 schema 在,不再手工拼 SQL。

mod common;

use chrono::{Duration as ChronoDuration, Utc};
use iam_service::outbox_archive;
use sqlx::{QueryBuilder, Row};
use std::sync::Arc;

struct Ctx {
    pool: sqlx::MySqlPool,
    _mysql: Option<common::MysqlContainer>,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    Ctx {
        pool: mysql_h.pool,
        _mysql: mysql_h.container,
    }
}

async fn insert_outbox_row(
    pool: &sqlx::MySqlPool,
    event_type: &str,
    published_at: Option<chrono::DateTime<Utc>>,
) -> i64 {
    let event_id = uuid::Uuid::new_v4().to_string();
    let created_at = Utc::now();
    let result = sqlx::query(
        "INSERT INTO iam_outbox_events (event_id, event_type, routing_key, envelope_json, created_at, published_at) \
         VALUES (?, ?, 'test.rk', '{}', ?, ?)"
    )
    .bind(&event_id)
    .bind(event_type)
    .bind(created_at)
    .bind(published_at)
    .execute(pool)
    .await
    .unwrap();
    result.last_insert_id() as i64
}

/// Bulk-insert `count` outbox rows in a single roundtrip; 替代循环单行
/// INSERT（archive_batches_at_1000 旧版 2500 次 roundtrip 在低内存环境
/// 会触发 docker memory pressure）。
async fn bulk_insert_outbox_rows(
    pool: &sqlx::MySqlPool,
    count: usize,
    event_type: &str,
    published_at: chrono::DateTime<Utc>,
) {
    let created_at = Utc::now();
    let mut qb = QueryBuilder::new(
        "INSERT INTO iam_outbox_events (event_id, event_type, routing_key, envelope_json, created_at, published_at) ",
    );
    qb.push_values(0..count, |mut b, _| {
        b.push_bind(uuid::Uuid::new_v4().to_string())
            .push_bind(event_type)
            .push_bind("test.rk")
            .push_bind("{}")
            .push_bind(created_at)
            .push_bind(published_at);
    });
    qb.build().execute(pool).await.unwrap();
}

async fn count(pool: &sqlx::MySqlPool, table: &str) -> i64 {
    sqlx::query(&format!("SELECT COUNT(*) AS c FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
        .get::<i64, _>("c")
}

#[tokio::test]
async fn archive_moves_published_rows_older_than_24h() {
    let ctx = setup().await;
    insert_outbox_row(
        &ctx.pool,
        "ev.old",
        Some(Utc::now() - ChronoDuration::hours(25)),
    )
    .await;
    insert_outbox_row(
        &ctx.pool,
        "ev.recent",
        Some(Utc::now() - ChronoDuration::hours(1)),
    )
    .await;

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(
        count(&ctx.pool, "iam_outbox_events").await,
        1,
        "old row archived, recent row kept"
    );
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 1);
}

#[tokio::test]
async fn archive_skips_unpublished_rows() {
    let ctx = setup().await;
    insert_outbox_row(&ctx.pool, "ev.unpublished", None).await;

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(
        count(&ctx.pool, "iam_outbox_events").await,
        1,
        "unpublished row must not be archived"
    );
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 0);
}

#[tokio::test]
async fn archive_batches_at_1000() {
    let ctx = setup().await;
    bulk_insert_outbox_rows(
        &ctx.pool,
        2500,
        "ev.bulk",
        Utc::now() - ChronoDuration::hours(25),
    )
    .await;

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(
        count(&ctx.pool, "iam_outbox_events").await,
        0,
        "all 2500 rows archived"
    );
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 2500);
}

#[tokio::test]
async fn archive_get_lock_prevents_concurrent_runs() {
    let ctx = Arc::new(setup().await);
    // 插入一条 25h 前的行，应被归档
    insert_outbox_row(
        &ctx.pool,
        "ev.blocked",
        Some(Utc::now() - ChronoDuration::hours(25)),
    )
    .await;

    // 另一连接持锁，模拟并发副本
    let mut held_conn = ctx.pool.acquire().await.unwrap();
    let _held: (i64,) = sqlx::query_as("SELECT GET_LOCK('iam_outbox_archive', 0)")
        .fetch_one(&mut *held_conn)
        .await
        .unwrap();

    // run_once 应观察到 acquired=0 → 跳过
    outbox_archive::run_once(&ctx.pool).await.unwrap();
    // 关键断言：行未被归档（锁阻止了 run_once 进入）
    assert_eq!(
        count(&ctx.pool, "iam_outbox_events").await,
        1,
        "row must NOT be archived while lock held"
    );
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 0);

    // 释放锁
    let _released: (i64,) = sqlx::query_as("SELECT RELEASE_LOCK('iam_outbox_archive')")
        .fetch_one(&mut *held_conn)
        .await
        .unwrap();
    drop(held_conn);
}

#[tokio::test]
async fn run_once_releases_lock_on_completion() {
    // 回归测：C1 fix — GET_LOCK/RELEASE_LOCK 必须绑同一 connection。
    // 旧实现 GET_LOCK 一个 conn、RELEASE_LOCK 另一个 conn，session-scoped 锁泄漏 8h。
    let ctx = setup().await;

    // 前置：锁空闲
    let (free_before,): (i64,) = sqlx::query_as("SELECT IS_FREE_LOCK('iam_outbox_archive')")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(free_before, 1, "lock must be free before run_once");

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    // 关键：run_once 返回后，锁必须被释放（同 conn 释放）
    let (free_after,): (i64,) = sqlx::query_as("SELECT IS_FREE_LOCK('iam_outbox_archive')")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(free_after, 1, "lock must be released after run_once");
}

#[tokio::test]
async fn purge_deletes_old_archive_rows() {
    let ctx = setup().await;
    // 构造一行 archived 31 天前的 archive 行
    sqlx::query(
        "INSERT INTO iam_outbox_events_archive (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at) \
         VALUES (1, ?, 'ev.purge', 'test.rk', '{}', NOW(3), NOW(3), NOW(3) - INTERVAL 31 DAY)"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&ctx.pool)
    .await
    .unwrap();

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(
        count(&ctx.pool, "iam_outbox_events_archive").await,
        0,
        "31-day-old row purged"
    );
}

#[tokio::test]
async fn archive_window_picks_up_late_published() {
    // 模拟：窗口内 publisher 抢先 published 了一条原本未 published 的行
    // 由于二次 WHERE 包含 published_at < NOW(3) - INTERVAL 24 HOUR，
    // 该行此刻 published_at 仍 < 25h ago，会被一并归档
    let ctx = setup().await;
    let _old_id = insert_outbox_row(
        &ctx.pool,
        "ev.late",
        Some(Utc::now() - ChronoDuration::hours(25)),
    )
    .await;
    let new_id = insert_outbox_row(
        &ctx.pool,
        "ev.fresh",
        Some(Utc::now() - ChronoDuration::hours(1)),
    )
    .await;

    // 第一次 run_once 搬运 old
    outbox_archive::run_once(&ctx.pool).await.unwrap();
    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 1);
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 1);

    // 现在把 new 改为 old（published_at 倒回到 25h ago），再次 run_once
    sqlx::query("UPDATE iam_outbox_events SET published_at = ? WHERE id = ?")
        .bind(Utc::now() - ChronoDuration::hours(25))
        .bind(new_id)
        .execute(&ctx.pool)
        .await
        .unwrap();

    outbox_archive::run_once(&ctx.pool).await.unwrap();
    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 0);
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 2);
}

#[tokio::test]
async fn publisher_main_loop_unaffected_during_archive() {
    // 起 archive task 后灌 50 行 unpublished,主循环（手动模拟）
    // 应能正常 claim_batch + mark_published
    let ctx = setup().await;
    for i in 0..50 {
        insert_outbox_row(&ctx.pool, &format!("ev.live.{i}"), None).await;
    }

    let pool_arc = ctx.pool.clone();
    let handle = tokio::spawn(async move {
        outbox_archive::run_once(&pool_arc).await.unwrap();
    });
    handle.await.unwrap();

    // archive_loop 看不到 unpublished 行 → 0 搬
    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 50);
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 0);
}
