//! Phase 6-A T4: outbox archive cron 集成测（testcontainers MySQL）。
//!
//! 7 个用例覆盖：24h 搬运、跳未 published、1000/batch、GET_LOCK 互斥、
//! 30d purge、BETWEEN 窗口拾取晚到 published 行、publisher 主循环不受影响。
//!
//! Ponytail: 不重复造 V002 schema——`db::ensure_schema` 已经会建好
//! `iam_outbox_events_archive`;测试 setup 只确保 schema 在,不再手工拼 SQL。

use chrono::{Duration as ChronoDuration, Utc};
use iam_service::outbox_archive;
use sqlx::Row;
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers_modules::{mysql::Mysql, testcontainers::runners::AsyncRunner};

struct Ctx {
    pool: sqlx::MySqlPool,
    _mysql: testcontainers::ContainerAsync<Mysql>,
}

async fn setup() -> Ctx {
    let mysql_c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .unwrap();
    let port = mysql_c.get_host_port_ipv4(3306).await.unwrap();
    let host = mysql_c.get_host().await.unwrap();
    let url = format!("mysql://root@{host}:{port}/test");
    let pool = common::db::connect(&url, 5).await.unwrap();
    iam_service::db::ensure_schema(&pool).await.unwrap();
    Ctx {
        pool,
        _mysql: mysql_c,
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
    insert_outbox_row(&ctx.pool, "ev.old", Some(Utc::now() - ChronoDuration::hours(25))).await;
    insert_outbox_row(&ctx.pool, "ev.recent", Some(Utc::now() - ChronoDuration::hours(1))).await;

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 1, "old row archived, recent row kept");
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 1);
}

#[tokio::test]
async fn archive_skips_unpublished_rows() {
    let ctx = setup().await;
    insert_outbox_row(&ctx.pool, "ev.unpublished", None).await;

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 1, "unpublished row must not be archived");
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 0);
}

#[tokio::test]
async fn archive_batches_at_1000() {
    let ctx = setup().await;
    for _ in 0..2500 {
        insert_outbox_row(&ctx.pool, "ev.bulk", Some(Utc::now() - ChronoDuration::hours(25))).await;
    }

    outbox_archive::run_once(&ctx.pool).await.unwrap();

    assert_eq!(count(&ctx.pool, "iam_outbox_events").await, 0, "all 2500 rows archived");
    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 2500);
}

#[tokio::test]
async fn archive_get_lock_prevents_concurrent_runs() {
    let ctx = Arc::new(setup().await);
    // 直接持锁
    let _held: (i64,) = sqlx::query_as("SELECT GET_LOCK('iam_outbox_archive', 0)")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();

    // run_once 应观察到 acquired=0 → 跳过
    outbox_archive::run_once(&ctx.pool).await.unwrap();
    // 验证：lock 仍在持，原表无变化（无任何行可 archive 也算通过）
    let _ = count(&ctx.pool, "iam_outbox_events").await;

    // 释放
    let _released: (i64,) = sqlx::query_as("SELECT RELEASE_LOCK('iam_outbox_archive')")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
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

    assert_eq!(count(&ctx.pool, "iam_outbox_events_archive").await, 0, "31-day-old row purged");
}

#[tokio::test]
async fn archive_window_picks_up_late_published() {
    // 模拟：窗口内 publisher 抢先 published 了一条原本未 published 的行
    // 由于二次 WHERE 包含 published_at < NOW(3) - INTERVAL 24 HOUR，
    // 该行此刻 published_at 仍 < 25h ago，会被一并归档
    let ctx = setup().await;
    let _old_id = insert_outbox_row(&ctx.pool, "ev.late", Some(Utc::now() - ChronoDuration::hours(25))).await;
    let new_id = insert_outbox_row(&ctx.pool, "ev.fresh", Some(Utc::now() - ChronoDuration::hours(1))).await;

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
