//! Phase 6-A T4 — outbox archive cron task. Lives inside outbox-publisher
//! binary as a tokio task alongside the publish loop.
//!
//! ```text
//! Every 5 min:
//!   try GET_LOCK('iam_outbox_archive', 0):   acquired
//!     ├── archive rows where published_at < now-24h, 1000/batch, until empty
//!     ├── purge archive rows where archived_at < now-30d, 1000/batch, until empty
//!     └── RELEASE_LOCK
//!   else: log debug "another replica holds the lock", skip this tick
//! ```

use sqlx::{Acquire, MySqlConnection, MySqlPool};
use std::time::Duration;
use tokio::time::interval;
#[allow(unused_imports)]
use tracing::{debug, error, info, warn};

const ARCHIVE_BATCH: i64 = 1000;
const ARCHIVE_TICK: Duration = Duration::from_secs(300); // 5 min
const ARCHIVE_LOCK_NAME: &str = "iam_outbox_archive";
const RETENTION_HOURS: i64 = 24;
const PURGE_DAYS: i64 = 30;

pub async fn run(pool: MySqlPool) {
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
pub async fn run_once(pool: &MySqlPool) -> sqlx::Result<()> {
    // 进程级互斥（多副本时只有一个跑归档）
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

    let archived = archive_loop(&mut conn).await?;
    let purged = purge_loop(&mut conn).await?;
    if archived > 0 || purged > 0 {
        info!(archived, purged, "archive cycle complete");
    } else {
        debug!("archive cycle: nothing to do");
    }

    let _: (i64,) = sqlx::query_as("SELECT RELEASE_LOCK(?)")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(&mut *conn)
        .await?;
    Ok(())
}

async fn archive_loop(conn: &mut MySqlConnection) -> sqlx::Result<u64> {
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
               AND published_at < NOW(3) - INTERVAL ? HOUR"
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

async fn purge_loop(conn: &mut MySqlConnection) -> sqlx::Result<u64> {
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
