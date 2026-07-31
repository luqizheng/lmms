# Phase 4 Cheap Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清掉 Phase 4 后续 4 个低风险技术债：I2（audit-service 死代码）、I5（`MySqlOutboxRepo` 持有优化）、4.3-fu4（consumer `run()` block-on-disconnect 修复）、4.4-fu4（`PermissionService::create` 修 INSERT...RETURNING）。无新功能，纯收尾。

**Architecture:** 4 个独立 task，按服务/库归口，每个 task 一个独立 commit（便于 review/revert/cherry-pick）。Task 间无依赖，可任意顺序实现。TDD 仅适用 4.4-fu4（写失败测试 → 修复 → 通过）；其余 3 项是删代码 / 重构 / 模式移植，无失败测试可写（与 4.3 announcement_run 修复同等待遇）。

**Tech Stack:** Rust, sqlx 0.7 (mysql), tokio, lapin 2.x, axum 0.8 (现有栈)。

## Global Constraints

- Spec 参考：`docs/superpowers/specs/2026-06-28-phase4-cheap-fixes-design.md`。
- **禁止**修改 `libs/digua_common-event/src/consumer.rs` 的 `subscribe_inner`（库核心，仅服务侧使用方改）。
- **禁止**修改 `MySqlOutboxRepo::new` / `with_table` 签名（库 API 兼容，服务侧改用法即可）。
- 所有 commit 之前必须跑全量回归：`cargo build --workspace` + `cargo test --workspace`。
- 表命名规范：不涉及新表。
- `node scripts/contracts-gen.js --check` 必须 in-sync（删除 openapi schema 后必须重生成前端 SDK）。
- `node scripts/check-db-naming.js` 必须 PASS（虽然不涉及新表，但改了 service 代码）。
- 每个 task 末尾必须 commit；commit message 用 type 化的格式（`fix(audit-service): ...` 等）。

---

## File Structure

| 文件 | 责任 | Task |
|------|------|------|
| `services/audit-service/src/services/audit.rs` | 删 `create_operation_log` / `create_login_log` | T1 |
| `services/audit-service/src/models/operation.rs` | 删 `CreateOperationLog` struct | T1 |
| `services/audit-service/src/models/login.rs` | 删 `CreateLoginLog` struct | T1 |
| `services/audit-service/src/openapi.rs` | 删 4 处 schema 引用 | T1 |
| `services/audit-service/tests/models_test.rs` | 删 3 个死测试 | T1 |
| `frontend/src/api/audit/audit.client.ts` | auto-codegen 重生成（手工 fallback） | T1 |
| `services/iam-service/src/state.rs` | 加 `outbox_repo: Arc<MySqlOutboxRepo>` 字段 | T2 |
| `services/iam-service/src/main.rs` | 构造 outbox_repo | T2 |
| `services/iam-service/src/service/user_svc.rs` | 替换 2 处 per-call | T2 |
| `services/notification-service/src/db.rs` | 加 outbox_repo 字段 + 替换 `publish_announcement` per-call | T2 |
| `services/notification-service/src/main.rs` | 构造 outbox_repo | T2 |
| `services/audit-service/src/consumer.rs` | `run()` 改 announcement_run 模式 | T3 |
| `services/notification-service/src/consumer/mod.rs` | `run()` 改 announcement_run 模式 | T3 |
| `services/iam-service/src/service/permission_svc.rs` | `create()` 改 INSERT + re-SELECT + tx | T4 |
| `services/iam-service/tests/permission_create_test.rs` | 新增 create 集成测试 | T4 |
| `TODO.md` | 勾选本批次 4 项 | T1-T4 末尾 |

---

## Task 1: I2 — audit-service 死代码清理

**Files:**
- Modify: `services/audit-service/src/services/audit.rs:14-55`
- Modify: `services/audit-service/src/models/operation.rs:22+`
- Modify: `services/audit-service/src/models/login.rs:21+`
- Modify: `services/audit-service/src/openapi.rs:5,6,28,31`
- Modify: `services/audit-service/tests/models_test.rs:2,3,9,10,44,45,63`
- Regenerate: `frontend/src/api/audit/audit.client.ts`
- Modify: `TODO.md`

**Interfaces:**
- Consumes: 无（纯删除）
- Produces: `audit-service` 内部不再有 `create_operation_log` / `create_login_log` 公开方法 + `CreateOperationLog` / `CreateLoginLog` schema 从 openapi 消失

- [ ] **Step 1: 删 `AuditService` 死方法**

打开 `services/audit-service/src/services/audit.rs`，删除 line 14-55（`create_operation_log` 与 `create_login_log` 方法体）。同时移除 `use crate::models::{... CreateOperationLog, CreateLoginLog, ...}` 中的 `CreateOperationLog` 与 `CreateLoginLog`（`operation.rs` line 22 引用若已删则一并消失）。

预期：保存后 file line 数减少 42 行。

- [ ] **Step 2: 删 `CreateOperationLog` struct**

打开 `services/audit-service/src/models/operation.rs`，删除 `pub struct CreateOperationLog { ... }` 整块（含派生宏 `Serialize, Deserialize, Debug, Clone` 等，参考该 file 内其他 struct 的风格）。若该 struct 单独 use 了 `serde::*` 之外的其他 import，留意不要误删其他 struct 共用的 import。

- [ ] **Step 3: 删 `CreateLoginLog` struct**

打开 `services/audit-service/src/models/login.rs`，删除 `pub struct CreateLoginLog { ... }` 整块。同上注意 import 清理。

- [ ] **Step 4: 删 openapi.rs 中 4 处引用**

打开 `services/audit-service/src/openapi.rs`：
- line 5: `CreateLoginLog` 从 use 列表移除（保留 `LoginLog, PagedLoginLogs`）
- line 6: `CreateOperationLog` 从 use 列表移除（保留 `OperationLog, PagedOperationLogs`）
- line 28: 从 `schemas(...)` 列表移除 `CreateOperationLog,`
- line 31: 从 `schemas(...)` 列表移除 `CreateLoginLog,`

- [ ] **Step 5: 删 models_test.rs 中 3 个死测试**

打开 `services/audit-service/tests/models_test.rs`：
- line 2-3: 从 use 列表移除 `CreateOperationLog, CreateLoginLog`
- 删除 `fn create_operation_log_serialization()` 整函数（line 9 起的整段，含 `#[test]` 标注）
- 删除 `fn create_login_log_serialization()` 整函数
- 删除 file 中可能存在的第三个相关测试（grep `CreateLoginLog` 或 `CreateOperationLog` 应只剩 use 行；若有第三个测试函数，整段删除）

- [ ] **Step 6: 验证编译**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build -p audit-service
```
Expected: `Finished` 0 error。**若有 error**：报告具体 error 信息，不要继续。

- [ ] **Step 7: 跑 audit-service 测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p audit-service
```
Expected: 所有测试 pass（含 `models_test`）。

- [ ] **Step 8: 重新生成前端 SDK**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
node scripts/contracts-gen.js
```
预期：脚本成功，stdout 显示 `frontend/src/api/audit/audit.client.ts` 重写。

- [ ] **Step 9: 验证前端 SDK 自动消失两个方法**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
grep -n "createOperationLog\|createLoginLog" frontend/src/api/audit/audit.client.ts
```
Expected: 无输出。若仍存在，手工删除 `audit.client.ts` 中的 `createOperationLog` / `createLoginLog` 方法体（保留 import 中若仍存在的类型，但通常两个 type 也会被 codegen 一并清掉）。

- [ ] **Step 10: contracts in-sync 验证**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
node scripts/contracts-gen.js --check
```
Expected: `in-sync` 或 PASS（不同项目有不同措辞，但成功状态一致）。

- [ ] **Step 11: 勾选 TODO**

打开 `TODO.md`，找到 Phase 4 broad review 段（"I2 audit-service 仍有 `create_operation_log`/`create_login_log` 死方法..."），把 `- [ ]` 改为 `- [x]`。

- [ ] **Step 12: 全量回归 + Commit**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build --workspace
cargo test --workspace --no-fail-fast 2>&1 | tail -50
node scripts/check-db-naming.js
```
预期：build 0 error；test 全 pass 或仅 token-service testcontainers flake（与 4.4 已知 flake 同类）；naming PASS。

Commit:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
git add services/audit-service/ frontend/src/api/audit/ TODO.md
git commit -m "fix(audit-service): remove dead create_operation_log/create_login_log methods + orphan CreateOperationLog/CreateLoginLog schemas (Phase 4 I2)"
```

---

## Task 2: I5 — `MySqlOutboxRepo` AppState 持有优化

**Files:**
- Modify: `services/iam-service/src/state.rs:7+`（AppState struct）
- Modify: `services/iam-service/src/main.rs`（构造 AppState 处）
- Modify: `services/iam-service/src/service/user_svc.rs:87,313`（两处 per-call 替换）
- Modify: `services/notification-service/src/db.rs`（Db struct 加字段 + `publish_announcement` 替换 line 323）
- Modify: `services/notification-service/src/main.rs`（构造 Db 处）
- Modify: `TODO.md`

**Interfaces:**
- Consumes: `MySqlOutboxRepo` API（不变）
- Produces: 两个服务的状态结构持有 `Arc<MySqlOutboxRepo>`，调用方从 `.clone()` 取得（不再 per-call `new()`）

- [ ] **Step 1: iam-service AppState 加字段**

打开 `services/iam-service/src/state.rs`，在 `pub struct AppState { ... }` 内加字段（参考现有字段风格，复制一行修改）：
```rust
pub outbox_repo: Arc<digua_common_event::MySqlOutboxRepo>,
```
确认 `use std::sync::Arc;` 已存在（若没有就在 file 顶部加）。

- [ ] **Step 2: iam-service main 构造 outbox_repo**

打开 `services/iam-service/src/main.rs`，找到构造 `AppState { ... }` 的位置（一般在 `let state = AppState { ... };`）。在 file 内已有的 pool 定义后增加一行：
```rust
let outbox_repo = Arc::new(digua_common_event::MySqlOutboxRepo::new(pool.clone()));
```
然后在 `AppState { ... }` 中填字段：
```rust
AppState {
    // ... 现有字段 ...
    outbox_repo: outbox_repo.clone(),
}
```
若 main.rs 没有现成的 pool 变量，参考 `outbox_publisher.rs:50-56` 的模式：
```rust
let pool = sqlx::mysql::MySqlPoolOptions::new()
    .max_connections(8)
    .acquire_timeout(Duration::from_secs(5))
    .connect(&db_url).await?;
let outbox_repo = Arc::new(digua_common_event::MySqlOutboxRepo::new(pool.clone()));
```

- [ ] **Step 3: iam-service user_svc.rs 替换 line 87**

打开 `services/iam-service/src/service/user_svc.rs`，找到 line 87 附近（`digua_common_event::MySqlOutboxRepo::new(self.state.db.clone())`），替换为：
```rust
self.state.outbox_repo.clone()
```
（注意：保留对 `self.state.outbox_repo` 的访问上下文，可能需要 `state.outbox_repo` 取决于 impl 块是 `&self` 还是其他。参考 line 87 上下文的 `self` 形式。）

- [ ] **Step 4: iam-service user_svc.rs 替换 line 313**

同上，将 line 313 附近的 `digua_common_event::MySqlOutboxRepo::new(self.state.db.clone())` 替换为 `self.state.outbox_repo.clone()`。

- [ ] **Step 5: notification-service Db/AppState 加字段**

打开 `services/notification-service/src/db.rs`，在 `pub struct Db { ... }` 内加字段：
```rust
pub outbox_repo: Arc<digua_common_event::MySqlOutboxRepo>,
```
确认 `use std::sync::Arc;` 存在。然后在 `Db::new()` / `Db::from_pool()` 等构造函数初始化：
```rust
outbox_repo: Arc::new(digua_common_event::MySqlOutboxRepo::with_table(pool.clone(), "ntf_outbox_events")),
```

- [ ] **Step 6: notification-service publish_announcement 替换**

打开 `services/notification-service/src/db.rs`，找到 line 323 附近（`MySqlOutboxRepo::with_table(self.pool.clone(), "ntf_outbox_events")`），替换为：
```rust
self.outbox_repo.clone()
```

- [ ] **Step 7: 验证编译**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build -p iam-service
cargo build -p notification-service
```
Expected: 都 0 error。

- [ ] **Step 8: 验证测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p iam-service
cargo test -p notification-service
```
Expected: 全 pass。

- [ ] **Step 9: 勾选 TODO + commit**

打开 `TODO.md`，找到 `I5 MySqlOutboxRepo::new(pool.clone()) per-call — 移到 AppState 优化`，`- [ ]` → `- [x]`。

Commit（**按服务拆 2 个独立 commit**）：
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
git add services/iam-service/ TODO.md
git commit -m "refactor(iam-service): hold MySqlOutboxRepo in AppState (Phase 4 I5)"

git add services/notification-service/
git commit -m "refactor(notification-service): hold MySqlOutboxRepo in Db state (Phase 4 I5)"
```

---

## Task 3: 4.3-fu4 — consumer `run()` block-on-disconnect 修复

**Files:**
- Modify: `services/audit-service/src/consumer.rs:19-54`（`run()` + `try_subscribe`）
- Modify: `services/notification-service/src/consumer/mod.rs:19-60`（`run()` + `try_subscribe`）
- Modify: `TODO.md`

**Interfaces:**
- Consumes: `RabbitMqConsumer::connection()`（已存在，`libs/digua_common-event/src/consumer.rs:80`）
- Produces: 两个 `run()` 改持 `RabbitMqConsumer`，`while consumer.connection().status().connected() { sleep(5s) }` 循环

参考实现：`services/notification-service/src/consumer/mod.rs:62-83` `announcement_run()`（已有，4.3 落地）

- [ ] **Step 1: notification-service `run()` 改造**

打开 `services/notification-service/src/consumer/mod.rs`，把现有 `run()`（line 19-37）与 `try_subscribe()`（line 39-60）整体替换为以下形式（**完整复制，不要省略**）：

```rust
pub async fn run(
    pool: MySqlPool,
    redis: RedisCli,
    hub: WsHub,
    event_bus_url: String,
) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(pool.clone(), redis.clone(), hub.clone(), &event_bus_url).await {
            Ok(consumer) => {
                // Hold the consumer (and thus its durable queue + connection)
                // alive, polling connection health. Re-subscribe ONLY on an
                // actual disconnect — never on a timer — otherwise each cycle
                // would redundantly queue_declare/bind/consume on a healthy
                // broker (durable queue is idempotent, so no leak, but pure
                // churn).
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!("notification consumer connection lost, resubscribing");
            }
            Err(e) => {
                warn!(error = %e, "notification consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(
    pool: MySqlPool,
    redis: RedisCli,
    hub: WsHub,
    event_bus_url: &str,
) -> anyhow::Result<RabbitMqConsumer> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    consumer
        .subscribe::<IamUserCreated, _, _>(
            QUEUE_NAME,
            &[BINDING],
            move |env: Envelope<IamUserCreated>| {
                let pool = pool.clone();
                let redis = redis.clone();
                let hub = hub.clone();
                async move { iam_events::handle_iam_user_created(&pool, &redis, &hub, &env).await }
            },
        )
        .await?;
    info!("notification consumer subscribed to {} ({})", QUEUE_NAME, BINDING);
    Ok(consumer)
}
```

**唯一差异 vs announcement_run**：announcement 是 ephemeral（断线即重新声明），本改动的 iam.user.created 是 durable（断线重订阅后 broker 重投未 ack 消息）。其余逻辑相同。

- [ ] **Step 2: audit-service `run()` 改造**

打开 `services/audit-service/src/consumer.rs`，把现有 `run()`（line 19-38）与 `try_subscribe()`（line 40-54）整体替换为：

```rust
pub async fn run(pool: MySqlPool, event_bus_url: String) -> anyhow::Result<()> {
    let backoff = Duration::from_secs(5);
    loop {
        match try_subscribe(pool.clone(), &event_bus_url).await {
            Ok(consumer) => {
                // Hold the consumer alive, polling connection health.
                // Re-subscribe ONLY on actual disconnect — durable queue is
                // idempotent so re-declare is safe, but a healthy broker
                // shouldn't pay the cost every 30s. See notification
                // announcement_run for the ephemeral counterpart.
                while consumer.connection().status().connected() {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                warn!("audit consumer connection lost, resubscribing");
            }
            Err(e) => {
                warn!(error = %e, "audit consumer subscribe failed, retry in 5s");
                tokio::time::sleep(backoff).await;
            }
        }
    }
}

async fn try_subscribe(pool: MySqlPool, event_bus_url: &str) -> anyhow::Result<RabbitMqConsumer> {
    let consumer = RabbitMqConsumer::new(event_bus_url).await?;
    consumer
        .subscribe::<serde_json::Value, _, _>(
            "audit.iam-events",
            &["iam.#"],
            move |env: Envelope<serde_json::Value>| {
                let pool = pool.clone();
                async move { persist(&pool, &env).await }
            },
        )
        .await?;
    info!("audit consumer subscribed to audit.iam-events (iam.#)");
    Ok(consumer)
}
```

注意：原 file 顶部 doc 注释提到 "reconnect on broker bounce... after a short backoff" — 现在的实现仍然 reconnect on disconnect，所以 doc 不变；删掉 line 27-29 注释（"park here for a long time..." 这段在 `while ... connected()` 循环下已无意义）。

- [ ] **Step 3: 验证编译**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build -p audit-service
cargo build -p notification-service
```
Expected: 都 0 error。

- [ ] **Step 4: 验证测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p audit-service
cargo test -p notification-service
```
Expected: 全 pass（行为不变）。

- [ ] **Step 5: 勾选 TODO + commit（按服务拆 2 个 commit）**

打开 `TODO.md`，找到 4.3 follow-up 段（"4.3-fu4 apply the same block-on-disconnect fix..."），`- [ ]` → `- [x]`。

Commit:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
git add services/notification-service/ TODO.md
git commit -m "fix(notification): block-on-disconnect for iam.user.created durable consumer (Phase 4.3-fu4)"

git add services/audit-service/
git commit -m "fix(audit): block-on-disconnect for audit.iam-events durable consumer (Phase 4.3-fu4)"
```

---

## Task 4: 4.4-fu4 — `PermissionService::create` 修 INSERT...RETURNING

**Files:**
- Modify: `services/iam-service/src/service/permission_svc.rs:35-59`
- Create: `services/iam-service/tests/permission_create_test.rs`

**Interfaces:**
- Consumes: `AppState`, `CreatePermissionRequest`
- Produces: 修后的 `PermissionService::create()` —— INSERT + re-SELECT + tx 模式，无 RETURNING 子句

参考实现：同 file `update()` 方法 line 178-194（4.4 task 8 已落地）

- [ ] **Step 1: 写失败测试（覆盖 create 路径）**

创建文件 `services/iam-service/tests/permission_create_test.rs`（**完整代码**，直接复制使用）：

```rust
//! Phase 4.4-fu4 integration test: covers `PermissionService::create` —
//! the INSERT...RETURNING MySQL-8.0 incompatibility that 4.4 task 8 missed.
//!
//! Same testcontainer pattern as `permission_changed_test.rs` (4.4 task 8):
//! real MySQL via testcontainers, apply iam schema, drive
//! `PermissionService::create`, assert the round-trip succeeds and the
//! returned row matches.

use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::db;
use iam_service::service::permission_svc::{CreatePermissionRequest, PermissionService};
use iam_service::state::AppState;
use digua_common_cache::MemoryCache;
use serial_test::serial;
use testcontainers::ImageExt;
use testcontainers_modules::{mysql::Mysql, testcontainers::runners::AsyncRunner};

async fn setup_pool() -> sqlx::MySqlPool {
    let mysql_c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .unwrap();
    let my_port = mysql_c.get_host_port_ipv4(3306).await.unwrap();
    let my_host = mysql_c.get_host().await.unwrap();
    let url = format!("mysql://root@{my_host}:{my_port}/test");
    let pool = digua_common::db::connect(&url, 5).await.unwrap();
    db::ensure_schema(&pool).await.unwrap();
    pool
}

fn make_state(pool: sqlx::MySqlPool) -> AppState {
    let config = Config {
        app: AppConfig { name: "iam-service".into(), host: "0.0.0.0".into(), port: 8080 },
        database: DatabaseConfig { url: "unused".into(), max_connections: 5, min_connections: 1 },
        redis: RedisConfig { url: "redis://127.0.0.1:6379".into(), ttl_sec: 86400, prefix: "superpowers:".into() },
        jwt: JwtConfig {
            private_key_path: "/app/keys/private.pem".into(),
            public_key_path: "/app/keys/public.pem".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig { host: "consul".into(), port: 8500, service_name: "iam-service".into() },
    };
    let cache = MemoryCache::new(10000);
    // pass `None` for redis mgr — create path doesn't touch perm_epoch.
    AppState::new(pool, cache, config, None)
}

#[tokio::test]
#[serial]
async fn create_permission_round_trip() {
    let pool = setup_pool().await;
    let state = make_state(pool);
    let svc = PermissionService::new(state);

    let req = CreatePermissionRequest {
        code: format!("test.perm.{}", uuid::Uuid::new_v4()),
        name: "Test permission".to_string(),
        description: Some("Phase 4.4-fu4 round-trip".to_string()),
        category: Some(0),
        resource: Some("test".to_string()),
        action: Some("read".to_string()),
        sort_order: Some(0),
        status: None,
    };

    let resp = svc.create(req).await.expect("create should succeed under MySQL 8.0");
    assert!(!resp.id.is_empty(), "id must be populated");
    assert_eq!(resp.status, "enabled");
    assert_eq!(resp.name, "Test permission");
    assert_eq!(resp.code.starts_with("test.perm."), true);
}
```

**说明**：
- 模板复制自 `services/iam-service/tests/permission_changed_test.rs`（4.4 task 8 已落地）
- 若 `AppState::new` 签名不一致，参考该 file line 81 的实际用法微调（最常见：redis mgr 是 `Option<redis::aio::ConnectionManager>`，create 路径不需要，传 `None`）
- `serial_test` + `#[serial]` 防止 testcontainer 共享状态干扰（与 4.4 task 8 同款）
- 若 `cargo test -p iam-service --test permission_create_test` 跑时 `AppState::new` 字段不匹配，按编译错误调整

- [ ] **Step 2: 运行测试验证失败**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p iam-service --test permission_create_test
```
预期：**FAIL** with `MySQL error: ... syntax error near "RETURNING"` 或类似 sqlx mysql 不支持 RETURNING 的错误（因为 Step 3 还没改）。

如果 testcontainer 启动失败 / StartupTimeout（与 4.4 task 8 已知 flake 同类），跑 `cargo test -p iam-service --test permission_create_test -- --test-threads=1` 重试。

- [ ] **Step 3: 实现修复（INSERT + re-SELECT + tx）**

打开 `services/iam-service/src/service/permission_svc.rs`，把 `create()` 方法（line 35-59）整体替换为：

```rust
pub async fn create(&self, req: CreatePermissionRequest) -> Result<PermissionResponse, IamError> {
    let permission_id = nanoid!();
    let category = match req.category {
        0 => "menu",
        1 => "button",
        2 => "api",
        _ => "api",
    };

    let mut tx = self.state.db.begin().await?;

    sqlx::query(
        "INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'enabled', NOW(), NOW())"
    )
    .bind(&permission_id)
    .bind(&req.code)
    .bind(&req.name)
    .bind(&req.description)
    .bind(category)
    .bind(&req.resource)
    .bind(&req.action)
    .bind(req.sort_order.unwrap_or(0))
    .execute(&mut *tx)
    .await?;

    let permission: PermissionRow = sqlx::query_as(
        "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ?"
    )
    .bind(&permission_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(self.to_response(&permission))
}
```

注意：当前 file 已 `use sqlx::FromRow;` 但可能没用 `use sqlx::mysql::MySqlQueryResult;` — 这一行不变（不需 MySqlQueryResult）。如 file 顶部 use 中没有 `Transaction` 类型相关 import，无需新增。

- [ ] **Step 4: 验证编译**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build -p iam-service
```
Expected: 0 error。

- [ ] **Step 5: 运行 create 测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p iam-service --test permission_create_test
```
预期：**PASS**（testcontainers 启动 + schema apply + INSERT + SELECT + commit 全通，无 RETURNING 错误）。

- [ ] **Step 6: 全量 iam-service 测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p iam-service
```
Expected: 全 pass（已有 `update` 测试覆盖 RETURNING 修复后的模式）。

- [ ] **Step 7: 勾选 TODO + commit**

打开 `TODO.md`，找到 4.4 follow-up 段（"4.4-fu4 `PermissionService::create` 仍用 MySQL 不支持的 `INSERT ... RETURNING`..."），`- [ ]` → `- [x]`。

Commit:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
git add services/iam-service/
git commit -m "fix(iam): PermissionService::create drop INSERT...RETURNING (MySQL 8.0 incompat); add create integration test (Phase 4.4-fu4)"
```

---

## 最终回归（所有 task 完成后）

- [ ] **全量构建 + 测试**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
cargo build --workspace
cargo test --workspace --no-fail-fast 2>&1 | tail -80
node scripts/check-db-naming.js
node scripts/contracts-gen.js --check
```

预期：
- `cargo build` 0 error
- `cargo test` 全 pass（含 task 1-4 新增）
- `check-db-naming.js` PASS
- `contracts-gen --check` in-sync

- [ ] **Live smoke（可选但推荐）**

按 spec §5.3 live 验收：手 POST `/iam/permissions` 验证 create 路径无 RETURNING 错误；手 docker stop rabbitmq 验证 audit/notification 两个 consumer 5s 内尝试重连。

- [ ] **进度记录（推荐）**

参照 4.4 task 9 模式，在 `.superpowers/sdd/progress.md` 末尾追加 "Phase 4 Cheap Fixes — Progress Ledger"，记录 4 个 task + 4 个 commit + 回归结果。这便于 Phase 4.x 整体回看。

---

## Commit 索引（完成后核对）

期望 4 个 commit（按 T1-T4 顺序）：
1. `fix(audit-service): remove dead create_operation_log/create_login_log methods + orphan CreateOperationLog/CreateLoginLog schemas (Phase 4 I2)`
2. `refactor(iam-service): hold MySqlOutboxRepo in AppState (Phase 4 I5)`
3. `refactor(notification-service): hold MySqlOutboxRepo in Db state (Phase 4 I5)`
4. `fix(notification): block-on-disconnect for iam.user.created durable consumer (Phase 4.3-fu4)`
5. `fix(audit): block-on-disconnect for audit.iam-events durable consumer (Phase 4.3-fu4)`
6. `fix(iam): PermissionService::create drop INSERT...RETURNING (MySQL 8.0 incompat); add create integration test (Phase 4.4-fu4)`

实际为 6 个 commit（I5 + 4.3-fu4 各按服务拆 2 个），便于 per-service review/revert。

---

## 风险与回滚

- **I2、T4 修复是行为正确的修复**，回滚等于把 bug 还原，无风险。
- **I5 是纯重构**，回滚等价于未优化，无功能损失。
- **T3 修复模式与已在线上跑的 announcement_run 完全一致**，风险低；如出问题，回滚 commit，外层 reconnect 5s 仍能恢复订阅。