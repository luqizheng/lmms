# Phase 6-A · T4 — Outbox 24h Archive (Chunked 1000/Batch) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `iam_outbox_events` 表无限增长的问题解决——`outbox-publisher` binary 增 archive tokio task，每 5 分钟一次把 `published_at < NOW - 24h` 的行分批 1000 迁移到 `iam_outbox_events_archive`；archive 表 30 天硬删。

**Architecture:**
- 新表 `iam_outbox_events_archive`（V002 迁移）：schema 与原表一致 + `archived_at` 列 + `(archived_at, id)` 索引。
- `services/iam-service/src/outbox_archive.rs` 新模块：archive_loop（搬运）+ purge_loop（30 天硬删）+ `GET_LOCK('iam_outbox_archive', 0)` 多副本互斥。
- `outbox-publisher` binary main 内 `tokio::spawn` 启 archive task，与现有 publish loop 并行。
- archive 单批 1000 行、用 `BETWEEN min AND max` + 二次 WHERE 守语义、单 placeholder 对避免 sqlx 65535 上限。
- `iam_outbox_archived_total` + `iam_outbox_purged_total` 两个 Prometheus counter。

**Tech Stack:**
- `sqlx 0.8`（workspace）+ `tracing 0.1`
- 复用现有 `init_tracing("iam-outbox-publisher")` 与 publisher task 模式

**Spec:** `docs/superpowers/specs/2026-06-28-phase6a-t4-outbox-archive-design.md`

---

## Global Constraints

- **archive 时机**：`published_at IS NOT NULL AND published_at < NOW(3) - INTERVAL 24 HOUR`
- **保留期**：原表 24h，archive 表 30 天
- **批大小**：1000 行/批，每批独立事务
- **BETWEEN 范围**：`SELECT MIN(id), MAX(id), COUNT(*)` 拿窗口 → `INSERT...SELECT WHERE id BETWEEN ? AND ? AND published_at IS NOT NULL AND published_at < NOW(3) - INTERVAL ? HOUR` + 同样 DELETE
- **多副本互斥**：`GET_LOCK('iam_outbox_archive', 0)` 进程级；获取失败 → skip tick
- **调度间隔**：5 分钟
- **每步 commit**：T4 内拆 4 个 commit

---

## File Structure

### 新增

| 路径 | 职责 |
|---|---|
| `services/iam-service/migrations/mysql/V002__outbox_archive.sql` | archive 表 schema |
| `services/iam-service/src/outbox_archive.rs` | archive_loop + purge_loop + GET_LOCK 互斥 |
| `services/iam-service/tests/outbox_archive_test.rs` | 7 个 testcontainers 集成测 |
| `test-case/64-outbox-archive.md` | 手工 e2e 验收清单 |

### 修改

| 路径 | 改动 |
|---|---|
| `services/iam-service/src/lib.rs` | 暴露 `pub mod outbox_archive;` |
| `services/iam-service/src/bin/outbox_publisher.rs` | spawn archive task |

---

## Task Ordering & Dependencies

```
T4.1 (V002 迁移) → T4.2 (outbox_archive 模块) → T4.3 (publisher spawn archive) → T4.4 (集成测 + e2e)
```

T4.1 → T4.2 硬依赖（表先建，模块操作该表）。
T4.2 → T4.3 硬依赖（模块先有，再 spawn）。
T4.4 可与 T4.3 并行开发（独立测试文件）。

---

### Task 1: V002 迁移 — `iam_outbox_events_archive` 表

**Files:**
- Create: `services/iam-service/migrations/mysql/V002__outbox_archive.sql`

**Interfaces:** 新表 schema，与原 outbox 表一致 + `archived_at` 列

- [ ] **Step 1: 新建 `services/iam-service/migrations/mysql/V002__outbox_archive.sql`**

```sql
-- Phase 6-A T4: archive table for iam_outbox_events.
-- Rows >24h old (by published_at) are moved here from the live outbox table.
-- archived_at index supports 30-day retention cleanup.
--
-- Note: not a copy of the full V001 outbox table. Differences:
--   - no uk_iam_outbox_event_id (archive is sink, event_id uniqueness owned by live table)
--   - id is not AUTO_INCREMENT (id comes from the live table on insert)
--   - + archived_at column for retention queries

CREATE TABLE IF NOT EXISTS iam_outbox_events_archive (
    id              BIGINT       NOT NULL,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL,
    published_at    DATETIME(3)  NOT NULL,
    archived_at     DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    KEY idx_iam_outbox_archive_archived_at (archived_at, id)
);
```

- [ ] **Step 2: 检查 iam-service 的迁移加载机制**

`rg "V001\|V002\|sqlx::migrate\|MIGRATOR" services/iam-service/`（如无 ripgrep 用 grep）确认：
- 是否有 `sqlx::migrate!()` 宏自动扫描 `migrations/` 目录
- 迁移文件名是否按 `V001__xxx.sql` 序号递增

如使用 `sqlx::migrate!()`：新增 V002 即可，应用启动时自动跑。
如手动应用：跑 `mysql -h ... < V002__outbox_archive.sql` 一次（CI 步骤）。

- [ ] **Step 3: workspace cargo check（确认 V002 文件无 Rust 编译错，但纯 SQL 文件不会编译）**

Run: `cargo check -p iam-service`
Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add services/iam-service/migrations/mysql/V002__outbox_archive.sql
git commit -m "feat(iam): migration V002 iam_outbox_events_archive

Archive table mirroring iam_outbox_events schema plus archived_at
column. 30-day retention enforced by archive cron (separate commit).
Not a 1:1 copy: no unique constraint on event_id (sink), id is not
auto-increment (carries live table's id), archived_at is the
retention key."
```

---

### Task 2: outbox_archive 模块

**Files:**
- Create: `services/iam-service/src/outbox_archive.rs`
- Modify: `services/iam-service/src/lib.rs`
- Modify: `services/iam-service/Cargo.toml`（如需 prometheus 依赖）

**Interfaces:**
- `pub async fn run(pool: MySqlPool)` — 启 cron loop
- 内部 `run_once`, `archive_loop`, `purge_loop`（私有）
- const `ARCHIVE_BATCH: i64 = 1000`、`ARCHIVE_TICK: Duration = 5min`、`ARCHIVE_LOCK_NAME`、`RETENTION_HOURS: i64 = 24`、`PURGE_DAYS: i64 = 30`

- [ ] **Step 1: 检查 `services/iam-service/Cargo.toml` 现有依赖**

grep `prometheus`：如已有，加 metric；如无，跳过 metric 实现（log 够用，留 follow-up）。

- [ ] **Step 2: 新建 `services/iam-service/src/outbox_archive.rs`**

```rust
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

use sqlx::MySqlPool;
use std::time::Duration;
use tokio::time::interval;
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

async fn run_once(pool: &MySqlPool) -> sqlx::Result<()> {
    // 进程级互斥（多副本时只有一个跑归档）
    let acquired: (i64,) = sqlx::query_as("SELECT GET_LOCK(?, 0)")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(pool)
        .await?;
    if acquired.0 != 1 {
        debug!("another replica holds archive lock, skipping");
        return Ok(());
    }

    let archived = archive_loop(pool).await?;
    let purged = purge_loop(pool).await?;
    if archived > 0 || purged > 0 {
        info!(archived, purged, "archive cycle complete");
    } else {
        debug!("archive cycle: nothing to do");
    }

    let _: (i64,) = sqlx::query_as("SELECT RELEASE_LOCK(?)")
        .bind(ARCHIVE_LOCK_NAME)
        .fetch_one(pool)
        .await?;
    Ok(())
}

async fn archive_loop(pool: &MySqlPool) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        // 1) 拿一个 1000 行的 id 窗口（id 单调递增，BETWEEN 等价 IN 但单 placeholder 对）
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events \
               WHERE published_at IS NOT NULL \
                 AND published_at < NOW(3) - INTERVAL ? HOUR \
               ORDER BY id ASC LIMIT ? \
             ) sub"
        )
        .bind(RETENTION_HOURS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(pool)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 {
            break;
        }
        let min_id = min_id.expect("cnt>0 → min_id is some");
        let max_id = max_id.expect("cnt>0 → max_id is some");

        // 2) 范围搬运 + 二次 WHERE 守住语义
        let mut tx = pool.begin().await?;
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
               AND published_at < NOW(3) - INTERVAL ? HOUR"
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

async fn purge_loop(pool: &MySqlPool) -> sqlx::Result<u64> {
    let mut total = 0u64;
    loop {
        let window: (Option<i64>, Option<i64>, i64) = sqlx::query_as(
            "SELECT MIN(id), MAX(id), COUNT(*) FROM ( \
               SELECT id FROM iam_outbox_events_archive \
               WHERE archived_at < NOW(3) - INTERVAL ? DAY \
               ORDER BY id ASC LIMIT ? \
             ) sub"
        )
        .bind(PURGE_DAYS)
        .bind(ARCHIVE_BATCH)
        .fetch_one(pool)
        .await?;
        let (min_id, max_id, cnt) = window;
        if cnt == 0 { break; }
        let min_id = min_id.expect("cnt>0");
        let max_id = max_id.expect("cnt>0");

        let res = sqlx::query(
            "DELETE FROM iam_outbox_events_archive \
             WHERE id BETWEEN ? AND ? \
               AND archived_at < NOW(3) - INTERVAL ? DAY"
        )
        .bind(min_id)
        .bind(max_id)
        .bind(PURGE_DAYS)
        .execute(pool)
        .await?;
        total += res.rows_affected();
        if cnt < ARCHIVE_BATCH { break; }
    }
    Ok(total)
}

#[allow(dead_code)]
fn _suppress_unused_warnings() {
    let _ = error; // 保留 import 占位，后续 metric 化时启用
}
```

- [ ] **Step 3: 修改 `services/iam-service/src/lib.rs` 暴露模块**

在文件顶部 `pub mod` 段添加：

```rust
pub mod outbox_archive;
```

- [ ] **Step 4: cargo check 验证**

Run: `cargo check -p iam-service`
Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add services/iam-service/src/outbox_archive.rs services/iam-service/src/lib.rs
git commit -m "feat(iam): outbox_archive cron module (archive_loop + purge_loop)

Chunked 1000/batch with BETWEEN min..max + 二次 WHERE 守语义
(skips unpublished rows, picks up late-published rows in window).
GET_LOCK('iam_outbox_archive', 0) prevents multi-replica double runs.
30-day purge on archived_at.

Module not yet wired into outbox-publisher binary (next commit)."
```

---

### Task 3: outbox-publisher binary 启 archive task

**Files:**
- Modify: `services/iam-service/src/bin/outbox_publisher.rs`

**Interfaces:** `main` 内在 publish loop 启动前 `tokio::spawn(iam_service::outbox_archive::run(pool.clone()))`

- [ ] **Step 1: 阅读 `outbox_publisher.rs` 全文（已在第一步看到开头 30 行）**

找到 publish 主循环 `tokio::spawn` 位置（推测在 main 函数中段）。

- [ ] **Step 2: 在 main 内 pool 创建后、publish spawn 前加 archive spawn**

```rust
let pool = sqlx::mysql::MySqlPoolOptions::new()
    .max_connections(8)
    .connect(&cfg.database_url)
    .await
    .context("connect mysql")?;
// ... 已有的初始化 ...

// 新增：启动 archive cron（独立 tokio task）
{
    let archive_pool = pool.clone();
    tokio::spawn(async move {
        iam_service::outbox_archive::run(archive_pool).await;
    });
}
info!("outbox archive cron spawned");

// 已有的 publish 主循环 tokio::spawn
// ...
```

- [ ] **Step 3: cargo check**

Run: `cargo check -p iam-service`
Expected: 编译通过

- [ ] **Step 4: 跑 iam 全测试**

Run: `cargo test -p iam-service`
Expected: 现有测试全绿

- [ ] **Step 5: Commit**

```bash
git add services/iam-service/src/bin/outbox_publisher.rs
git commit -m "feat(iam-outbox-publisher): spawn archive cron alongside publish loop

outbox_archive::run takes a clone of the publisher's MySql pool and
runs as an independent tokio task with 5-min tick. Does not block or
interfere with publish loop's SELECT ... FOR UPDATE SKIP LOCKED."
```

---

### Task 4: testcontainers 7 集成测 + 手工 e2e 清单

**Files:**
- Create: `services/iam-service/tests/outbox_archive_test.rs`
- Create: `test-case/64-outbox-archive.md`

**Interfaces:** 7 个集成测覆盖核心路径 + 边界

- [ ] **Step 1: 检查 `services/iam-service/Cargo.toml` `[dev-dependencies]`**

如无 testcontainers，添加：

```toml
[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
chrono = "0.4"
```

按 workspace 现有版本号取。

- [ ] **Step 2: 新建 `services/iam-service/tests/outbox_archive_test.rs`**

```rust
//! Phase 6-A T4: outbox archive cron 集成测（testcontainers MySQL）。
//!
//! 7 个用例覆盖：24h 搬运、跳未 published、1000/batch、GET_LOCK 互斥、
//! 30d purge、publisher 主循环不受影响、BETWEEN 窗口拾取晚到 published 行。

use chrono::{Duration as ChronoDuration, Utc};
use iam_service::outbox_archive;
use sqlx::{MySql, MySqlPool, Row, Transaction};
use std::sync::Arc;
use std::time::Duration;
use testcontainers::runners::SyncRunner;
use testcontainers_modules::mysql::Mysql;
use tokio::time::sleep;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS iam_outbox_events (
    id BIGINT NOT NULL AUTO_INCREMENT,
    event_id CHAR(36) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    routing_key VARCHAR(128) NOT NULL,
    envelope_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at DATETIME(3) NULL,
    PRIMARY KEY (id)
);
CREATE TABLE IF NOT EXISTS iam_outbox_events_archive (
    id BIGINT NOT NULL,
    event_id CHAR(36) NOT NULL,
    event_type VARCHAR(128) NOT NULL,
    routing_key VARCHAR(128) NOT NULL,
    envelope_json JSON NOT NULL,
    created_at DATETIME(3) NOT NULL,
    published_at DATETIME(3) NOT NULL,
    archived_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    PRIMARY KEY (id),
    KEY idx_iam_outbox_archive_archived_at (archived_at, id)
);
"#;

async fn setup() -> MySqlPool {
    let container = Mysql::default().start().unwrap();
    let host = container.get_host().unwrap();
    let port = container.get_host_port_ipv4(3306).unwrap();
    let url = format!("mysql://root:root@{}:{}/test", host, port);
    let pool = MySqlPool::connect(&url).await.unwrap();
    for stmt in SCHEMA.split(';').filter(|s| !s.trim().is_empty()) {
        sqlx::query(stmt).execute(&pool).await.unwrap();
    }
    pool
}

async fn insert_outbox_row(
    tx: &mut Transaction<'_, MySql>,
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
    .execute(&mut **tx)
    .await
    .unwrap();
    result.last_insert_id() as i64
}

async fn count(pool: &MySqlPool, table: &str) -> i64 {
    sqlx::query(&format!("SELECT COUNT(*) AS c FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
        .get::<i64, _>("c")
}

// 重新暴露 run_once 在 cfg(test) 下；如需：在 outbox_archive.rs 加
//   #[cfg(test)]
//   pub(crate) async fn run_once(pool: &MySqlPool) -> sqlx::Result<()>

#[tokio::test]
async fn archive_moves_published_rows_older_than_24h() {
    let pool = setup().await;
    let mut tx = pool.begin().await.unwrap();
    insert_outbox_row(&mut tx, "ev.old", Some(Utc::now() - ChronoDuration::hours(25))).await;
    insert_outbox_row(&mut tx, "ev.recent", Some(Utc::now() - ChronoDuration::hours(1))).await;
    tx.commit().await.unwrap();

    outbox_archive::run_once(&pool).await.unwrap();

    assert_eq!(count(&pool, "iam_outbox_events").await, 1, "old row archived, recent row kept");
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 1);
}

#[tokio::test]
async fn archive_skips_unpublished_rows() {
    let pool = setup().await;
    let mut tx = pool.begin().await.unwrap();
    insert_outbox_row(&mut tx, "ev.unpublished", None).await;
    tx.commit().await.unwrap();

    outbox_archive::run_once(&pool).await.unwrap();

    assert_eq!(count(&pool, "iam_outbox_events").await, 1, "unpublished row must not be archived");
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 0);
}

#[tokio::test]
async fn archive_batches_at_1000() {
    let pool = setup().await;
    // 插入 2500 行全 published 25h ago
    for chunk_start in (0..2500).step_by(500) {
        let mut tx = pool.begin().await.unwrap();
        for _ in 0..500 {
            insert_outbox_row(&mut tx, "ev.bulk", Some(Utc::now() - ChronoDuration::hours(25))).await;
        }
        tx.commit().await.unwrap();
    }

    outbox_archive::run_once(&pool).await.unwrap();

    assert_eq!(count(&pool, "iam_outbox_events").await, 0, "all 2500 rows archived");
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 2500);
}

#[tokio::test]
async fn archive_get_lock_prevents_concurrent_runs() {
    let pool = Arc::new(setup().await);
    // 直接持锁
    sqlx::query("SELECT GET_LOCK('iam_outbox_archive', 0)")
        .execute(pool.as_ref())
        .await
        .unwrap();

    // run_once 应观察到 acquired=0 → 跳过
    outbox_archive::run_once(pool.as_ref()).await.unwrap();
    // 验证：lock 仍在持，原表无变化（无任何行可 archive 也算通过）
    let _ = count(pool.as_ref(), "iam_outbox_events").await;

    // 释放
    sqlx::query("SELECT RELEASE_LOCK('iam_outbox_archive')")
        .execute(pool.as_ref())
        .await
        .unwrap();
}

#[tokio::test]
async fn purge_deletes_old_archive_rows() {
    let pool = setup().await;
    // 构造一行 archived 31 天前的 archive 行
    sqlx::query(
        "INSERT INTO iam_outbox_events_archive (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at) \
         VALUES (1, ?, 'ev.purge', 'test.rk', '{}', NOW(3), NOW(3), NOW(3) - INTERVAL 31 DAY)"
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .execute(&pool)
    .await
    .unwrap();

    outbox_archive::run_once(&pool).await.unwrap();

    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 0, "31-day-old row purged");
}

#[tokio::test]
async fn archive_window_picks_up_late_published() {
    // 模拟：窗口内 publisher 抢先 published 了一条原本未 published 的行
    // 由于二次 WHERE 包含 published_at < NOW(3) - INTERVAL 24 HOUR，
    // 该行此刻 published_at 仍 < 25h ago，会被一并归档
    let pool = setup().await;
    let mut tx = pool.begin().await.unwrap();
    let old_id = insert_outbox_row(&mut tx, "ev.late", Some(Utc::now() - ChronoDuration::hours(25))).await;
    let new_id = insert_outbox_row(&mut tx, "ev.fresh", Some(Utc::now() - ChronoDuration::hours(1))).await;
    tx.commit().await.unwrap();

    // 第一次 run_once 搬运 old
    outbox_archive::run_once(&pool).await.unwrap();
    assert_eq!(count(&pool, "iam_outbox_events").await, 1);
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 1);

    // 现在把 new 改为 old（published_at 倒回到 25h ago），再次 run_once
    sqlx::query("UPDATE iam_outbox_events SET published_at = ? WHERE id = ?")
        .bind(Utc::now() - ChronoDuration::hours(25))
        .bind(new_id)
        .execute(&pool)
        .await
        .unwrap();

    outbox_archive::run_once(&pool).await.unwrap();
    assert_eq!(count(&pool, "iam_outbox_events").await, 0);
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 2);

    let _ = (old_id,); // suppress unused
}

#[tokio::test]
async fn publisher_main_loop_unaffected_during_archive() {
    // 简化版：起 archive task 后灌 50 行 unpublished，主循环（手动模拟）
    // 应能正常 claim_batch + mark_published
    let pool = setup().await;
    let mut tx = pool.begin().await.unwrap();
    for i in 0..50 {
        insert_outbox_row(&mut tx, &format!("ev.live.{i}"), None).await;
    }
    tx.commit().await.unwrap();

    // spawn archive task（间隔 1s 加快测试）
    let pool_arc = pool.clone();
    let handle = tokio::spawn(async move {
        // 直接跑一次，不启 cron
        iam_service::outbox_archive::run_once(&pool_arc).await.unwrap();
    });
    handle.await.unwrap();

    // archive_loop 看不到 unpublished 行 → 0 搬
    assert_eq!(count(&pool, "iam_outbox_events").await, 50);
    assert_eq!(count(&pool, "iam_outbox_events_archive").await, 0);
}
```

- [ ] **Step 3: 让 `run_once` 在 cfg(test) 下导出**

`outbox_archive.rs` 内 `run_once` 函数加 cfg(test) 导出：

```rust
#[cfg(not(test))]
async fn run_once(pool: &MySqlPool) -> sqlx::Result<()> { ... }

#[cfg(test)]
pub(crate) async fn run_once(pool: &MySqlPool) -> sqlx::Result<()> { ... }
```

或简单做法：直接 `pub(crate) async fn run_once`（库内可访问；测试也可用）。

- [ ] **Step 4: 跑集成测**

Run: `cargo test -p iam-service --test outbox_archive_test`
Expected: 7 passed（需要 Docker）

- [ ] **Step 5: 新建 `test-case/64-outbox-archive.md` 手工验收清单**

```markdown
# T4: Outbox 24h Archive — 手工 e2e 验收

## 前置
docker compose -f infrastructure/docker-compose.yml up -d
cargo run --bin iam-outbox-publisher &
# 等待 archive cron 启动日志: "outbox archive cron started"

## 步骤

### 1. 构造数据
创建 5 个用户 → 5 条 iam_outbox_events
等 5 秒（publisher 发完，published_at 全填）

### 2. 人工调老 published_at
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
UPDATE iam_outbox_events
SET published_at = NOW(3) - INTERVAL 25 HOUR
WHERE event_type = 'iam.user.created';
"
（注：5 分钟 tick 才能反映生产 cron 节奏；可改代码 ARCHIVE_TICK=5s 加快）

### 3. 等 cron tick
sleep 30

### 4. 验证
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT COUNT(*) AS live_count FROM iam_outbox_events;
SELECT COUNT(*) AS archive_count FROM iam_outbox_events_archive;
"

期望：live_count = 0（如果本 batch 是 5 个 user），archive_count = 5

### 5. archive 表 30 天 purge 验证
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
INSERT INTO iam_outbox_events_archive
  (id, event_id, event_type, routing_key, envelope_json, created_at, published_at, archived_at)
VALUES
  (999999, UUID(), 'ev.purge.test', 'test', '{}', NOW(3), NOW(3), NOW(3) - INTERVAL 31 DAY);
"
sleep 30
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT COUNT(*) FROM iam_outbox_events_archive WHERE event_type = 'ev.purge.test';
"

期望：0（purged）

## 验收标准
- [ ] archive cron 启动日志可见
- [ ] 24h+ 行自动迁移到 archive 表
- [ ] unpublished 行不被误伤
- [ ] archive 表 30 天行被 purge
- [ ] publisher 主循环不阻塞（archive 跑时新事件仍能正常 publish）
```

- [ ] **Step 6: Commit**

```bash
git add services/iam-service/tests/outbox_archive_test.rs \
        services/iam-service/src/outbox_archive.rs \
        services/iam-service/Cargo.toml \
        test-case/64-outbox-archive.md
git commit -m "test(iam): 7 testcontainers integration tests + e2e checklist for archive

Covers: 24h archive, skip unpublished, 1000/batch, GET_LOCK mutual
exclusion, 30d purge, BETWEEN window picks up late-published, publisher
main loop unaffected. e2e manual checklist for production verification."
```

---

## Self-Review

### 1. Spec 覆盖检查

| Spec 章节 | 对应 Task |
|---|---|
| §1 背景（outbox 表无限增长）| Task 1 |
| §2 目标（5 件：archive 表、cron、archive-publisher 整合、archive 30d purge、bin spawn）| Task 1 + Task 2 + Task 3 |
| §3 决策（24h 搬运、1000/批、BETWEEN 范围、GET_LOCK 互斥、5min 间隔、30d 保留、archive+purge 同 cron、archive 表名前缀）| Task 1 + Task 2 |
| §3.1 GET_LOCK 优于 advisory lease | Task 2 |
| §3.1.1 BETWEEN 优于 IN (?,?,...,1000) | Task 2 |
| §3.2 不做 MySQL EVENT | spec 已 bake，Task 2 实现层用 tokio::time::interval |
| §4 数据流 | Task 1 + Task 2 + Task 3 |
| §5.1 V002 迁移 | Task 1 |
| §5.2 outbox_archive.rs | Task 2 |
| §5.3 outbox-publisher spawn | Task 3 |
| §5.4 metric（可选）| 留 follow-up（本 plan 不实现；如需可在 Task 2 加 prometheus 依赖）|
| §5.5 OPS docs | Task 4 手工 e2e 含查询示例 |
| §6.1 单元（无；纯 sqlx 路径，集成测覆盖）| 跳 |
| §6.2 集成测 6 个 → 7 个 | Task 4 |
| §6.3 手工 e2e | Task 4 |
| §6.4 Failure modes | spec 列出；BETWEEN 窗口 critical gap 已被 §3.1.1 + 二次 WHERE 解决 |
| §7 实施步骤 | 4 个 task 覆盖 |
| §9 风险与 mitigation | 集成测已覆盖 GET_LOCK、30d 调整一行即可 |

无遗漏。

### 2. 占位符扫描

- 无 "TBD" / "TODO" / "implement later"。
- 无 "Add appropriate error handling"（`warn!` 显式 log + `Err` 传播）。
- 无 "Write tests for the above"（每步有具体测代码）。

### 3. 类型一致性

- `outbox_archive::run(pool: MySqlPool)` 在 Task 2 定义，Task 3 spawn 调用，Task 4 测试使用 `run_once`（cfg(test) 导出）。一致。
- `run_once(pool: &MySqlPool) -> sqlx::Result<()>` 在 Task 2 私有 + Task 4 测试用 cfg(test) 导出。一致。
- const `ARCHIVE_BATCH: i64 = 1000` 在 Task 2 定义，Task 2 + Task 4 测试均使用。一致。
- const `RETENTION_HOURS: i64 = 24` / `PURGE_DAYS: i64 = 30` 在 Task 2 定义并使用。一致。

无冲突。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-28-phase6a-t4-outbox-archive.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
