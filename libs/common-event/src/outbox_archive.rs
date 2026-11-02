//! outbox 归档 cron 任务（iam-service 与 iam-outbox-publisher 共用）。
//!
//! 从两份逐字节重复的 `services/*/src/outbox_archive.rs` 收敛而来
//! （计划 2026-11-02-postgresql-service-layer-support Task S5 Step 3b）。
//!
//! ```text
//! Every 5 min:
//!   try <dialect lock>('iam_outbox_archive', 0):   acquired
//!     ├── archive rows where published_at < now-24h, 1000/batch, until empty
//!     ├── purge archive rows where archived_at < now-30d, 1000/batch, until empty
//!     └── release lock
//!   else: log debug "another replica holds the lock", skip this tick
//! ```
//!
//! 方言差异（设计 §3 归属表 #11/#21/#22）：
//! - 锁：MySQL `GET_LOCK(?,0)`/`RELEASE_LOCK(?)` ↔ PG
//!   `pg_try_advisory_lock(hashtext($1))`/`pg_advisory_unlock(hashtext($1))`。
//!   ⚠️ 语义并非完全等价：`hashtext` 是 int4（32 位），理论存在哈希碰撞；
//!   当前唯一锁名 `iam_outbox_archive`，单键无碰撞面。锁名增多时应改
//!   `pg_try_advisory_lock($1::bigint)` + Rust 侧稳定 64 位哈希。
//! - 时间：MySQL `NOW(3) - INTERVAL ? HOUR` ↔ PG
//!   `CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 hour')`（`INTERVAL $1 HOUR`
//!   在 PG 是语法错误，必须乘法式）。
//!
//! 🔴 两条硬约束（双方言都成立）：
//! 1. 锁与全部归档语句必须持有**同一条连接**（锁是 session-scoped，
//!    走连接池每语句换连接会把锁丢在别的会话上）——因此本模块不走
//!    `db_*!` 宏（宏按池取连接），而是 acquire 单连接后全程使用。
//! 2. PG 侧必须显式 `pg_advisory_unlock`：连接断开虽会自动释放，
//!    但不能依赖「连接池会回收」兜底。

#[allow(unused_imports)]
use tracing::{debug, info, warn};

use digua_common_db::DbPool;

use sqlx::Acquire;
use std::time::Duration;
use tokio::time::interval;

const ARCHIVE_BATCH: i64 = 1000;
const ARCHIVE_TICK: Duration = Duration::from_secs(300); // 5 min
const ARCHIVE_LOCK_NAME: &str = "iam_outbox_archive";
const RETENTION_HOURS: i64 = 24;
const PURGE_DAYS: i64 = 30;

/// 归档 cron 主循环（每 5 分钟一 tick；单实例语义由方言锁保证）。
pub async fn run(pool: DbPool) {
    let mut tick = interval(ARCHIVE_TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    info!(
        interval_secs = ARCHIVE_TICK.as_secs(),
        retention_hours = RETENTION_HOURS,
        purge_days = PURGE_DAYS,
        "outbox archive cron started"
    );
    loop {
        tick.tick().await;
        if let Err(e) = run_once(&pool).await {
            warn!(error = %e, "archive tick failed, retry next interval");
        }
    }
}

// Ponytail: `pub` (not `pub(crate)`) is required for integration tests under
// `tests/`. Doc-hidden so the public lib API surface stays clean.
#[doc(hidden)]
pub async fn run_once(pool: &DbPool) -> sqlx::Result<()> {
    // 进程级互斥（多副本时只有一个跑归档）。方言分派只发生在锁的获取/释放
    // 与时间表达上，两条腿各自是「单连接 + 原生 sqlx」的完整闭环。
    match pool {
        #[cfg(feature = "mysql")]
        DbPool::Mysql(p) => run_once_mysql(p).await,
        // 该臂随 feature 存在性裁剪：未开 `postgres` 时 `DbPool` 没有 Postgres 变体。
        #[cfg(feature = "postgres")]
        DbPool::Postgres(p) => run_once_postgres(p).await,
        #[allow(unreachable_patterns)]
        _ => Err(sqlx::Error::Configuration(
            "outbox_archive: dialect feature not enabled for this pool".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// MySQL 腿（原 services/*/src/outbox_archive.rs 原样迁移）
// ---------------------------------------------------------------------------

#[cfg(feature = "mysql")]
async fn run_once_mysql(pool: &sqlx::MySqlPool) -> sqlx::Result<()> {
    // 必须用单连接：GET_LOCK/RELEASE_LOCK 是 session-scoped，跨连接会泄漏 8 小时。
    let mut conn = pool.acquire().await?;

    let acquired: (i64,) = sqlx::query_as("SELECT GET_LOCK(?, 0)")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(&mut *conn)
        .await?;
    if acquired.0 != 1 {
        debug!("another replica holds archive lock, skipping");
        return Ok(());
    }

    let archived = archive_loop_mysql(&mut conn).await?;
    let purged = purge_loop_mysql(&mut conn).await?;
    report(archived, purged);

    let _: (i64,) = sqlx::query_as("SELECT RELEASE_LOCK(?)")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(&mut *conn)
        .await?;
    Ok(())
}

#[cfg(feature = "mysql")]
async fn archive_loop_mysql(conn: &mut sqlx::MySqlConnection) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        // 1) 拿一个 1000 行的 id 窗口（id 单调递增，BETWEEN 等价 IN 但单 placeholder 对）
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events \
               WHERE published_at IS NOT NULL \
                 AND published_at < NOW(3) - INTERVAL ? HOUR \
               ORDER BY id ASC LIMIT ? \
             ) sub",
        )
        .bind(RETENTION_HOURS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(&mut *conn)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 {
            break;
        }
        let min_id = min_id.expect("cnt>0 → min_id is some");
        let max_id = max_id.expect("cnt>0 → max_id is some");

        // 2) 范围搬运 + 二次 WHERE 守住语义
        let mut tx = conn.begin().await?;
        sqlx::query(
            "INSERT INTO iam_outbox_events_archive \
               (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at) \
             SELECT id, event_id, event_type, routing_key, envelope_json, created_at, published_at, NOW(3) \
             FROM iam_outbox_events \
             WHERE id BETWEEN ? AND ? \
               AND published_at IS NOT NULL \
               AND published_at < NOW(3) - INTERVAL ? HOUR",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(RETENTION_HOURS)
        .execute(&mut *tx)
        .await?;

        let res = sqlx::query(
            "DELETE FROM iam_outbox_events \
             WHERE id BETWEEN ? AND ? \
               AND published_at IS NOT NULL \
               AND published_at < NOW(3) - INTERVAL ? HOUR",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(RETENTION_HOURS)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        total += res.rows_affected();
        if cnt < ARCHIVE_BATCH {
            break; // 最后一批不足，已搬完
        }
    }
    Ok(total)
}

#[cfg(feature = "mysql")]
async fn purge_loop_mysql(conn: &mut sqlx::MySqlConnection) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events_archive \
               WHERE archived_at < NOW(3) - INTERVAL ? DAY \
               ORDER BY id ASC LIMIT ? \
             ) sub",
        )
        .bind(PURGE_DAYS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(&mut *conn)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 {
            break;
        }
        let min_id = min_id.expect("cnt>0");
        let max_id = max_id.expect("cnt>0");

        let res = sqlx::query(
            "DELETE FROM iam_outbox_events_archive \
             WHERE id BETWEEN ? AND ? \
               AND archived_at < NOW(3) - INTERVAL ? DAY",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(PURGE_DAYS)
        .execute(&mut *conn)
        .await?;
        total += res.rows_affected();
        if cnt < ARCHIVE_BATCH {
            break;
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// PostgreSQL 腿（S5d 新增；SQL 形状与 MySQL 腿逐条对应）
// ---------------------------------------------------------------------------

#[cfg(feature = "postgres")]
async fn run_once_postgres(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    // 与 MySQL 腿同款单连接约束：advisory lock 是 session-scoped。
    let mut conn = pool.acquire().await?;

    // hashtext → int4：当前唯一锁名，碰撞面为零（见模块头注释）。
    let acquired: (bool,) = sqlx::query_as("SELECT pg_try_advisory_lock(hashtext($1))")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(&mut *conn)
        .await?;
    if !acquired.0 {
        debug!("another replica holds archive lock, skipping");
        return Ok(());
    }

    let archived = archive_loop_postgres(&mut conn).await?;
    let purged = purge_loop_postgres(&mut conn).await?;
    report(archived, purged);

    let _: (bool,) = sqlx::query_as("SELECT pg_advisory_unlock(hashtext($1))")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(&mut *conn)
        .await?;
    Ok(())
}

/// `INTERVAL $N HOUR` 在 PG 非法（D-7 实测），统一乘法式：
/// `CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 hour')`。
#[cfg(feature = "postgres")]
async fn archive_loop_postgres(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events \
               WHERE published_at IS NOT NULL \
                 AND published_at < CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 hour') \
               ORDER BY id ASC LIMIT $2 \
             ) sub",
        )
        .bind(RETENTION_HOURS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(&mut *conn)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 {
            break;
        }
        let min_id = min_id.expect("cnt>0 → min_id is some");
        let max_id = max_id.expect("cnt>0 → max_id is some");

        let mut tx = conn.begin().await?;
        sqlx::query(
            "INSERT INTO iam_outbox_events_archive \
               (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at) \
             SELECT id, event_id, event_type, routing_key, envelope_json, created_at, published_at, CURRENT_TIMESTAMP(3) \
             FROM iam_outbox_events \
             WHERE id BETWEEN $1 AND $2 \
               AND published_at IS NOT NULL \
               AND published_at < CURRENT_TIMESTAMP(3) - ($3 * INTERVAL '1 hour')",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(RETENTION_HOURS)
        .execute(&mut *tx)
        .await?;

        let res = sqlx::query(
            "DELETE FROM iam_outbox_events \
             WHERE id BETWEEN $1 AND $2 \
               AND published_at IS NOT NULL \
               AND published_at < CURRENT_TIMESTAMP(3) - ($3 * INTERVAL '1 hour')",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(RETENTION_HOURS)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        total += res.rows_affected();
        if cnt < ARCHIVE_BATCH {
            break;
        }
    }
    Ok(total)
}

#[cfg(feature = "postgres")]
async fn purge_loop_postgres(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events_archive \
               WHERE archived_at < CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 day') \
               ORDER BY id ASC LIMIT $2 \
             ) sub",
        )
        .bind(PURGE_DAYS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(&mut *conn)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 {
            break;
        }
        let min_id = min_id.expect("cnt>0");
        let max_id = max_id.expect("cnt>0");

        let res = sqlx::query(
            "DELETE FROM iam_outbox_events_archive \
             WHERE id BETWEEN $1 AND $2 \
               AND archived_at < CURRENT_TIMESTAMP(3) - ($3 * INTERVAL '1 day')",
        )
        .bind(min_id)
        .bind(max_id)
        .bind(PURGE_DAYS)
        .execute(&mut *conn)
        .await?;
        total += res.rows_affected();
        if cnt < ARCHIVE_BATCH {
            break;
        }
    }
    Ok(total)
}

fn report(archived: u64, purged: u64) {
    if archived > 0 || purged > 0 {
        info!(archived, purged, "archive cycle complete");
    } else {
        debug!("archive cycle: nothing to do");
    }
}
