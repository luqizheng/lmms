# Phase 6-A · T4 — Outbox 24h Archive（Chunked 1000/Batch）

> 日期：2026-06-28
> 来源：TODO Phase 4 broad review 遗留 `I6 iam_outbox_events 无归档 cron（spec §7.3 要求 24h archive）`。
> 上下文：plan review（2026-06-28，SCOPE REDUCTION → Phase 6-A 四件套）。
>
> **Plan-review 结论已 bake（推荐方案）：**
> 4-Q1 B 分批 1000 行/批，循环到空，每批独立事务 ·
> 归档目标 `iam_outbox_events_archive` 同库同 schema，保留 30 天 ·
> archive cron 进 outbox-publisher binary 作为 sub-task（同进程，不新增 binary）。

## 1. 背景与问题

Phase 4 落地的 outbox 表 `iam_outbox_events`（`services/iam-service/migrations/mysql/V001__init.sql:140`）：

```sql
CREATE TABLE iam_outbox_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at    DATETIME(3)  NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_iam_outbox_event_id (event_id),
    KEY idx_iam_outbox_unpublished (published_at, id)
);
```

`outbox-publisher` 把 `published_at IS NULL` 的行发出去并 `UPDATE ... SET published_at = ...`，
**永远不删除**。生产规模下：

- 每天 N 个事件 × envelope_json 平均 1KB = 表无限增长。
- `published_at` 索引项跟着 published row 一起留着，影响 publisher poll `WHERE published_at IS NULL ORDER BY id LIMIT N FOR UPDATE SKIP LOCKED` 的 index efficiency。
- 任何 `OPTIMIZE TABLE` / pt-online-schema-change 都越拖越久。

Phase 4 spec §7.3 已写「24h archive」，落地缺这条 cron。本 spec 补齐。

## 2. 目标 / 非目标

### 目标

- 新建表 `iam_outbox_events_archive`，schema 与 `iam_outbox_events` 完全一致（额外加一列 `archived_at`）。
- 后台 cron：每 5 分钟一次，把 `published_at < NOW() - INTERVAL 24 HOUR` 的行 **分批 1000 行**
  迁移到 archive 表。
- archive 表本身保留 30 天，超过的硬删（DELETE，简单粗暴，不再二级归档）。
- cron 进 `outbox-publisher` 现有 binary（独立 tokio task）；**不**新建二进制。
- 单批事务 < 100ms，避免阻塞 publisher 的 `FOR UPDATE SKIP LOCKED`。

### 非目标（YAGNI）

- **不**支持 cold storage（S3 / OSS）；archive 在同库同实例。
- **不**做按 event_type / routing_key 分表。
- **不**做归档前压缩 envelope_json。
- **不**做查询接口（管理面后续如有需要单独排，本期只管搬数据）。
- **不**给 archive 表加 `event_id` 唯一索引（archive 是 sink，不查重）—— 仅保留主键 id 自增 + `archived_at` 索引。
- **不**做 partition by date（MySQL 8.0 partition 与 FOREIGN KEY 兼容性麻烦；当前规模未到瓶颈）。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|---|---|---|
| 归档时机 | published_at < NOW() - 24h | 24h 内供消费者回放 / 排障 |
| 批大小 | 1000 行/批 | 单事务 ~100ms，与 publisher 抢锁影响最小 |
| 批策略 | 见下方算法：`SELECT MIN(id), MAX(id), COUNT(*)` 拿窗口 → `INSERT...SELECT WHERE id BETWEEN ? AND ? AND published_at < ?` + `DELETE WHERE id BETWEEN ? AND ? AND published_at < ?` | 单 statement 单 placeholder 对，避免 `IN (?, ?, ..., 1000?)` 接近 sqlx 65535 placeholder 上限；二次 WHERE 守住语义防误伤未 published 行 |
| 多副本 | 用同 MySQL `GET_LOCK('iam_outbox_archive', 0)` 进程级别互斥 | 多 outbox-publisher 副本时只有一个跑归档；publisher 主循环不受影响 |
| 调度间隔 | 5 分钟 | 24h 范围内即便堆 12 小时也能在 1 小时内清空 |
| archive 表保留期 | 30 天 | 与生产环境日志保留期对齐 |
| 二级清理（archive → 删除） | 同一 cron 跑完归档后顺手 DELETE archived_at < NOW() - 30 天 LIMIT 1000 直到无行 | 单一进程，单一时间窗口 |
| 失败处理 | 单批事务失败 → log warn，跳出循环到下次 tick | 不阻塞主循环；下次接着搬 |
| 表名前缀 | `iam_outbox_events_archive`（不是 `iam_outbox_archive_events`） | 与 contracts/db-conventions.md 一致：`iam_` 前缀 + 业务表名 + `_archive` 后缀 |

### 3.1 关于 `GET_LOCK` 而非 advisory lease

替代：archive 表加 row-level 锁 / 用 outbox 同表的「lease 列」。
**`GET_LOCK` 更 explicit**：DB 原生 named lock，连接断开自动释放，无 schema 改动，无 lease 过期判定。
session-scope 与归档进程生命周期天然对齐。

### 3.1.1 关于 `BETWEEN` 而非 `IN (?, ?, ..., 1000?)`

`IN` 占位符方案的三个问题：

- **65535 上限**：sqlx + MySQL 单 prepared statement 上限是 65535 placeholder，1000 个不超但接近上限，
  扩到 5000/批就撞墙。
- **优化器开销**：MySQL 8.0 对 1000 个常量 IN 会转 hash 表，比 BETWEEN 范围扫描多一次额外查找。
- **prepared statement 缓存利用率**：每批的 statement text 都因 placeholder 数量变化而稍有不同
  （收尾不足 1000 的最后一批）；BETWEEN 始终 2 个参数，statement 复用率高。

BETWEEN 方案的关键 trade-off：归档期间若有**新的也满足条件的行** 落在 (min,max) 范围内（如 publisher 刚好把
一些 25h+ 前的旧消息发了 + published_at 落在窗口内），会被一并归档——**这是好事**（顺手清干净），但需要
`AND published_at IS NOT NULL AND published_at < NOW(3) - INTERVAL ? HOUR` 二次条件防止误伤未 published 行
或还在保留期内的行。

### 3.2 关于「为啥不用 MySQL EVENT SCHEDULER」（4-Q1 C 否决）

EVENT SCHEDULER 把生产策略埋进 DB DDL：

- 部署需在 DB 上额外执行 CREATE EVENT，跨环境配置漂移风险大。
- 错误日志只在 MySQL error log，应用层不可见。
- 中断 / 关停困难（应用进程退出但 event 仍跑）。

应用层 cron 更符合「explicit over clever」。

## 4. 数据流

```
┌──────────────────────────────────────────────────────────────────┐
│ outbox-publisher binary (单进程，多 tokio task)                  │
│                                                                  │
│  ┌─────────────────┐         ┌─────────────────────────────┐    │
│  │ task 1: publish │         │ task 2: archive cron        │    │
│  │  loop           │         │  ticker every 5 min         │    │
│  └────────┬────────┘         └──────────────┬──────────────┘    │
│           │ SELECT FOR UPDATE              │ GET_LOCK            │
│           │   SKIP LOCKED                  │  (per process)      │
│           ▼                                ▼                     │
└───────────────────────────────────────────────────────────────────┘
            │                                │
            ▼                                ▼
       MySQL iam_outbox_events ◄──reads─────┐
                                            │
            ┌───────────────────────────────┘
            │
            ▼   archive loop (until no rows):
        -- 1) 拿一个 1000 行的 id 窗口（id 连续单调递增）
        SELECT MIN(id), MAX(id), COUNT(*) FROM (
          SELECT id FROM iam_outbox_events
          WHERE published_at IS NOT NULL
            AND published_at < NOW(3) - INTERVAL 24 HOUR
          ORDER BY id ASC LIMIT 1000
        ) sub;
        -- COUNT == 0 → break
        BEGIN;
        -- 2) 范围搬运 + 二次 WHERE 守语义
        INSERT INTO iam_outbox_events_archive (...)
          SELECT ..., NOW(3) FROM iam_outbox_events
          WHERE id BETWEEN ? AND ?
            AND published_at IS NOT NULL
            AND published_at < NOW(3) - INTERVAL 24 HOUR;
        DELETE FROM iam_outbox_events
          WHERE id BETWEEN ? AND ?
            AND published_at IS NOT NULL
            AND published_at < NOW(3) - INTERVAL 24 HOUR;
        COMMIT;
            │
            ▼   purge loop (until no rows):
        SELECT MIN(id), MAX(id), COUNT(*) FROM (
          SELECT id FROM iam_outbox_events_archive
          WHERE archived_at < NOW(3) - INTERVAL 30 DAY
          ORDER BY id ASC LIMIT 1000
        ) sub;
        DELETE FROM iam_outbox_events_archive
          WHERE id BETWEEN ? AND ?
            AND archived_at < NOW(3) - INTERVAL 30 DAY;
```

## 5. 组件改动

### 5.1 新建迁移 `services/iam-service/migrations/mysql/V002__outbox_archive.sql`

```sql
-- Phase 6-A T4: archive table for iam_outbox_events.
-- Rows >24h old (by published_at) are moved here from the live outbox table.
-- archived_at index supports 30-day retention cleanup.
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

**注意：**
- 不复制 `uk_iam_outbox_event_id`（archive 是 sink，event_id 唯一约束在线上表已是真相源）。
- `id` 用线上表的 id（不自增），方便排障关联。

### 5.2 新建 `services/iam-service/src/outbox_archive.rs`

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
    info!("outbox archive cron started (interval=5min, retention=24h, purge=30d)");
    loop {
        tick.tick().await;
        if let Err(e) = run_once(&pool).await {
            warn!(error = %e, "archive tick failed, retry next interval");
        }
    }
}

async fn run_once(pool: &MySqlPool) -> sqlx::Result<()> {
    // 进程级互斥（多副本时只有一个跑归档）
    let acquired: (i64,) =
        sqlx::query_as("SELECT GET_LOCK(?, 0)")
            .bind(ARCHIVE_LOCK_NAME)
            .fetch_one(pool)
            .await?;
    if acquired.0 != 1 {
        debug!("another replica holds archive lock, skipping");
        return Ok(());
    }

    let archived = archive_loop(pool).await?;
    let purged = purge_loop(pool).await?;
    info!(archived, purged, "archive cycle complete");

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
```

`services/iam-service/src/lib.rs` 加 `pub mod outbox_archive;`。

### 5.3 `services/iam-service/src/bin/outbox_publisher.rs` 起 archive task

```rust
// 在主循环 spawn 之前：
let archive_pool = pool.clone();
tokio::spawn(async move {
    iam_service::outbox_archive::run(archive_pool).await;
});

// 然后是已有的 publisher 主循环（不变）
```

注意：归档使用**独立 connection**（pool 内自分配）；publisher 主循环用自己的 connection 跑
`SELECT ... FOR UPDATE SKIP LOCKED`，两者不抢同一 connection 的 lock 状态。`GET_LOCK` 的 session-scope
正好与每个 connection 的生命周期对齐。

### 5.4 Prometheus metric（可选，小成本）

```rust
register_int_counter!(
    "iam_outbox_archived_total",
    "Total rows archived from iam_outbox_events"
)
.expect("metric register");

register_int_counter!(
    "iam_outbox_purged_total",
    "Total rows purged from iam_outbox_events_archive (older than 30d)"
)
.expect("metric register");
```

在 `archive_loop` / `purge_loop` 结尾 `inc_by(total)`。outbox-publisher 已有 `/metrics`（如未有则
本期不补，cron 走 log 也够）。

### 5.5 OPS docs

加一行到 `docs/USER_MANUAL.md` 的 outbox section：

> outbox archive 与 publisher 同进程；查询长期事件历史去 `iam_outbox_events_archive`；
> 表保留 30 天，超过自动删除。

## 6. 测试计划

### 6.1 单元测试 `services/iam-service/tests/outbox_archive_test.rs`（testcontainers MySQL）

```rust
#[tokio::test]
async fn archive_moves_published_rows_older_than_24h() {
    // setup：插入 2 行
    //   id=1: published_at = NOW - 25h
    //   id=2: published_at = NOW - 1h
    // run_once
    // 断言：archive 表有 id=1，原表仅 id=2
}

#[tokio::test]
async fn archive_skips_unpublished_rows() {
    // setup：published_at IS NULL，created_at = NOW - 25h
    // run_once
    // 断言：原表仍有该行，archive 为空
}

#[tokio::test]
async fn archive_batches_at_1000() {
    // setup：插入 2500 行 all published_at = NOW - 25h
    // run_once
    // 断言：archive 2500，原表 0；至少 3 批
}

#[tokio::test]
async fn archive_window_picks_up_late_published() {
    // setup：先插入 id=1..1000 全 published 25h ago
    //        再插入 id=500..600 之间（虚构 — 实际由 INSERT 顺序保证）一些 published 25h ago 的"晚到"行
    //        模拟 BETWEEN(min,max) 窗口内有满足条件的新行
    // run_once：第一次窗口 SELECT MIN/MAX → 1..1000
    //          BETWEEN 1..1000 + 二次 WHERE → 所有窗口内 25h+ published 行被搬走
    // 断言：所有满足条件的行都被搬走，没有 published_at IS NULL / 不足 24h 的行误伤
}

#[tokio::test]
async fn archive_get_lock_prevents_concurrent_runs() {
    // 起两个 sqlx pool 模拟两个副本
    // pool1 先 GET_LOCK
    // pool2 run_once：应观察到 acquired==0 → 跳过
}

#[tokio::test]
async fn purge_deletes_old_archive_rows() {
    // setup：archive 表插入 archived_at = NOW - 31 day
    // run_once
    // 断言：archive 表中该行被删
}

#[tokio::test]
async fn publisher_main_loop_unaffected_during_archive() {
    // 起 publisher 主循环 task + archive task 各 1
    // 灌大量 unpublished 行
    // 5 秒后断言：publisher_published_total > 0（未 starve）
}
```

### 6.2 E2E `test-case/64-outbox-archive.md`（手工验收）

```text
1. 起完整 infra；iam + outbox-publisher 跑起来
2. 创建 5 用户 → 5 条 iam_outbox_events
3. 等 publisher 发完（published_at 全填）
4. 手动 UPDATE iam_outbox_events SET published_at = NOW(3) - INTERVAL 25 HOUR;
5. 等 5 分钟（或缩短 ARCHIVE_TICK=5s 走 dev 模式）
6. SELECT COUNT(*) FROM iam_outbox_events;           -- 期望 0
   SELECT COUNT(*) FROM iam_outbox_events_archive;   -- 期望 5
7. 用 OPTIMIZE TABLE 验证 publisher poll 性能未受退化（select FOR UPDATE 不卡）
```

### 6.3 Failure modes

| 路径 | 失败 | 测试? | 错误处理? | 用户可见? |
|---|---|---|---|---|
| GET_LOCK 已被持 | 副本 B 直接跳过 tick | ✅ 单测 | ✅ debug log | 仅 log |
| archive_loop 单批 commit 失败 | log warn，等下次 tick | ✅（注入断 connection） | ✅ | log 可见 |
| INSERT...SELECT 行数 ≠ DELETE 行数 | 同事务保证一致；BETWEEN + 二次 WHERE 共用相同谓词 | ✅ 隐式 | ✅ 事务回滚 | 不可见 |
| BETWEEN 窗口内意外有新行（如 publisher 抢先 published 一条旧消息落入 min,max 范围） | 二次 WHERE 把 published_at 条件过一遍，新行若也满足条件则一并归档（好事） | ✅ 单测 `archive_window_picks_up_late_published` | ✅ | 不可见 |
| publisher 与 archive 同时锁同 id | publisher 的 FOR UPDATE SKIP LOCKED 与 archive 的 id IN(...) 是不同行集合（archive 找的是 published_at NOT NULL，publisher 找的是 IS NULL） | ✅ 设计上隔离 | n/a | n/a |
| RELEASE_LOCK 漏调（process panic） | MySQL session 关闭自动释放 | ❌（lib 行为） | ✅ MySQL 兜底 | 不可见 |
| 31 天的 archive 不慎被 query 接口反查 | 本 spec 不提供 query 接口 | n/a | n/a | n/a |

无 critical gap。

## 7. 实施步骤

1. 加迁移 `services/iam-service/migrations/mysql/V002__outbox_archive.sql`。
2. 加 `services/iam-service/src/outbox_archive.rs`（archive_loop + purge_loop + GET_LOCK 互斥）。
3. `services/iam-service/src/lib.rs` 暴露 `pub mod outbox_archive;`。
4. `services/iam-service/src/bin/outbox_publisher.rs` 起 archive tokio task。
5. 加 testcontainers 集成测试 `services/iam-service/tests/outbox_archive_test.rs` 7 个（含 BETWEEN 窗口测试）。
6. workspace `cargo build` + `cargo test -p iam-service`。
7. 手工 e2e（test-case/64）。
8. 更新 `docs/USER_MANUAL.md` outbox section。

## 8. 验收

- [ ] 迁移 V002 应用后两表存在。
- [ ] outbox-publisher 启动日志有 `outbox archive cron started`。
- [ ] 6 个 testcontainers 测试全绿（含并发互斥 + BETWEEN 窗口测试）。
- [ ] 手工 e2e 验证：人工调老 published_at → 5 分钟内 archive 表收到。
- [ ] OPTIMIZE / publisher poll 性能无明显退化（archive 不与 publisher poll 抢锁）。

## 9. 风险与 mitigation

| 风险 | 缓解 |
|---|---|
| `GET_LOCK` session 异常 → 死锁残留 | MySQL session 关闭即释放，无需手动清理 |
| archive_loop 跑过头打满 IO | 1000/批 + 单批 ~100ms + 每 tick 跑完一轮就 sleep 5min；可由 metric 看 archived_total |
| 30 天保留期太短 / 太长 | 留作 `const PURGE_DAYS` 顶部常量，调整一行即可；本期 30 天 |
| archive 表 disk 满 | 30 天硬上限保护，DBA 可看 `iam_outbox_purged_total` |
| 迁移 V002 在已落地 V001 的 dev DB 上失败 | `CREATE TABLE IF NOT EXISTS` 幂等；纯加表无 schema 兼容性问题 |
| 多副本 publisher 中 archive lock 漂移 | GET_LOCK 是 instance-scope，所有副本指向同 MySQL 实例 → 单进程持锁安全 |
