# Phase 6-A · T1 — Envelope.trace_id Producer Auto-Inject Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让所有 outbox producer 的 `Envelope.trace_id` 自动从当前 tracing span 注入，5 处现有 callsite 零业务代码改动，跨服务 trace 链在 Jaeger 端到端可见。

**Architecture:**
- 把 `current_trace_id()` 函数从 `common-event` 迁到 `common-telemetry`（与 OTel SDK 依赖内聚）。
- 抽 `fill_trace_id(env, source)` 内部 helper：把"取 trace_id"与"写入 envelope"两件事解耦，单元测试可注入 fake source。
- `MySqlOutboxRepo::enqueue` 内调 helper，caller 已填的尊重、已开 OTel 的取当前 span、单元测试/无 OTel 环境返 None 不写假值。
- `OutboxRepo` trait 签名从 `&Envelope<P>` 改为 `&mut Envelope<P>`（必须能写回 trace_id）。5 处 callsite `let envelope` → `let mut envelope` + `&envelope` → `&mut envelope`。

**Tech Stack:**
- Rust workspace（workspace Cargo.toml 锁版本）
- `tracing 0.1`、`tracing-subscriber 0.3`、`opentelemetry 0.24`、`opentelemetry_sdk 0.24`、`tracing-opentelemetry 0.25`、`sqlx 0.8`
- `testcontainers` 0.x（已在本仓库使用，集成测试模式与 `permission_changed_test` 对齐）

**Spec:** `docs/superpowers/specs/2026-06-28-phase6a-t1-trace-id-injection-design.md`

---

## Global Constraints

- **OTel SDK 版本**：`opentelemetry 0.24`、`opentelemetry_sdk 0.24`、`tracing-opentelemetry 0.25`（与 api-gateway 现用版本对齐，见 `services/api-gateway/Cargo.toml`）。
- **`current_trace_id()` 位置**：放 `libs/common-telemetry`，不进 `common-event`。
- **trait 改动**：`OutboxRepo::enqueue` 第二参数从 `&Envelope<P>` 改为 `&mut Envelope<P>`。`common-event/src/outbox.rs:20-24`。
- **6 处 callsite 必须改 mut**：`user_svc.rs:78,306` + `auth_svc.rs:106,345` + `perm_change.rs:31`（iam-service 内 5 处）+ **`notification-service/src/db.rs:265`（publish_announcement → ntf_outbox_events 写 NotificationAnnouncementPublished 事件，1 处）**。T1 Task 2 实测发现 plan 漏列第 6 处，必须本 task 一并修。
- **另外 4 处 common-event tests 也要改**：`libs/common-event/tests/outbox_repo.rs:63,93,98` + `libs/common-event/tests/rabbit_integration.rs:127`（T1 Task 4 实测发现）。总 10 处。
- **caller 已填的尊重**：`.with_trace_id(...)` 写过的 envelope 永不被自动注入覆盖。
- **无 span context → None**：绝不在 envelope 里写假 trace_id。
- **每步 commit**：T1 内拆 5 个 commit，按 task 边界自然分开。
- **不引 OTel 到 outbox-publisher**（binary 端不需要 trace_id 取）。
- **cargo check 全过 + 三层测试全绿 + e2e traceparent 透传验证**。

---

## File Structure

### 新增

| 路径 | 职责 |
|---|---|
| `libs/common-telemetry/src/trace_id.rs` | `current_trace_id()` 函数（OTel feature 开关） |
| `libs/common-telemetry/tests/current_trace_id_test.rs` | 纯函数测 3 case |
| `libs/common-event/tests/enqueue_trace_id_test.rs` | 注入逻辑 mock 测 3 case |
| `libs/common-event/tests/enqueue_writes_trace_id_to_db.rs` | testcontainers 集成测 1 case |
| `scripts/e2e-trace-id-flow.js` | e2e：traceparent → audit.trace_id |
| `test-case/61-trace-id-injection.md` | 手工验收清单 |

### 修改

| 路径 | 改动 |
|---|---|
| `libs/common-telemetry/Cargo.toml` | 加 opentelemetry + tracing-opentelemetry 依赖（feature-gated） |
| `libs/common-telemetry/src/lib.rs` | 暴露 `current_trace_id` |
| `libs/common-event/Cargo.toml` | 加 `common-telemetry` 依赖 |
| `libs/common-event/src/outbox.rs` | trait 改 `&mut` + `fill_trace_id` helper + 实现内调 helper |
| `services/iam-service/src/service/user_svc.rs` | 2 处 `let mut` + `&mut envelope` |
| `services/iam-service/src/service/auth_svc.rs` | 2 处 `let mut` + `&mut envelope` |
| `services/iam-service/src/service/perm_change.rs` | 1 处 `let mut` + `&mut envelope` |
| `services/notification-service/src/db.rs` | 1 处 `let mut` + `&mut envelope`（publish_announcement → ntf_outbox_events） |
| `libs/common-event/tests/outbox_repo.rs` | 3 处 `let mut env` + `&mut env` |
| `libs/common-event/tests/rabbit_integration.rs` | 1 处 `let mut env` + `&mut env` |

---

## Task Ordering & Dependencies

```
Task 1 → Task 2 → Task 3 → Task 4 → Task 5
T1.1   T1.2     T1.3     T1.4     T1.5
  ↓      ↓        ↓        ↓        ↓
common-telemetry 加 current_trace_id + 纯函数测
                    ↓
        common-event 改 trait + fill_trace_id helper + 注入逻辑 mock 测
                              ↓
                              5 处 callsite 改 mut + workspace cargo check
                                       ↓
                                       testcontainers 集成测 + e2e + 手工验收
```

---

### Task 1: 在 common-telemetry 加 `current_trace_id()` 与纯函数测

**Files:**
- Modify: `libs/common-telemetry/Cargo.toml`
- Modify: `libs/common-telemetry/src/lib.rs`
- Create: `libs/common-telemetry/src/trace_id.rs`
- Create: `libs/common-telemetry/tests/current_trace_id_test.rs`

**Interfaces:**
- Produces: `common_telemetry::current_trace_id() -> Option<String>`（otlp feature 内有效；无 otlp 时恒返 None）

- [ ] **Step 1: 修改 `libs/common-telemetry/Cargo.toml` 加 OTel feature deps**

```toml
[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true

# OTel SDK deps — opt-in via `otlp` feature
opentelemetry = { version = "0.24", optional = true }
opentelemetry_sdk = { version = "0.24", optional = true, features = ["rt-tokio"] }
tracing-opentelemetry = { version = "0.25", optional = true }

[dev-dependencies]
# 测试中需要起 TracerProvider
opentelemetry = { version = "0.24", features = ["trace"] }
opentelemetry_sdk = { version = "0.24", features = ["rt-tokio", "testing"] }
tracing-opentelemetry = "0.25"

[features]
default = []
otlp = ["dep:opentelemetry", "dep:opentelemetry_sdk", "dep:tracing-opentelemetry"]
```

> 用 `testing` feature 让 TracerProvider 可 inline 测试（否则 `TracerProvider::builder` 在测试中编译不过）。

- [ ] **Step 2: 新建 `libs/common-telemetry/src/trace_id.rs`**

```rust
//! 从当前 tracing span 取 W3C TraceId 字符串。
//!
//! 与 trace_request_middleware（middleware.rs）的 trace_id 取法对齐：
//! 通过 tracing-opentelemetry 的 `OpenTelemetrySpanExt::context()`
//! 拿到 OTel context，再从 span_context() 取 trace_id。
//! `is_valid()` 为 false（未接 OTel / 无 root span / 测试场景）时返回 None，绝不伪造。

#[cfg(feature = "otlp")]
pub fn current_trace_id() -> Option<String> {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let span = tracing::Span::current();
    let cx = span.context();
    let sc = cx.span().span_context().clone();
    if sc.is_valid() {
        Some(sc.trace_id().to_string())
    } else {
        None
    }
}

#[cfg(not(feature = "otlp"))]
pub fn current_trace_id() -> Option<String> {
    None
}
```

- [ ] **Step 3: 修改 `libs/common-telemetry/src/lib.rs`**

在文件末尾添加：

```rust
mod trace_id;
pub use trace_id::current_trace_id;
```

- [ ] **Step 4: 新建 `libs/common-telemetry/tests/current_trace_id_test.rs`**

```rust
//! 纯函数测：current_trace_id() 在三种 span context 下返回正确结果。
//! 用 `tracing::subscriber::with_default(...)` scope 隔离，避免全局 provider 污染。

use opentelemetry::trace::{Tracer, TracerProvider as _};
use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn run_with_otel_layer<F: FnOnce() -> R, R>(f: F) -> R {
    let provider = TracerProvider::builder().build();
    let tracer = provider.tracer("test");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);
    tracing::subscriber::with_default(subscriber, f)
}

#[test]
fn returns_trace_id_inside_active_span() {
    run_with_otel_layer(|| {
        let _g = tracing::info_span!("test").entered();
        let tid = common_telemetry::current_trace_id();
        assert!(tid.is_some(), "expected Some(trace_id) inside active span");
    });
}

#[test]
fn returns_none_with_otel_subscriber_but_no_span() {
    run_with_otel_layer(|| {
        // 不进入 span，subscriber 已就绪但无 active span
        assert!(common_telemetry::current_trace_id().is_none());
    });
}

#[test]
fn returns_none_without_subscriber() {
    // 无 with_default scope，tracing 默认全局 subscriber 不带 OTel layer
    // Span::current() 返回 invalid span context
    assert!(common_telemetry::current_trace_id().is_none());
}
```

- [ ] **Step 5: 跑测试验证**

Run: `cargo test -p common-telemetry --test current_trace_id_test`
Expected: 3 passed

- [ ] **Step 6: 跑无 otlp feature 编译验证**

Run: `cargo check -p common-telemetry --no-default-features`
Expected: 编译通过

Run: `cargo check -p common-telemetry --features otlp`
Expected: 编译通过

- [ ] **Step 7: Commit**

```bash
git add libs/common-telemetry/
git commit -m "feat(common-telemetry): add current_trace_id() with otlp feature gate

Pure function that returns the current tracing span's W3C trace_id, or
None when no valid OTel span context exists. Uses feature=otlp so CLI
binaries (outbox-publisher) don't pull in OTel SDK.

Three unit tests cover: span active + otel, no span + otel, no otel."
```

---

### Task 2: common-event 加 common-telemetry 依赖 + 改 OutboxRepo trait + 抽 fill_trace_id helper

**Files:**
- Modify: `libs/common-event/Cargo.toml`
- Modify: `libs/common-event/src/outbox.rs`
- Modify: `libs/common-event/src/lib.rs`（如需）

**Interfaces:**
- `OutboxRepo::enqueue<P>(&self, tx, env: &mut Envelope<P>) -> Result<(), OutboxError>` — 第二参数改 `&mut`
- 内部 `fill_trace_id<P, F>(env: &mut Envelope<P>, source: F) where F: Fn() -> Option<String>` — 不可导出，模块内使用

- [ ] **Step 1: 修改 `libs/common-event/Cargo.toml` 加 common-telemetry 依赖**

找到 `[dependencies]` 段，在最末追加：

```toml
common-telemetry = { path = "../common-telemetry", default-features = false, features = ["otlp"] }
```

> 不带 `default-features` 避免 transitive 的 `axum` 等被带入（即便 common-telemetry 现在 default 啥都没有，是个好习惯）。

- [ ] **Step 2: 修改 `libs/common-event/src/outbox.rs` — 改 trait 签名**

找到第 19-29 行的 trait 定义：

```rust
#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError>;

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}
```

改为：

```rust
#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &mut Envelope<P>,           // 改 &mut
    ) -> Result<(), OutboxError>;

    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}
```

- [ ] **Step 3: 在 outbox.rs 顶部加 `fill_trace_id` helper**

在 `use` 块下方、`pub struct OutboxRow` 上方添加：

```rust
/// Internal helper: fill `env.trace_id` from `source()` only when caller left it None.
/// Extracted so tests can inject a fake source without an OTel runtime.
/// Caller-supplied trace_id (via `Envelope::with_trace_id(...)`) is always respected.
fn fill_trace_id<P: Event, F: Fn() -> Option<String>>(env: &mut Envelope<P>, source: F) {
    if env.trace_id.is_none() {
        if let Some(tid) = source() {
            env.trace_id = Some(tid);
        }
    }
}
```

- [ ] **Step 4: 修改 `MySqlOutboxRepo::enqueue` — 调用 fill_trace_id**

找到第 60-82 行的 `MySqlOutboxRepo::enqueue` impl：

```rust
#[async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError> {
        let json = serde_json::to_value(env)?;
        // ... INSERT 语句 ...
```

改为：

```rust
#[async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &mut Envelope<P>,
    ) -> Result<(), OutboxError> {
        fill_trace_id(env, common_telemetry::current_trace_id);
        let json = serde_json::to_value(&*env)?;  // 解引用避免 &mut 与 serde 冲突
        // ... INSERT 语句（同原样）...
```

注意 INSERT 语句内的 `&*env`、`.bind(env.event_id.to_string())` 等均保持原样，因为 `env` 仍是 `&mut` 但已 mut 借用回填过 trace_id，后续序列化只用 `&*env` 解引用为 `&Envelope<P>`。

- [ ] **Step 5: workspace cargo check（应全部编译失败，因 callsite 仍是 `&envelope`）**

Run: `cargo check --workspace`
Expected: **失败**，错误在 `user_svc.rs:87-88`、`user_svc.rs:313-314`、`auth_svc.rs:106` 附近，提示 `expected &mut Envelope, found &Envelope`

> 这是预期失败，callsite 还没改。Task 3 改 callsite 后会过。

- [ ] **Step 6: Commit**

```bash
git add libs/common-event/Cargo.toml libs/common-event/src/outbox.rs
git commit -m "feat(common-event): OutboxRepo::enqueue takes &mut Envelope; auto-inject trace_id

Breaking trait signature change: &Envelope<P> -> &mut Envelope<P>.
Required so MySqlOutboxRepo::enqueue can fill trace_id from current OTel
span before serializing to outbox row JSON.

Internal fill_trace_id() helper extracted so unit tests can inject a
fake source instead of standing up a real OTel provider."
```

---

### Task 3: 5 处 callsite 改 `let mut` + `&mut envelope`

**Files:**
- Modify: `services/iam-service/src/service/user_svc.rs`
- Modify: `services/iam-service/src/service/auth_svc.rs`
- Modify: `services/iam-service/src/service/perm_change.rs`
- Modify: `services/notification-service/src/db.rs`
- Modify: `libs/common-event/tests/outbox_repo.rs`
- Modify: `libs/common-event/tests/rabbit_integration.rs`

**Interfaces:** 无新增。仅消费 Task 2 改后的 trait 签名。

- [ ] **Step 1: 改 `services/iam-service/src/service/user_svc.rs` — create（line 78）**

找到第 78-89 行：

```rust
let envelope = Envelope::new(
    "iam-service",
    IamUserCreated {
        user_id: user_id.clone(),
        username: req.username.clone(),
        email: req.email.clone(),
        org_unit_id: None,
    },
);
self.state.outbox_repo.clone()
    .enqueue(&mut tx, &envelope)
    .await
    .map_err(|e| IamError::Internal(e.to_string()))?;
```

改为：

```rust
let mut envelope = Envelope::new(
    "iam-service",
    IamUserCreated {
        user_id: user_id.clone(),
        username: req.username.clone(),
        email: req.email.clone(),
        org_unit_id: None,
    },
);
self.state.outbox_repo.clone()
    .enqueue(&mut tx, &mut envelope)
    .await
    .map_err(|e| IamError::Internal(e.to_string()))?;
```

- [ ] **Step 2: 改 `user_svc.rs` — delete（line 306）**

找到第 306-316 行：

```rust
let envelope = Envelope::new(
    "iam-service",
    IamUserDeleted {
        user_id: user_id.to_string(),
        deleted_by: "system".to_string(),
    },
);
self.state.outbox_repo.clone()
    .enqueue(&mut tx, &envelope)
    .await
    .map_err(|e| IamError::Internal(e.to_string()))?;
```

改为：

```rust
let mut envelope = Envelope::new(
    "iam-service",
    IamUserDeleted {
        user_id: user_id.to_string(),
        deleted_by: "system".to_string(),
    },
);
self.state.outbox_repo.clone()
    .enqueue(&mut tx, &mut envelope)
    .await
    .map_err(|e| IamError::Internal(e.to_string()))?;
```

- [ ] **Step 3: 改 `services/iam-service/src/service/auth_svc.rs` — login_succeeded（line 106）**

打开文件，搜索 `let envelope = Envelope::new(` 出现的两个地方（行 106 和 345 附近），分别：

- `let envelope` → `let mut envelope`
- `.enqueue(&mut tx, &envelope)` → `.enqueue(&mut tx, &mut envelope)`

> 注意：login_succeeded 与 login_failed 的 Envelope 构造可能在同一函数内的不同位置，
> 需要 grep 全文两处都改。

- [ ] **Step 4: 改 `auth_svc.rs` — login_failed（line 345）**

与 Step 3 相同模式改另一处。

- [ ] **Step 5: 改 `services/iam-service/src/service/perm_change.rs` — publish_perm_change（line 31）**

找到 helper 函数签名（推测为 `pub async fn publish_perm_change<...>` 或类似）：

```rust
pub async fn publish_perm_change(...) -> Result<...> {
    // ...
    let env = Envelope::new(...);
    repo.enqueue(tx, &env).await?;
    // ...
}
```

改为：

```rust
pub async fn publish_perm_change(...) -> Result<...> {
    // ...
    let mut env = Envelope::new(...);
    repo.enqueue(tx, &mut env).await?;
    // ...
}
```

具体签名按文件实际形态调整；helper 内调用 `repo.enqueue(tx, ...)` 的位置都要把 `&env` 改 `&mut env` + 把 `let env` 改 `let mut env`。

> ⚠️ 该 helper 的签名本身可能在 Phase 4.4-fu4 已改成接受 `&MySqlOutboxRepo` 而非 `&MySqlPool`（见 TODO.md I5-fu1 commit 5bfb2da）。以当前 on-disk 代码为准。

- [ ] **Step 6: 改 `services/notification-service/src/db.rs` — publish_announcement（line 265）**

打开文件，定位 `publish_announcement` 函数内的 `Envelope::new` + `.enqueue(&mut tx, &envelope)`（推测在第 265-278 行附近）：

```rust
let envelope = Envelope::new(
    "notification-service",
    NotificationAnnouncementPublished { ... },
);
self.outbox_repo.clone()
    .enqueue(&mut tx, &envelope)
    .await
    .map_err(...)?;
```

改为：

```rust
let mut envelope = Envelope::new(
    "notification-service",
    NotificationAnnouncementPublished { ... },
);
self.outbox_repo.clone()
    .enqueue(&mut tx, &mut envelope)
    .await
    .map_err(...)?;
```

> 这是 plan 漏列的第 6 处 callsite（plan v1 只列了 iam-service 5 处），T1 Task 2 实测发现。
> notification-service 有独立的 `ntf_outbox_events` 表（不是 iam 的 `iam_outbox_events`），
> 但 `OutboxRepo` trait 共享（每个 service 实现各自表的 MySqlOutboxRepo），所以同样要走 fill_trace_id。

- [ ] **Step 6.5: 修 `libs/common-event/tests/` 下 2 个现有测试文件的 callsite**

T1 Task 2 实测发现 `cargo check -p common-event --tests` 失败，`cargo check` 不带 `--tests` 不报，但 `cargo test` 必报。涉及：

| 文件 | 行 | 现状 | 改后 |
|---|---|---|---|
| `libs/common-event/tests/outbox_repo.rs` | 63、93、98 | `repo.enqueue(&mut tx, &env).await.unwrap()` × 3 | `let mut env = Envelope::new(...); repo.enqueue(&mut tx, &mut env).await.unwrap()` |
| `libs/common-event/tests/rabbit_integration.rs` | 127 | 同上 | 同上 |

> `pub_.publish(&env)`（rabbit_integration.rs:87）不动 — `Publisher::publish` 签名仍是 `&Envelope<P>`，T1 只改了 `OutboxRepo::enqueue`。

每个文件的 3 处 / 1 处：找到 `Envelope::new(...)` 后 `let env`，改成 `let mut env`；找到 `.enqueue(&mut tx, &env)` 改成 `.enqueue(&mut tx, &mut env)`。机械 refactor，无业务逻辑改动。

- [ ] **Step 7: workspace cargo check + tests 编译**

Run: `cargo check --workspace --tests`
Expected: 全部编译通过（lib + 所有 tests 都没 `&Envelope` 不匹配错误）

> 仅 `cargo check --workspace`（不带 --tests）会漏报这 2 个 test 文件的错误；必须带 `--tests`。

- [ ] **Step 8: 跑 iam-service + notification-service 现有集成测试**

Run: `cargo test -p iam-service --test integration_test --test permission_changed_test --test permission_create_test`
Expected: 全绿

Run: `cargo test -p notification-service`
Expected: 全绿（如有 docker 则含 welcome_consumer_test 等）

> 如果本地 MySQL/testcontainers 不可用，可只跑 `cargo check -p iam-service -p notification-service`。
> 同样，如本机无 Docker，T1 Task 5 的 enqueue_writes_trace_id_to_db 也用 `#[ignore]` 跳过。

- [ ] **Step 9: Commit**

```bash
git add services/iam-service/src/service/ services/notification-service/src/db.rs \
        libs/common-event/tests/outbox_repo.rs libs/common-event/tests/rabbit_integration.rs
git commit -m "refactor(iam|notification|common-event-tests): 10 Envelope callsites take &mut (Phase 6-A T1)

Mechanical: let envelope -> let mut envelope, &envelope -> &mut envelope.
No business logic changes. Required by OutboxRepo::enqueue signature
update to allow fill_trace_id() to write envelope.trace_id.

5 iam-service sites + 1 notification-service site (publish_announcement
-> ntf_outbox_events for NotificationAnnouncementPublished) +
3 sites in libs/common-event/tests/outbox_repo.rs +
1 site in libs/common-event/tests/rabbit_integration.rs.

Plan v1 missed the notification-service site (T1 Task 2 surfaced) and
the 4 common-event test sites (T1 Task 4 surfaced). All 10 sites
discovered by sequential task verification."
```

---

### Task 4: 注入逻辑 mock 测 3 case

**Files:**
- Create: `libs/common-event/tests/enqueue_trace_id_test.rs`

**Interfaces:** 测试 `fill_trace_id(env, source)` 的三个分支（无 otlp / 已填 / 应填）

> 注意：`fill_trace_id` 是模块私有函数。要么：
> (a) 加 `#[cfg(test)]` 导出，或
> (b) 把 helper 抽到 `pub(crate)` 子模块 `internal::fill_trace_id`。
>
> 选 (b) 干净，但需要小幅重构 outbox.rs。本 plan 选 (a) — 编译期 cfg test，不影响生产 binary。

- [ ] **Step 1: 修改 `libs/common-event/src/outbox.rs` 把 fill_trace_id 在 cfg(test) 下导出**

找到 helper 定义：

```rust
fn fill_trace_id<P: Event, F: Fn() -> Option<String>>(env: &mut Envelope<P>, source: F) {
    ...
}
```

改为：

```rust
/// Internal helper: fill `env.trace_id` from `source()` only when caller left it None.
/// Extracted so tests can inject a fake source without an OTel runtime.
/// Caller-supplied trace_id (via `Envelope::with_trace_id(...)`) is always respected.
///
/// `pub` is required for integration tests under `tests/` (separate compilation
/// unit — `pub(crate)` and `#[cfg(test)]` are not visible). `#[doc(hidden)]`
/// keeps the helper out of rustdoc and signals "implementation detail".
#[doc(hidden)]
pub fn fill_trace_id<P: Event, F: Fn() -> Option<String>>(
    env: &mut Envelope<P>,
    source: F,
) {
    if env.trace_id.is_none() {
        if let Some(tid) = source() {
            env.trace_id = Some(tid);
        }
    }
}
```

> 或更简洁：保留单一签名、加 `#[cfg(test)] pub(crate) use ...`。本 step 选重复定义以保持 diff 小。

- [ ] **Step 2: 新建 `libs/common-event/tests/enqueue_trace_id_test.rs`**

```rust
//! 注入逻辑测：fill_trace_id 的三个分支。
//! 不起 OTel、不起 MySQL，纯单元测分支行为。

use common_event::envelope::Envelope;
use common_event::event::Event;

// 测试 fixture payload
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct TestPayload;

impl Event for TestPayload {
    fn event_type() -> &'static str { "test.payload" }
    fn schema_version() -> u32 { 1 }
}

#[test]
fn fills_trace_id_when_none_and_source_some() {
    let mut env = Envelope::new("test", TestPayload);
    assert!(env.trace_id.is_none(), "precondition: fresh envelope has no trace_id");

    // 通过 outbox 的内部 fill_trace_id，注入 fake source
    common_event::outbox::fill_trace_id(&mut env, || Some("test-trace-id".into()));

    assert_eq!(env.trace_id.as_deref(), Some("test-trace-id"));
}

#[test]
fn respects_caller_trace_id_when_already_set() {
    let mut env = Envelope::new("test", TestPayload)
        .with_trace_id("caller-fixed-id");

    // source 即便返 Some 也不应覆盖
    common_event::outbox::fill_trace_id(&mut env, || Some("should-be-ignored".into()));

    assert_eq!(env.trace_id.as_deref(), Some("caller-fixed-id"));
}

#[test]
fn leaves_none_when_source_returns_none() {
    let mut env = Envelope::new("test", TestPayload);

    common_event::outbox::fill_trace_id(&mut env, || None);

    assert!(env.trace_id.is_none(), "expected None when source returns None");
}
```

- [ ] **Step 3: 跑测试验证**

Run: `cargo test -p common-event --test enqueue_trace_id_test`
Expected: 3 passed

- [ ] **Step 4: Commit**

```bash
git add libs/common-event/tests/enqueue_trace_id_test.rs libs/common-event/src/outbox.rs
git commit -m "test(common-event): fill_trace_id unit tests cover 3 branches

No OTel runtime, no MySQL — pure mock tests for the trace_id injection
helper. Confirms: source=Some fills when env None, source=Some respects
caller-set value, source=None leaves env None."
```

---

### Task 5: testcontainers 集成测 + e2e + 手工验收

**Files:**
- Create: `libs/common-event/tests/enqueue_writes_trace_id_to_db.rs`
- Create: `scripts/e2e-trace-id-flow.js`
- Create: `test-case/61-trace-id-injection.md`

**Interfaces:** 测试 `MySqlOutboxRepo::enqueue` 真的把 trace_id 写进 outbox 表 + 端到端 traceparent 透传

- [ ] **Step 1: 新建 `libs/common-event/tests/enqueue_writes_trace_id_to_db.rs`**

```rust
//! 端到端：testcontainers MySQL，构造 envelope.trace_id=Some("test-trace")，
//! 调真 MySqlOutboxRepo::enqueue，读回 envelope_json，断言 trace_id 字段正确入库。

use common_event::envelope::Envelope;
use common_event::outbox::{MySqlOutboxRepo, OutboxRepo};
use common_event::event::Event;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;
use testcontainers::runners::SyncRunner;
use testcontainers_modules::mysql::Mysql;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestPayload;
impl Event for TestPayload {
    fn event_type() -> &'static str { "test.payload" }
    fn schema_version() -> u32 { 1 }
}

async fn setup_pool() -> MySqlPool {
    let container = Mysql::default().start().unwrap();
    let host = container.get_host().unwrap();
    let port = container.get_host_port_ipv4(3306).unwrap();
    let url = format!("mysql://root:root@{}:{}/test", host, port);
    let pool = MySqlPool::connect(&url).await.unwrap();

    sqlx::query(
        "CREATE TABLE iam_outbox_events (
            id BIGINT NOT NULL AUTO_INCREMENT,
            event_id CHAR(36) NOT NULL,
            event_type VARCHAR(128) NOT NULL,
            routing_key VARCHAR(128) NOT NULL,
            envelope_json JSON NOT NULL,
            created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
            published_at DATETIME(3) NULL,
            PRIMARY KEY (id)
        )"
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn enqueue_writes_caller_supplied_trace_id_to_db() {
    let pool = setup_pool().await;
    let repo = MySqlOutboxRepo::new(pool.clone());

    let mut env = Envelope::new("test", TestPayload)
        .with_trace_id("test-trace-fixed");

    let mut tx = pool.begin().await.unwrap();
    repo.enqueue(&mut tx, &mut env).await.unwrap();
    tx.commit().await.unwrap();

    let row: (serde_json::Value,) = sqlx::query_as(
        "SELECT envelope_json FROM iam_outbox_events ORDER BY id DESC LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let parsed = row.0;
    assert_eq!(
        parsed.get("trace_id").and_then(|v| v.as_str()),
        Some("test-trace-fixed"),
        "trace_id should round-trip through JSON column"
    );
}
```

> 测试要 `dev-dependencies`：`testcontainers`、`testcontainers-modules`、`tokio = { features = ["macros", "rt-multi-thread"] }`。如果 common-event 已有，加进去；否则需要 spec 之外的轻微调整。

- [ ] **Step 2: 检查 `libs/common-event/Cargo.toml` 加 testcontainers dev-dependencies**

打开文件，如果 `[dev-dependencies]` 段无 `testcontainers` 系列，追加：

```toml
[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde_json = "1"
```

具体版本以 workspace 现有用法对齐（grep `testcontainers` 看其他 service 测试用法）。

- [ ] **Step 3: 跑集成测**

Run: `cargo test -p common-event --test enqueue_writes_trace_id_to_db`
Expected: 1 passed（需要 Docker daemon）

如果本机无 Docker：跳过此测试，标注 `[ignore]` 即可，本 plan 在 CI 环境跑。

- [ ] **Step 4: 新建 `scripts/e2e-trace-id-flow.js`**

参考 `tests/e2e` 已有的 e2e script 风格（Vitest + supertest 跨网关测真服务），写一个 e2e 用例：

```js
// scripts/e2e-trace-id-flow.js
// 验收 T1: 跨服务 traceparent 透传后，audit 数据库 envelope_json->>'$.trace_id' 与调用方 traceparent 一致。
import { test, expect } from 'vitest';
import request from 'supertest';
import { execSync } from 'node:child_process';
import mysql from 'mysql2/promise';

const GATEWAY = process.env.GATEWAY_URL || 'http://localhost:3000';
const MYSQL = process.env.MYSQL_URL || 'mysql://root:rootpass@localhost:3306/superpowers';

test('iam.user.created event carries caller's W3C trace_id end-to-end', async () => {
  const traceId = 'aabbccddeeff00112233445566778899';
  const traceparent = `00-${traceId}-1122334455667788-01`;

  // 1. 触发业务事件：创建用户，带 traceparent header
  await request(GATEWAY)
    .post('/v1/iam/users')
    .set('traceparent', traceparent)
    .send({
      username: `e2e_trace_${Date.now()}`,
      password: 'hunter2',
      nickname: 'e2e',
    })
    .expect(201);

  // 2. 等 outbox publisher + audit consumer 处理完（生产 ~ 5s）
  await new Promise((r) => setTimeout(r, 5000));

  // 3. 断言 audit 库 envelope_json.trace_id == traceId
  const conn = await mysql.createConnection(MYSQL);
  const [rows] = await conn.execute(
    `SELECT envelope_json FROM audit_events WHERE event_type = 'iam.user.created' ORDER BY id DESC LIMIT 1`
  );
  await conn.end();

  expect(rows.length).toBeGreaterThan(0);
  const env = typeof rows[0].envelope_json === 'string'
    ? JSON.parse(rows[0].envelope_json)
    : rows[0].envelope_json;
  expect(env.trace_id).toBe(traceId);
});
```

> 实际 endpoints 与 event 表名以仓库现状为准。脚本可放在 `tests/e2e/trace-id.spec.js` 而非
> scripts/ 下，按已有 e2e 模式挂入 `pnpm test:e2e`。

- [ ] **Step 5: 新建 `test-case/61-trace-id-injection.md` 手工验收清单**

```markdown
# T1: envelope.trace_id producer 自动注入 — 手工验收

## 前置
- docker compose -f infrastructure/docker-compose.yml up -d
- observability overlay（可选）

## 步骤

### 1. 单元测试
cd /home/kudi/projects/templates/low-memory-micro-services
cargo test -p common-telemetry --test current_trace_id_test
cargo test -p common-event --test enqueue_trace_id_test
cargo test -p common-event --test enqueue_writes_trace_id_to_db  # 需要 Docker
期望：3+3+1 全绿

### 2. iam-service 集成测试回归
cargo test -p iam-service
期望：全绿

### 3. 端到端 traceparent 透传
# 起 iam + outbox-publisher + audit-service
# 用带 traceparent 的 curl 触发用户创建
TRACE_ID="aabbccddeeff00112233445566778899"
TRACEPARENT="00-${TRACE_ID}-1122334455667788-01"

curl -X POST http://localhost:3000/v1/iam/users \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "traceparent: ${TRACEPARENT}" \
  -H "Content-Type: application/json" \
  -d '{"username":"trace_test","password":"hunter2"}'

# 等 5s
sleep 5

# 在 MySQL 里查 audit_events
mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT JSON_EXTRACT(envelope_json, '\$.trace_id') AS trace_id
FROM audit_events
WHERE event_type = 'iam.user.created'
ORDER BY id DESC LIMIT 1;"

期望：trace_id == aabbccddeeff00112233445566778899

### 4. 反例（无 traceparent）
curl -X POST http://localhost:3000/v1/iam/users \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"username":"no_trace_test","password":"hunter2"}'

# 等 5s
sleep 5

mysql -h 127.0.0.1 -u root -prootpass superpowers -e "
SELECT JSON_EXTRACT(envelope_json, '\$.trace_id') AS trace_id
FROM audit_events WHERE event_type = 'iam.user.created'
ORDER BY id DESC LIMIT 1;"

期望：trace_id 非空（fallback 32 hex，与 traceparent T1 不同）

## 验收标准
- [ ] 单元测 3+3 全绿
- [ ] 集成测 1 全绿（需 Docker）
- [ ] iam-service 集成测试无回归
- [ ] traceparent 透传验证 trace_id 一致
- [ ] 无 traceparent 验证 fallback trace_id 非空且不同
```

- [ ] **Step 6: workspace 全测试**

Run: `cargo test --workspace`
Expected: 全绿（T1 改动对其他 crate 不应有回归）

- [ ] **Step 7: Commit**

```bash
git add libs/common-event/tests/enqueue_writes_trace_id_to_db.rs \
        libs/common-event/Cargo.toml \
        scripts/e2e-trace-id-flow.js \
        test-case/61-trace-id-injection.md
git commit -m "test(common-event): integration test + e2e + manual checklist for T1

- testcontainers MySQL: enqueue writes trace_id to JSON column
- e2e script: gateway traceparent -> iam -> outbox -> audit trace_id roundtrip
- test-case/61: manual acceptance steps"
```

---

## Self-Review

按 writing-plans §Self-Review 检查：

### 1. Spec 覆盖检查

| Spec 章节 | 对应 Task |
|---|---|
| §1 背景（5 处 callsite）| Task 3 |
| §2 目标（自动注入 + 尊重 caller + 无 span → None + 5 处零改动）| Task 2 + Task 3 + Task 5 |
| §3 决策（注入点 / 取法 / 编码 / 已填尊重 / 无 OTel 不写）| Task 1 + Task 2 + Task 4 |
| §3.1 trait 改 `&mut` | Task 2 |
| §5.1 common-telemetry 加 current_trace_id | Task 1 |
| §5.2 outbox.rs 改 trait + fill_trace_id helper | Task 2 + Task 4 |
| §5.3 5 处 callsite 改 mut | Task 3 |
| §6.1.A 纯函数测 | Task 1 Step 4 |
| §6.1.B 注入逻辑测 | Task 4 |
| §6.1.C 集成测 | Task 5 |
| §6.2 e2e | Task 5 Step 4 |
| §6.3 test-case | Task 5 Step 5 |
| §7 实施步骤 | 5 个 task 覆盖 |

无遗漏。

### 2. 占位符扫描

- 无 "TBD" / "TODO" / "implement later"。
- 无 "Add appropriate error handling"（已具体到错误处理类型）。
- 无 "Write tests for the above"（每步都有具体 test code）。
- 无 "Similar to Task N"（每 task 都独立完整）。

### 3. 类型一致性

- `current_trace_id() -> Option<String>` 在 Task 1 定义，Task 2 调用，Task 1 测试使用。一致。
- `OutboxRepo::enqueue` 签名在 Task 2 改 `&mut Envelope<P>`，Task 3 callsite 改 `&mut envelope`，Task 5 集成测 `&mut env`。一致。
- `fill_trace_id<P, F>` 在 Task 2 定义 + Task 4 测试 + Task 5 集成测间接使用。一致。

无冲突。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-28-phase6a-t1-trace-id-injection.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
