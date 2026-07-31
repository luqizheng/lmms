# Phase 4 Cheap Fixes — Phase 4 后续小修复合集（批次 1/4）

> Spec ID: phase4-cheap-fixes
> Status: Draft → Review
> Created: 2026-06-28
> Phase: 4.x 小迭代（与 4.1-4.4 同级，但性质是「收尾」，不是新功能）
> 前置: Phase 4 事件总线、Phase 4.3 announcement WS 推送（4.3-fu4 复用其修复模式）、Phase 4.4 permission_changed（4.4-fu4 复用其 update 修复模式）
> Plan: 待 `writing-plans` skill 输出

## 1. 概述与动机

### 问题

Phase 4 main + 4.1-4.4 全部完成并合并后，`TODO.md` 与 broad review 留下 11 个 follow-up。
其中 4 个为**纯技术债 / 低风险修复**，无新功能、无外部行为变化，可在单 spec 内统一处理：

| ID | 来源 | 服务 / 库 | 性质 |
|---|---|---|---|
| **I2** | Phase 4 broad review Minor #I2 | audit-service | 死方法 + 孤儿 schema 清理 |
| **I5** | Phase 4 broad review Minor #I5 | digua_common-event + 3 调用方 | `MySqlOutboxRepo::new(pool.clone())` per-call → AppState 持有 |
| **4.3-fu4** | Phase 4.3 final review follow-up | notification-service + audit-service | consumer `run()` block-on-disconnect 修复（与 announcement_run 同模式）|
| **4.4-fu4** | Phase 4.4 task-9 review follow-up | iam-service | `PermissionService::create` 修 INSERT...RETURNING |

### 显式排除（后续批次处理）

- **4.3-fu1** 大规模扇出批量 INSERT（依赖 4.3-fu2 落地）
- **4.3-fu2** role/dept 接收人解析（依赖 iam 成员查询能力）
- **4.3-fu3** welcome 路径事件驱动 WS 推送（依赖 4.3 后续优化）
- **4.4-fu1** `PUT /iam/users/:id/roles` 写端点（新功能）
- **4.4-fu2** Principal 透传（跨 routes/service 重构）
- **4.4-fu3** gateway perm_epoch 本地短缓存（性能优化）
- **I1** consumer transient/permanent 区分（Phase 5 重型子项目）
- **I4** envelope.trace_id producer 侧填充（Phase 6）
- **I6** iam_outbox_events 归档 cron（运维 / 单独 spec）

### 方案

**单 spec + 单 plan** 覆盖全部 4 项修复。每个修复一个独立 task，每个 task 一个独立 commit
（按服务/库归口，便于 review/revert/cherry-pick）。**禁止 task 间互相依赖**。

## 2. 设计

### 2.1 I2 — audit-service 死方法 + 孤儿 schema 清理

#### 现状

Phase 4 task 15 已删除 `POST /audit/operation` 与 `POST /audit/login` 路由（`services/audit-service/src/routes/operation.rs:1-7`、`routes/login.rs:1-7` 仅剩 GET）。但以下三层仍残存：
- `services/audit-service/src/services/audit.rs:14-55` — `AuditService::create_operation_log`、`create_login_log` 私有方法
- `services/audit-service/src/models/operation.rs:22` — `CreateOperationLog` struct
- `services/audit-service/src/models/login.rs:21` — `CreateLoginLog` struct
- `services/audit-service/src/openapi.rs:5,6,28,31` — schema 引用
- `services/audit-service/tests/models_test.rs:2,3,9,10,44,45,63` — `create_operation_log_serialization`、`create_login_log_serialization` 等 3 个测试
- `frontend/src/api/audit/audit.client.ts:69-71,84-86` — `createOperationLog` / `createLoginLog` 方法
- 各语言 SDK：现状**无 audit-service SDK**（`sdk/dotnet/`、`sdk/java/`、`sdk/openapi/`、`sdk/rust/` 中均无 audit 客户端目录），无需 SDK 清理

#### 决策

完整清理（不留死代码）：
1. 删 `AuditService::create_operation_log` 与 `create_login_log` 方法体（`services/audit-service/src/services/audit.rs:14-55`）
2. 删 `models/operation.rs::CreateOperationLog` struct 与 `models/login.rs::CreateLoginLog` struct（含其派生宏、From impl 等）
3. 删 `openapi.rs` 中对这两个 schema 的 4 处引用（`schemas(...)` 列表与 `components(...)` 列表）
4. 删 `tests/models_test.rs` 中 3 个对应测试函数（保留 `OperationLog` / `LoginLog` / `QueryOperationLogs` / `QueryLoginLogs` / 分页响应等仍被路由使用的 schema 测试）
5. 重新生成前端 SDK：`scripts/contracts-gen.js`（frontend 部分应已从 openapi 自动 codegen；删除 schema 后 `audit.client.ts` 中两方法应自然消失——若未自动消失则手工删除）
6. ~~各语言 SDK~~ — **现状无 audit-service SDK**（已 grep 确认），无需清理

#### 护栏

- `cargo build -p audit-service` 必须通过（route 已删，service 方法删除后无引用 → 编译失败即护栏）
- `cargo test -p audit-service --test models_test` 必须仍通过（删的只是死测试）
- 前端 SDK 重新生成后，`audit.client.ts` 中 `createOperationLog` / `createLoginLog` 方法消失
- `node scripts/contracts-gen.js --check` 仍 in-sync（若不同步会 fail）

#### 风险评估

低。删除的是已删路由的支撑代码，无外部调用，无 API 兼容性问题（公开路由本就不存在这些 endpoint）。

### 2.2 I5 — `MySqlOutboxRepo::new(pool.clone())` 持有优化

#### 现状

`MySqlOutboxRepo` 定义于 `libs/digua_common-event/src/outbox.rs:31-57`，构造签名为：
```rust
pub fn new(pool: MySqlPool) -> Self
pub fn with_table(pool: MySqlPool, table: impl Into<String>) -> Self
```

**调用点**（已复查）：
- `services/iam-service/src/bin/outbox_publisher.rs:56` — **已正确一次性构造**（`let repo = MySqlOutboxRepo::new(pool);`，循环复用），无需改
- `services/iam-service/src/service/user_svc.rs:87` — `create_user` 内 `digua_common_event::MySqlOutboxRepo::new(self.state.db.clone())`（**per-call**）
- `services/iam-service/src/service/user_svc.rs:313` — 第二处 per-call（推测 delete_user 或 login 路径）
- `services/notification-service/src/db.rs:323` — `publish_announcement` 内 `MySqlOutboxRepo::with_table(self.pool.clone(), "ntf_outbox_events")`（**per-call**）
- `services/audit-service/` — **无 outbox repo 调用**（仅消费 audit_events，不发布），无需改

每次调用 clone 一次 `MySqlPool`（`Pool` 内部是 `Arc` 克隆，**极廉价**但仍非零开销）+ 一次 struct 构造。这是 Phase 4 broad review 标记的 Minor #I5 优化项。

#### 决策

**模式选择**：在每个服务的 AppState 持有已构造的 `MySqlOutboxRepo`（Arc 共享），需要时克隆 Arc，不再每次 `pool.clone()` + `new()`。

具体落地（每个服务一份）：
1. **iam-service**：在 `AppState` 加字段 `outbox_repo: Arc<MySqlOutboxRepo>`（默认表 `iam_outbox_events`），`main.rs` 构造 AppState 时一次性初始化；所有调用点从 `state.outbox_repo.clone()` 取 repo。涉及 `user_svc.rs:87` 与 `:313` 两处 per-call。
2. **audit-service**：**无 outbox repo 调用**（仅消费 audit_events，不发布），跳过。
3. **notification-service**：在 `Db` / `AppState` 加字段 `outbox_repo: Arc<MySqlOutboxRepo>`（默认表 `ntf_outbox_events`），构造时一次性初始化；`publish_announcement` 与未来调用点改用 `self.outbox_repo.clone()`。涉及 `db.rs:323` 一处 per-call。

#### 保留 API

`MySqlOutboxRepo::new(pool)` / `with_table(pool, table)` **保留**（库 API 不变），只是 **服务侧不再 per-call 调用**。库的使用方（如 `iam-service/src/bin/outbox_publisher.rs`）改为构造时一次性 `new()`，循环内复用 `Arc`。

#### 护栏

- `cargo build --workspace` 通过（库 API 未变）
- 所有现有 outbox 测试通过（行为不变，仅持有位置变）
- 全量回归：contracts in-sync + naming PASS + 各服务测试 + live e2e-event-flow

#### 风险评估

极低。纯内部重构，行为不变，性能提升（每次 enqueue 省一次 `pool.clone()` + struct 构造）。

### 2.3 4.3-fu4 — consumer `run()` block-on-disconnect 修复

#### 现状

`RabbitMqConsumer::subscribe_inner`（`libs/digua_common-event/src/consumer.rs:91-214`）是**一次性消费**：
```rust
tokio::spawn(async move {
    while let Some(delivery) = consumer.next().await {
        match delivery { ... Err(_) => break, }
    }
});
```
spawn 出的 task 在 stream 出错时 `break` 后退出，但外层 `run()` 函数不知道。

现有 consumer 模式有 2 种：

**模式 A（durable + 重声明型）**：
- `services/audit-service/src/consumer.rs:19-38` `run()` — 外层 reconnect loop，subscribe 成功后 `tokio::time::sleep(30s)` 后再次 `try_subscribe`（重声明 idempotent 队列 + 重绑定 + 新 consumer task）
- `services/notification-service/src/consumer/mod.rs:19-37` `run()`（iam.user.created 消费者）— 同模式

**问题**：每 30s 重声明 + 重绑定 + 丢弃旧 consumer task。**不漏消息**（durable 队列在 broker 侧保留，subscribe 失败重试时拉回未 ack 部分），但**冗余 churn**：
- 每 30s 一次 queue_declare + queue_bind + basic_consume
- 旧 spawn task 在断线时静默退出
- 30s 内若实际断线，最多滞后 30s 才能感知（期间 spawn task 已死，但 durable 队列未 ack 消息在重连后由 broker 重投）

**模式 B（ephemeral + 持有型，4.3 评审已落地）**：
- `services/notification-service/src/consumer/mod.rs:62-83` `announcement_run()` — 已按 4.3 review 修复为：try_subscribe 返回 `RabbitMqConsumer`，外层 `while consumer.connection().status().connected() { sleep(5s) }`，断线才重订阅

#### 决策

**复用 4.3 模式 B 修复模式 A 的两个 `run()`**：
1. `services/audit-service/src/consumer.rs::run()` — 改 `try_subscribe` 返回 `Result<RabbitMqConsumer, anyhow::Error>`，成功后 `while consumer.connection().status().connected() { sleep(5s) }`，断线才 `try_subscribe`
2. `services/notification-service/src/consumer/mod.rs::run()` — 同上

#### 与 announcement_run 的细微差异

announcement 是 ephemeral queue：
- 断线即重新声明（auto-delete，旧队列随连接死掉）
- 30s 计时无意义（每条连接死必须重订阅）

本次修改的是 durable queue：
- 仍可 30s 重声明（idempotent），但改进后改为**事件驱动**（连接断才动）
- `RabbitMqConsumer::connection()` 已存在（`libs/digua_common-event/src/consumer.rs:80-82`），可直接用

#### 护栏（与 4.3 announcement_run 修复同等级别）

4.3 announcement_run 修复 commit `57ac173` 也未新增自动化断线测试，仅靠 live 验证 + 现有测试不变。本期采用同等策略：

1. `cargo build -p audit-service` + `cargo build -p notification-service` 通过
2. `cargo test -p audit-service` + `cargo test -p notification-service` 全通过（行为不变）
3. live 验证：手动 `docker stop rabbitmq` → 看两个 `run()` 是否在 5s 内尝试重连（与 Phase 4 task 16 的 broker-bounce 验收同级别）

**明确不在本期**：lapin mock 注入（4.3 已决定**推迟**到 Phase 5-B 测试加固）。

#### 风险评估

低。模式 B 已在线上跑（notification announcement consumer），可参考。

### 2.4 4.4-fu4 — `PermissionService::create` 修 INSERT...RETURNING

#### 现状

`services/iam-service/src/service/permission_svc.rs:35-59::create()`：
```rust
let permission: PermissionRow = sqlx::query_as(
    "INSERT INTO iam_permissions (...) VALUES (...) RETURNING ..."
)
...
.fetch_one(&self.state.db).await?;
```

MySQL 8.0 **不支持** `INSERT ... RETURNING`（PostgreSQL 专属）。当前代码能编译（sqlx mysql feature 对 RETURNING 不报错）但**运行时会失败**。

4.4 task 8 已把同服务 `update()` 方法改成 INSERT/UPDATE + re-SELECT 模式（`permission_svc.rs:178-194`）。

#### 决策

**完全复用 update 的修复模式**：
```rust
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
.execute(&mut *tx).await?;

let permission: PermissionRow = sqlx::query_as(
    "SELECT id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at, deleted_at FROM iam_permissions WHERE id = ?"
)
.bind(&permission_id)
.fetch_one(&mut *tx).await?;

tx.commit().await?;
Ok(self.to_response(&permission))
```

#### 与 update 模式的差异

update 在 INSERT/UPDATE 前先 SELECT 检查 permission 是否存在（PermissionNotFound 守卫）；create 是 INSERT 新行，无守卫，可直接 INSERT 后 SELECT。

create **当前不使用事务**（`fetch_one(&self.state.db)` 直接用 pool）；改为事务后才与 update 模式对齐。事务保证 INSERT + SELECT 原子性（同 tx 内可见）。

#### 护栏

- `cargo build -p iam-service` 通过
- **新增集成测试**（task 4 实现时执行）：`POST /iam/permissions`（或直接 `PermissionService::create` 单元测试）端到端创建一条 permission，返回包含正确字段的 PermissionResponse，无 `RETURNING` 错误。这是 4.4 task 8 当时**未覆盖的路径**（create 不在 4.4 触发路径故未改），本期补齐。
- 全量回归：contracts in-sync + naming PASS + iam-service 全测试通过

#### 风险评估

低。修复模式与 4.4 task 8 完全相同，已成功先例。

## 3. 测试策略总览

| 修复 | 单元测试 | 集成测试 | Live 验收 |
|---|---|---|---|
| I2 | 删的测试本身是死测试，删除即可 | `cargo test -p audit-service --test models_test` 通过 + `cargo build` 通过 | `node scripts/contracts-gen.js` 重生成后，`audit.client.ts` 中 `createOperationLog` / `createLoginLog` 方法自动消失；`--check` 仍 in-sync |
| I5 | 现有 outbox 测试覆盖行为不变 | `cargo test --workspace` 全通过 | live e2e-event-flow.js 仍能跑通（iam outbox publisher → audit consumer） |
| 4.3-fu4 | 现有 consumer 测试通过 | 同上 | live broker-bounce 验证（与 4.3 ephemeral 测试同等级别，**本期不强求**，留 Phase 5-B） |
| 4.4-fu4 | **新增** PermissionService::create 测试覆盖 RETURNING 修复 | `cargo test -p iam-service` 全通过 | 手 POST `/iam/permissions` 端到端验证 |

## 4. 文件改动一览

| 修复 | 文件 |
|---|---|
| **I2** | `services/audit-service/src/services/audit.rs` · `services/audit-service/src/models/{operation,login}.rs` · `services/audit-service/src/openapi.rs` · `services/audit-service/tests/models_test.rs` · `frontend/src/api/audit/audit.client.ts`（auto-codegen）|
| **I5** | `services/iam-service/src/state.rs` · `services/iam-service/src/main.rs` · `services/iam-service/src/service/user_svc.rs` · `services/notification-service/src/db.rs`（`publish_announcement`）· `services/notification-service/src/main.rs` |
| **4.3-fu4** | `services/audit-service/src/consumer.rs` · `services/notification-service/src/consumer/mod.rs` |
| **4.4-fu4** | `services/iam-service/src/service/permission_svc.rs` · `services/iam-service/tests/` 新增 create 集成测试 |

## 5. 验收

1. **TODO.md**：本批次 4 项勾选
2. **回归全绿**：
   - `node scripts/check-db-naming.js` → PASS
   - `node scripts/contracts-gen.js --check` → in-sync
   - `cargo build --workspace` → 0 error
   - `cargo test --workspace` → 全通过（含本批次新增的 `PermissionService::create` 测试）
3. **Live**：手动 live 验证 e2e-event-flow（iam create_user → audit_events 落库）— 行为不变
4. **Commit 边界**：4 个独立 commit，按服务/库归口
5. **Plan 文档**：`docs/superpowers/plans/2026-06-28-phase4-cheap-fixes.md` 完成（writing-plans skill 输出）

## 6. Follow-ups（明确划出后续批次）

- **批次 2**：notification-service 业务扩面（4.3-fu1 + 4.3-fu3，依赖 4.3-fu2）
- **批次 3**：iam 写端点 + 失效链路（4.4-fu1 + 4.4-fu2 + 4.4-fu3，gateway 缓存）
- **批次 4**：iam 成员查询 + 4.3-fu2 role/dept 解析（独立 brainstorm，依赖关系较深）
- **批次 5**：I6 归档 cron（运维，单独 spec）
- **Phase 5**：I1 consumer transient/permanent 区分（已规划）
- **Phase 6**：I4 envelope.trace_id producer 侧填充（已规划）

## 7. 附录：相关 commit 索引

- 4.3 announcement_run 修复 commit: `57ac173`（参考修复模式）
- 4.4 task 8 update 修复 commit: `4230b06`（参考修复模式）
- 4.4 task 9 progress.md 已记录 `4.4-fu4 PermissionService::create RETURNING bug` follow-up