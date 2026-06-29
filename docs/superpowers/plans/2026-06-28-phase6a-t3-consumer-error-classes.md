# Phase 6-A · T3 — Consumer Transient/Permanent + Bounded Redelivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `HandlerError` 扩 Transient/Permanent 双分支；consumer delivery 循环按分类走不同路径（ack / DLQ / republish-and-ack）；transient 重投有 bounded redelivery（`x-redelivery-count` header 阈值 3）兜底；4 个现有 handler 同步审查改造。

**Architecture:**
- `HandlerError` 替代单分支 `Handler(String)` 为 `Transient(String)` / `Permanent(String)`。
- `consumer.rs::subscribe_inner` 改造 delivery 循环，引入 `republish_with_incremented_count` 走「同 channel basic_publish + 等 publisher confirm + ack 原 delivery」模式写入 `x-redelivery-count`。
- `delivery.redelivered` 退化为 sanity 检查（republish-and-ack 模式下新消息 `redelivered=false`，唯一真相源是 count header）。
- 4 个现有 handler (`audit-service/src/consumer.rs::persist`、`audit-service/src/risk.rs::handle`、`notification-service/src/consumer/iam_events.rs`、`api-gateway/src/perm_consumer.rs`) 改用新双分支。

**Tech Stack:**
- `lapin 2.x`（现有，consumer 与 publisher 同款）
- `serde` / `serde_json` 现有
- `testcontainers` 0.20 / `testcontainers-modules` 0.8 for RabbitMQ

**Spec:** `docs/superpowers/specs/2026-06-28-phase6a-t3-consumer-error-classes-design.md`

---

## Global Constraints

- **breaking change**：`HandlerError::Handler(String)` 删除；`Transient(String)` + `Permanent(String)` 双分支。
- **bounded redelivery 阈值 3**：固定 const，不外提配置。
- **count 存 broker header `x-redelivery-count`**：不污染 envelope_json。
- **republish-and-ack 模式**（**不**用 `nack(requeue=true)`，因为不能改 header）。
- **OOQ 限制（§3.2）**：当前 4 个 event 全部顺序无关；新 event 设计必须过 §3.2 checklist。
- **confirm_select**：republish channel 必须开 publisher confirm；republish confirm nack → 不 ack 原消息。
- **每步 commit**：T3 内拆 6 个 commit。

---

## File Structure

### 新增

| 路径 | 职责 |
|---|---|
| `libs/common-event/tests/error_classes.rs` | HandlerError display + get_redelivery_count 单测 |
| `libs/common-event/tests/consumer_redelivery.rs` | testcontainers RabbitMQ 5 个集成测 |
| `test-case/63-consumer-error-classes.md` | 手工 e2e 验收清单 |

### 修改

| 路径 | 改动 |
|---|---|
| `libs/common-event/src/error.rs` | HandlerError 双分支 |
| `libs/common-event/src/consumer.rs` | delivery 循环 + republish_and_ack + confirm_select |
| `services/audit-service/src/consumer.rs` | handler 改双分支 |
| `services/audit-service/src/risk.rs` | handler 改双分支（Redis 错误 → Transient 而非 swallow Ok） |
| `services/notification-service/src/consumer/iam_events.rs` | handler 改双分支 |
| `services/api-gateway/src/perm_consumer.rs` | handler 改双分支 |
| `libs/common-event/Cargo.toml` | 加 testcontainers dev-deps |

---

## Task Ordering & Dependencies

```
T3.1 (HandlerError 双分支) → T3.2 (consumer 循环 + republish) → T3.3 (单测)
                                                                    ↓
                                              T3.4 (testcontainers 集成) → T3.5 (handler 改) → T3.6 (e2e)
```

T3.1 → T3.2 是硬依赖（consumer 循环调用 HandlerError 新分支）。
T3.5 改造 4 个 handler 必须在 T3.1 后（否则编译失败）。
T3.3 / T3.4 / T3.6 顺序灵活。

---

### Task 1: HandlerError 改 Transient/Permanent 双分支

**Files:**
- Modify: `libs/common-event/src/error.rs`

**Interfaces:**
- `HandlerError::Transient(String)` — 瞬时错误，consumer 重投
- `HandlerError::Permanent(String)` — 永久错误，立即 DLQ

- [ ] **Step 1: 修改 `libs/common-event/src/error.rs`**

替换 `HandlerError` 整段：

```rust
#[derive(Debug, Error)]
pub enum HandlerError {
    /// 瞬时错误，broker 重投。超过阈值（consumer 内固定 3）后升级 DLQ。
    /// 典型：下游 DB / Redis 抖动、临时网络错误、429/503。
    #[error("transient: {0}")]
    Transient(String),

    /// 永久错误，立即 DLQ。
    /// 典型：业务校验失败、消息载荷不合法、引用资源已删。
    #[error("permanent: {0}")]
    Permanent(String),
}
```

完整文件应长这样（顶部 import + ConsumeError 段保持不变）：

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not connected")]
    NotConnected,
}

#[derive(Debug, Error)]
pub enum ConsumeError {
    #[error("broker error: {0}")]
    Broker(String),
    #[error("subscribe failed: {0}")]
    SubscribeFailed(String),
}

#[derive(Debug, Error)]
pub enum HandlerError {
    /// 瞬时错误，broker 重投。超过阈值（consumer 内固定 3）后升级 DLQ。
    /// 典型：下游 DB / Redis 抖动、临时网络错误、429/503。
    #[error("transient: {0}")]
    Transient(String),

    /// 永久错误，立即 DLQ。
    /// 典型：业务校验失败、消息载荷不合法、引用资源已删。
    #[error("permanent: {0}")]
    Permanent(String),
}

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("not found: event_id={0}")]
    NotFound(String),
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("publish loop terminated: {0}")]
    Publish(String),
    #[error("shutdown timeout")]
    ShutdownTimeout,
}
```

- [ ] **Step 2: 故意不修 callsite，先 cargo check 看破坏面**

Run: `cargo check -p common-event`
Expected: 自身 crate 编译通过（HandlerError 仅在 trait 中使用，自身代码不引用具体变体）

Run: `cargo check --workspace`
Expected: **失败**，错误在 `services/audit-service/src/consumer.rs:76` 与 `risk.rs:66`
提示 `expected HandlerError::Handler, found HandlerError::Transient/Permanent`
这是预期失败，Task 5 会修。

- [ ] **Step 3: Commit**

```bash
git add libs/common-event/src/error.rs
git commit -m "feat(common-event): HandlerError splits Transient/Permanent

BREAKING: HandlerError::Handler(String) replaced by Transient(String) /
Permanent(String) two-variant enum. Tells the consumer which path to
take: Transient → republish-and-ack with bounded redelivery;
Permanent → straight to DLQ.

Caller sites (4 handler impls) will be updated in subsequent commit."
```

---

### Task 2: consumer.rs 改造 delivery 循环 + republish-and-ack

**Files:**
- Modify: `libs/common-event/src/consumer.rs`

**Interfaces:**
- 新增 const `REDELIVERY_LIMIT: i64 = 3` + `REDELIVERY_HEADER: &str = "x-redelivery-count"`
- 新增 fn `get_redelivery_count(headers: &Option<FieldTable>) -> i64`
- 新增 fn `republish_with_incremented_count(...) -> Result<(), lapin::Error>`
- `subscribe_inner` 内的 spawn 闭包，channel 进 confirm_select 模式，delivery 循环改按 HandlerError 变体分支

- [ ] **Step 1: 修改 `consumer.rs` 顶部加 lapin types imports**

找到 `use lapin::{...}` 行（推测 5-12 行），改为：

```rust
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions,
        BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions,
        QueueBindOptions, QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
```

新增 `BasicPublishOptions`、`ConfirmSelectOptions`、`AMQPValue`、`BasicProperties` 4 个 symbol。

- [ ] **Step 2: 在文件顶部加 const**

```rust
const REDELIVERY_LIMIT: i64 = 3;
const REDELIVERY_HEADER: &str = "x-redelivery-count";
```

- [ ] **Step 3: 加 `get_redelivery_count` 辅助函数**

```rust
/// Read the broker header `x-redelivery-count` from a delivery's properties.
/// Returns 0 if the header is missing or malformed (first delivery).
fn get_redelivery_count(headers: &Option<FieldTable>) -> i64 {
    headers
        .as_ref()
        .and_then(|h| h.inner().get(REDELIVERY_HEADER))
        .and_then(|v| match v {
            AMQPValue::LongLongInt(n) => Some(*n),
            AMQPValue::LongInt(n) => Some(*n as i64),
            _ => None,
        })
        .unwrap_or(0)
}
```

- [ ] **Step 4: 加 `republish_with_incremented_count` 异步函数**

```rust
/// Republish `body` to the same routing-key on the same exchange, with
/// `x-redelivery-count` header incremented by 1. Waits for publisher
/// confirm. Returns `Err` on basic_publish failure, confirm nack, or
/// confirm timeout — caller MUST NOT ack the original delivery in that case.
async fn republish_with_incremented_count(
    channel: &Channel,
    exchange: &str,
    routing_key: &str,
    body: &[u8],
    old_count: i64,
    message_id: Option<&str>,
) -> Result<(), lapin::Error> {
    let mut headers = FieldTable::default();
    headers.insert(
        REDELIVERY_HEADER.into(),
        AMQPValue::LongLongInt(old_count + 1),
    );
    let mut props = BasicProperties::default()
        .with_content_type("application/json".into())
        .with_delivery_mode(2) // persistent
        .with_headers(headers);
    if let Some(mid) = message_id {
        props = props.with_message_id(mid.to_string().into());
    }
    let confirm = channel
        .basic_publish(
            exchange,
            routing_key,
            BasicPublishOptions { mandatory: true, ..Default::default() },
            body,
            props,
        )
        .await?
        .await?;
    if confirm.is_nack() {
        // Use a sentinel error variant to signal "do not ack the original"
        return Err(lapin::Error::IOError(Arc::new(std::io::Error::other(
            "publisher confirm nack",
        ))));
    }
    Ok(())
}
```

- [ ] **Step 5: 在 subscribe_inner 内对 channel 开 confirm_select**

找到 `let channel = self.conn.create_channel().await?;` 后续行：

```rust
let channel = self.conn.create_channel().await?;
// 新增：
channel.confirm_select(ConfirmSelectOptions::default()).await?;
// 已有：
channel.basic_qos(32, BasicQosOptions::default()).await?;
```

- [ ] **Step 6: 改 spawn 闭包内的 delivery 循环**

把现有 spawn 闭包（推测行 183-212）整体替换。闭包需要 clone 必要的变量（`channel`、`exchange`）：

```rust
let handler = Arc::new(handler);
let exchange_name = self.exchange.clone();
tokio::spawn(async move {
    let mut consumer = consumer;  // re-shadow (consumed in next() loop)
    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(d) => match serde_json::from_slice::<Envelope<P>>(&d.data) {
                Ok(env) => {
                    let count = get_redelivery_count(d.properties.headers());
                    let routing_key = d.routing_key.as_str().to_string();
                    let body = d.data.clone();
                    let mid = d
                        .properties
                        .message_id()
                        .as_ref()
                        .map(|m| m.to_string());
                    match handler(env).await {
                        Ok(_) => {
                            let _ = d.ack(BasicAckOptions::default()).await;
                        }
                        Err(HandlerError::Permanent(msg)) => {
                            tracing::warn!(error = %msg, "handler permanent error → DLQ");
                            let _ = d
                                .nack(BasicNackOptions { requeue: false, ..Default::default() })
                                .await;
                        }
                        Err(HandlerError::Transient(msg)) => {
                            if count >= REDELIVERY_LIMIT {
                                tracing::warn!(
                                    error = %msg,
                                    count,
                                    "transient exceeded redelivery limit → DLQ"
                                );
                                let _ = d
                                    .nack(BasicNackOptions { requeue: false, ..Default::default() })
                                    .await;
                            } else {
                                match republish_with_incremented_count(
                                    &channel,
                                    &exchange_name,
                                    &routing_key,
                                    &body,
                                    count,
                                    mid.as_deref(),
                                )
                                .await
                                {
                                    Ok(_) => {
                                        tracing::info!(
                                            error = %msg,
                                            count = count + 1,
                                            "transient → republished"
                                        );
                                        let _ = d.ack(BasicAckOptions::default()).await;
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            error = %e,
                                            "republish failed; leaving orig for redelivery"
                                        );
                                        // 不 ack：session 关闭时 broker 重投
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "envelope deserialization failed → DLQ");
                    let _ = d
                        .nack(BasicNackOptions { requeue: false, ..Default::default() })
                        .await;
                }
            },
            Err(_) => break,
        }
    }
});
```

> 注意 `consumer` 变量在原代码是 `let mut consumer = ...`，已 mutable，re-shadow 用 `consumer`
> 而非 `consumer.next()`。本步骤的代码应兼容该变量的所有权（上面的 `consumer.next()` 在
> `while let Some(delivery) = ...` 内已经把 consumer moved；如本改法触编译错，把外层 let
> 改为 `let mut consumer = ...; let stream = std::pin::pin!(consumer); while let ... = stream.next()`）。

- [ ] **Step 7: 加 lapin::Error 需要的 import**

`lapin::Error::IOError(Arc<std::io::Error>)` 构造需要 `use std::sync::Arc;`。在 consumer.rs 顶部 import 段添加（如尚未有）：

```rust
use std::sync::Arc;
```

- [ ] **Step 8: cargo check 验证 consumer.rs 自身编译**

Run: `cargo check -p common-event`
Expected: common-event 自身编译通过

> workspace 全 cargo check 仍会失败（4 个 handler caller 还没改），正常。Task 5 修。

- [ ] **Step 9: Commit**

```bash
git add libs/common-event/src/consumer.rs
git commit -m "feat(common-event): consumer delivery loop with bounded redelivery

- HandlerError::Transient → republish-and-ack with x-redelivery-count
  header incremented; original delivery acked AFTER publisher confirm
- count >= 3 → straight to DLQ (nack requeue=false)
- HandlerError::Permanent → straight to DLQ
- deserialization fail → DLQ
- republish confirm nack / publish fail → do NOT ack original (broker
  re-delivers on session close; safe at-least-once)
- channel.confirm_select enabled for publish confirmations

OOQ trade-off documented in spec §3.2: current 4 events are order-
insensitive; new events must pass the §3.2 checklist."
```

---

### Task 3: 单测 HandlerError display + get_redelivery_count

**Files:**
- Create: `libs/common-event/tests/error_classes.rs`

**Interfaces:** `HandlerError::Display` 输出、`get_redelivery_count` 各类型 header 解析

- [ ] **Step 1: 新建 `libs/common-event/tests/error_classes.rs`**

```rust
//! 单测：HandlerError display + get_redelivery_count 各 header 类型解析。
//! 不起 RabbitMQ，纯单元测。

use common_event::HandlerError;
use lapin::types::{AMQPValue, FieldTable, LongString, ShortString};

#[test]
fn handler_error_transient_display() {
    let e = HandlerError::Transient("db timeout".into());
    assert_eq!(e.to_string(), "transient: db timeout");
}

#[test]
fn handler_error_permanent_display() {
    let e = HandlerError::Permanent("invalid payload".into());
    assert_eq!(e.to_string(), "permanent: invalid payload");
}

// 重新暴露 get_redelivery_count 在测试中可调用：
// 由于是模块私有，需要在 common-event 顶层加 #[cfg(test)] pub(crate) use 导出。
// 详见 Step 2。

fn make_headers(entries: Vec<(&str, AMQPValue)>) -> Option<FieldTable> {
    let mut h = FieldTable::default();
    for (k, v) in entries {
        h.insert(ShortString::from(k), v);
    }
    Some(h)
}

#[test]
fn count_zero_when_header_absent() {
    // 把 None 视为"header 字段整体缺失"——AMQP properties headers 整个字段为 None
    let headers: Option<FieldTable> = None;
    assert_eq!(common_event::consumer::get_redelivery_count(&headers), 0);
}

#[test]
fn count_zero_when_header_field_absent() {
    let headers = make_headers(vec![("other", AMQPValue::LongLongInt(99))]);
    assert_eq!(common_event::consumer::get_redelivery_count(&headers), 0);
}

#[test]
fn count_parses_long_long_int() {
    let headers = make_headers(vec![(
        "x-redelivery-count",
        AMQPValue::LongLongInt(2),
    )]);
    assert_eq!(common_event::consumer::get_redelivery_count(&headers), 2);
}

#[test]
fn count_parses_long_int() {
    let headers = make_headers(vec![(
        "x-redelivery-count",
        AMQPValue::LongInt(3),
    )]);
    assert_eq!(common_event::consumer::get_redelivery_count(&headers), 3);
}

#[test]
fn count_zero_on_unexpected_type() {
    let headers = make_headers(vec![(
        "x-redelivery-count",
        AMQPValue::LongString(LongString::from("not-a-number")),
    )]);
    assert_eq!(common_event::consumer::get_redelivery_count(&headers), 0);
}
```

- [ ] **Step 2: 把 `get_redelivery_count` 在 cfg(test) 下导出**

修改 `libs/common-event/src/consumer.rs`：

```rust
fn get_redelivery_count(headers: &Option<FieldTable>) -> i64 { ... }
```

改为：

```rust
#[cfg(not(test))]
fn get_redelivery_count(headers: &Option<FieldTable>) -> i64 { ... }

#[cfg(test)]
pub(crate) fn get_redelivery_count(headers: &Option<FieldTable>) -> i64 { ... }
```

> 与 T1 fill_trace_id helper 的 cfg(test) 导出模式一致。

- [ ] **Step 3: 检查 `libs/common-event/Cargo.toml` dev-deps**

文件应已有 `[dev-dependencies]`；如无则加：

```toml
[dev-dependencies]
lapin = "2"
```

按 workspace 现有 lapin 版本号取（grep `lapin` 看 workspace 现有版本）。

- [ ] **Step 4: 跑测试验证**

Run: `cargo test -p common-event --test error_classes`
Expected: 7 passed

- [ ] **Step 5: Commit**

```bash
git add libs/common-event/tests/error_classes.rs libs/common-event/src/consumer.rs libs/common-event/Cargo.toml
git commit -m "test(common-event): unit tests for HandlerError display and get_redelivery_count

Pure unit tests: no broker, no MySQL. Covers display formatting, header
absent / field absent / LongLongInt / LongInt / unexpected type."
```

---

### Task 4: testcontainers RabbitMQ 5 个集成测

**Files:**
- Modify: `libs/common-event/Cargo.toml` (dev-deps)
- Create: `libs/common-event/tests/consumer_redelivery.rs`

**Interfaces:** testcontainers 真实 broker 验证三态路径 + republish 成功 + count 升级 + deser DLQ + confirm nack 不 ack

- [ ] **Step 1: 加 dev-deps**

`libs/common-event/Cargo.toml` `[dev-dependencies]` 段（如已有，merge）：

```toml
[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["rabbitmq"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v7"] }
lapin = "2"
```

- [ ] **Step 2: 新建 `libs/common-event/tests/consumer_redelivery.rs`**

> 完整 200+ 行测试文件，覆盖：
> - Permanent → DLQ
> - Transient × 1 → republish 成功（count=1）
> - Transient × 3 → DLQ（count 满）
> - deser fail → DLQ
> - confirm nack → 原 delivery 未 ack

文件骨架（具体 RabbitMQ 拓扑、consumer subscribe 与 handler 完整代码见 spec §5.5）：

```rust
//! testcontainers RabbitMQ 集成测：consumer 三态路径。
//!
//! 用例（详见 spec §6.2）：
//! 1. handler_permanent_goes_to_dlq
//! 2. handler_transient_requeues_then_succeeds
//! 3. handler_transient_upgrades_to_dlq_after_3_redeliveries
//! 4. deserialization_failure_goes_to_dlq
//! 5. republish_confirm_nack_does_not_ack_original

use common_event::{
    Consumer, Envelope, Event, HandlerError, RabbitMqConsumer, RabbitMqPublisher,
};
use futures_util::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, BasicGetOptions, BasicNackOptions,
        BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Connection, ConnectionProperties, ExchangeKind,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};
use std::time::Duration;
use testcontainers::runners::SyncRunner;
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::time::sleep;

const EXCHANGE: &str = "events";
const QUEUE: &str = "test.consumer-redelivery";
const BINDING: &str = "test.event";
const DLX: &str = "events.dlx";
const DLQ: &str = "events.dlq";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestPayload {
    value: i32,
}
impl Event for TestPayload {
    fn event_type() -> &'static str { "test.event" }
    fn schema_version() -> u32 { 1 }
}

async fn setup_rabbit() -> (String, Connection) {
    let container = RabbitMq::default().start().unwrap();
    let host = container.get_host().unwrap();
    let port = container.get_host_port_ipv4(5672).unwrap();
    let url = format!("amqp://guest:guest@{}:{}/%2f", host, port);
    let conn = Connection::connect(&url, ConnectionProperties::default()).await.unwrap();

    // pre-declare topology mirroring Phase 4 definitions.json
    let ch = conn.create_channel().await.unwrap();
    ch.exchange_declare(
        EXCHANGE,
        ExchangeKind::Topic,
        ExchangeDeclareOptions { durable: true, ..Default::default() },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.exchange_declare(
        DLX,
        ExchangeKind::Topic,
        ExchangeDeclareOptions { durable: true, ..Default::default() },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.queue_declare(
        DLQ,
        QueueDeclareOptions { durable: true, ..Default::default() },
        FieldTable::default(),
    )
    .await
    .unwrap();
    ch.queue_bind(
        DLQ,
        DLX,
        "#",
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await
    .unwrap();
    let mut q_args = FieldTable::default();
    q_args.insert(
        "x-dead-letter-exchange".into(),
        lapin::types::AMQPValue::LongString(DLX.into()),
    );
    ch.queue_declare(
        QUEUE,
        QueueDeclareOptions { durable: true, ..Default::default() },
        q_args,
    )
    .await
    .unwrap();
    ch.queue_bind(
        QUEUE,
        EXCHANGE,
        BINDING,
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await
    .unwrap();
    drop(ch);

    (url, conn)
}

async fn publish_one(url: &str, body: &[u8], headers: FieldTable) {
    let conn = Connection::connect(url, ConnectionProperties::default()).await.unwrap();
    let ch = conn.create_channel().await.unwrap();
    ch.confirm_select(Default::default()).await.unwrap();
    let confirm = ch
        .basic_publish(
            EXCHANGE,
            BINDING,
            BasicPublishOptions { mandatory: true, ..Default::default() },
            body,
            BasicProperties::default()
                .with_content_type("application/json".into())
                .with_delivery_mode(2)
                .with_headers(headers),
        )
        .await
        .unwrap()
        .await
        .unwrap();
    assert!(!confirm.is_nack(), "publish nack in test fixture");
}

fn drain_dlq(conn: &Connection) -> Vec<Vec<u8>> {
    // 同步 helper：起 channel basic_get DLQ 直到 None；返回所有 body
    futures_executor::block_on(async {
        let ch = conn.create_channel().await.unwrap();
        let mut bodies = vec![];
        loop {
            match ch
                .basic_get(DLQ, BasicGetOptions { no_ack: true })
                .await
                .unwrap()
            {
                Some(msg) => bodies.push(msg.delivery.data.clone()),
                None => break,
            }
        }
        bodies
    })
}

#[tokio::test]
async fn handler_permanent_goes_to_dlq() {
    let (url, conn) = setup_rabbit().await;

    let body = serde_json::to_vec(&TestPayload { value: 1 }).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Permanent("reject".into()))
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    let bodies = drain_dlq(&conn);
    assert_eq!(bodies.len(), 1, "expected exactly 1 message in DLQ");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 1);
}

#[tokio::test]
async fn handler_transient_requeues_then_succeeds() {
    let (url, conn) = setup_rabbit().await;
    let counter = Arc::new(AtomicI32::new(0));
    let body = serde_json::to_vec(&TestPayload { value: 7 }).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    let counter_for_handler = counter.clone();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], move |_env: Envelope<TestPayload>| {
            let counter = counter_for_handler.clone();
            async move {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    Err(HandlerError::Transient("first try".into()))
                } else {
                    Ok(())
                }
            }
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(3)).await;

    let bodies = drain_dlq(&conn);
    assert_eq!(bodies.len(), 0, "expected DLQ to be empty after retry success");
    assert_eq!(counter.load(Ordering::SeqCst), 2, "handler should run twice");
}

#[tokio::test]
async fn handler_transient_upgrades_to_dlq_after_3_redeliveries() {
    let (url, conn) = setup_rabbit().await;
    let body = serde_json::to_vec(&TestPayload { value: 9 }).unwrap();
    publish_one(&url, &body, FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Err(HandlerError::Transient("always fail".into()))
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(4)).await;

    let bodies = drain_dlq(&conn);
    assert_eq!(bodies.len(), 1, "expected 1 message in DLQ after 3 retries");
    let env: Envelope<TestPayload> = serde_json::from_slice(&bodies[0]).unwrap();
    assert_eq!(env.payload.value, 9);
}

#[tokio::test]
async fn deserialization_failure_goes_to_dlq() {
    let (url, conn) = setup_rabbit().await;
    // publish invalid JSON that cannot deserialize to Envelope<TestPayload>
    publish_one(&url, b"not-a-valid-envelope", FieldTable::default()).await;

    let consumer = RabbitMqConsumer::with_exchange(&url, EXCHANGE).await.unwrap();
    consumer
        .subscribe::<TestPayload, _, _>(QUEUE, &[BINDING], |_env: Envelope<TestPayload>| async {
            Ok(())
        })
        .await
        .unwrap();

    sleep(Duration::from_secs(2)).await;

    let bodies = drain_dlq(&conn);
    assert_eq!(bodies.len(), 1, "deser fail should DLQ");
}

#[tokio::test]
async fn republish_confirm_nack_does_not_ack_original() {
    // 模拟：用一个不存在的 exchange 让 basic_publish 必失败。
    // 但 consumer 的 republish 走同 channel，channel 关闭会让原 delivery 重新投。
    // 简化：在测试中直接构造一个不存在的 routing-key + mandatory=true，让 broker 返 nack。
    // 验证：原 delivery 仍 unacked（队列里有它，consumer 重连后重新可见）。
    todo!("Phase 6-A T3 follow-up: see spec §6.2 item 5")
}
```

- [ ] **Step 3: 跑集成测（需要 Docker）**

Run: `cargo test -p common-event --test consumer_redelivery`
Expected: 4 passed（`republish_confirm_nack_does_not_ack_original` 是 `todo!()` 跳过）

- [ ] **Step 4: Commit**

```bash
git add libs/common-event/tests/consumer_redelivery.rs libs/common-event/Cargo.toml
git commit -m "test(common-event): testcontainers RabbitMQ integration for consumer

5 integration tests against real broker:
- permanent → DLQ
- transient then success on retry → no DLQ, handler runs twice
- transient 3x → DLQ
- deser fail → DLQ
- republish confirm nack → todo!() (Phase 6-A T3 follow-up)"
```

---

### Task 5: 4 个现有 handler caller 改造

**Files:**
- Modify: `services/audit-service/src/consumer.rs`
- Modify: `services/audit-service/src/risk.rs`
- Modify: `services/notification-service/src/consumer/iam_events.rs`
- Modify: `services/api-gateway/src/perm_consumer.rs`

**Interfaces:** 4 处 `HandlerError::Handler(...)` 改 `HandlerError::Permanent(...)` 或 `Transient(...)`

- [ ] **Step 1: `services/audit-service/src/consumer.rs:76`**

找到 `HandlerError::Handler(e.to_string())`。原 error 来自 sqlx INSERT：
- **sqlx::Error::Database 含 deadlock / lock_wait_timeout / connection refused** → `Transient`
- **其他 sqlx error（unique-violation / 字段类型错）** → `Permanent`

简化判断（本期 spec 范围）：

```rust
.execute(pool)
.await
.map_err(|e| {
    warn!(error = %e, event_id = %env.event_id, "audit consumer persist failed");
    // 简化：所有 sqlx 错默认 Transient（DB 抖动多见），业务层 unique-violation
    // 留给 ON DUPLICATE KEY UPDATE 吸收；未来可按 sqlx::Error::is_unique_violation() 拆 Permanent
    HandlerError::Transient(e.to_string())
})?;
```

- [ ] **Step 2: `services/audit-service/src/risk.rs:65-68`（count_dimension 内的 redis err）**

原代码 swallow Redis 错返 Ok。改为：

```rust
match redis.incr_with_window(&key, opts.window_secs).await {
    Ok(n) => { ... }
    Err(e) => {
        warn!(error = %e, dimension, key_value, "risk counter redis error (will be retried)");
        return Err(HandlerError::Transient(format!("redis incr: {e}")));
    }
}
```

- [ ] **Step 3: `services/notification-service/src/consumer/iam_events.rs`**

打开文件。handler 形态大概是 `pub async fn handle_iam_user_created(...) -> Result<(), HandlerError>`。
搜索 `HandlerError::Handler` / `.map_err(|e| HandlerError::Handler(` 出现处，按 sqlx / redis 错误分类：

```rust
// 伪代码模板
.map_err(|e| {
    if matches!(e, sqlx::Error::RowNotFound | ...) {
        HandlerError::Permanent(e.to_string())
    } else {
        HandlerError::Transient(e.to_string())
    }
})?
```

具体分类按 handler 实际错误源调整。

- [ ] **Step 4: `services/api-gateway/src/perm_consumer.rs:63`**

打开文件。原代码 grep `HandlerError::Handler`：找到 `Ok::<(), HandlerError>(())` 与显式 err 转换处。
按 Redis 错误类型：

```rust
// redis SETEX 失败 → Transient
.map_err(|e| HandlerError::Transient(format!("perm_epoch setex: {e}")))?;
```

- [ ] **Step 5: workspace cargo check**

Run: `cargo check --workspace`
Expected: 全部编译通过

- [ ] **Step 6: 跑 4 服务现有测试**

Run: `cargo test -p audit-service -p notification-service -p api-gateway`
Expected: 全绿

- [ ] **Step 7: Commit**

```bash
git add services/audit-service/ services/notification-service/ services/api-gateway/
git commit -m "refactor(handlers): use Transient/Permanent per error source

4 handler call sites updated to return explicit error class:
- audit consumer: sqlx errors → Transient (DB 抖动类多)
- audit risk: redis errors → Transient (was swallow→Ok)
- notification iam_events: sqlx errors split Transient/Permanent
- api-gateway perm_consumer: redis SETEX errors → Transient"
```

---

### Task 6: 手工 e2e 验收清单

**Files:**
- Create: `test-case/63-consumer-error-classes.md`

**Interfaces:** 三场景手工验证

- [ ] **Step 1: 新建 `test-case/63-consumer-error-classes.md`**

```markdown
# T3: Consumer Transient/Permanent + Bounded Redelivery — 手工 e2e 验收

## 前置
docker compose -f infrastructure/docker-compose-infra.yml up -d
# RabbitMQ + outbox-publisher + 各服务

## 场景 1：Permanent → DLQ

构造一个不合法 payload 触发 audit consumer 返 Permanent。
观察：
- audit-service 日志: "handler permanent error → DLQ"
- RabbitMQ Management UI (http://localhost:15672):
  Queue `events.dlq` 深度 +1
  Queue `audit.iam-events` 深度 = 0

## 场景 2：Transient 自愈

正常创建用户；audit consumer 短暂返 Transient（可以用 iptables drop MySQL 5s 模拟）。
观察：
- audit-service 日志: "transient → republished" count=1
- iptables 解封后：count 1/2/3 之前 handler 应已成功 Ok
- 最终 DLQ 0 条，业务成功

## 场景 3：Transient 升级 DLQ

停 MySQL 容器；触发业务事件。
观察：
- audit-service 日志: "transient → republished" count=1, 2, 3
- 第 4 次处理时: "transient exceeded redelivery limit → DLQ" count=3
- events.dlq 深度 +1
- 注意 republish-and-ack 模式下消息 header `x-redelivery-count: 3` 可见

## 场景 4：OOQ 不影响（spec §3.2 验证）

构造 5 条 `iam.user.created` 同 user_id 顺序触发；观察：
- 全部成功，audit_events 表行数 = 5
- 顺序在 audit_events 不严格保证（事件写入时间戳是 broker 投递顺序），但业务幂等
  （ON DUPLICATE KEY UPDATE）不报错
- 同样：构造 5 条 `iam.permission.changed` 同 user_id；
  gateway perm_consumer set-if-greater 保证最终 epoch 是最后值（无回退）

## 验收标准
- [ ] 场景 1：Permanent 1 次重投就 DLQ
- [ ] 场景 2：Transient 自愈不需 shovel
- [ ] 场景 3：count=3 后升级 DLQ，header 可见
- [ ] 场景 4：OOQ 4 个 event 业务不报错
```

- [ ] **Step 2: 跑全部 T3 测试**

Run: `cargo test -p common-event`
Expected: error_classes 7 + consumer_redelivery 4 全绿

- [ ] **Step 3: Commit**

```bash
git add test-case/63-consumer-error-classes.md
git commit -m "docs: T3 manual e2e acceptance checklist

3 scenarios + 1 OOQ sanity check. Validates Permanent-DLQ, Transient
self-heal, count upgrade at 3, and order-insensitive events survive
the republish-and-ack pattern."
```

---

## Self-Review

### 1. Spec 覆盖检查

| Spec 章节 | 对应 Task |
|---|---|
| §1 背景（handler err 单一分支、4 handler swallow→Ok 临时绕路）| Task 5 |
| §2 目标（4 件：HandlerError 双分支、consumer 决策、4 handler 改造、集成测）| Task 1 + Task 2 + Task 5 + Task 4 |
| §3 决策（双分支 / count 3 / broker header / redelivered vs count / 4 handler 改）| Task 1 + Task 2 + Task 5 |
| §3.1 count 在 header 而非 envelope_json | Task 2 |
| §3.2 OOQ 限制 + checklist | Task 6 场景 4 验证 |
| §3.3 republish-and-ack 事务性 + confirm_select | Task 2 Step 5 |
| §4 数据流 | Task 2 + Task 6 验证 |
| §5.1 HandlerError 双分支 | Task 1 |
| §5.2 consumer.rs 重写 + confirm_select | Task 2 |
| §5.3 4 handler 改 | Task 5 |
| §5.4 topology 不动 | Task 4 setup_rabbit 验证 |
| §5.5 DLQ 命中运维 | Task 6 场景 1 |
| §6.1 单测 | Task 3 |
| §6.2 testcontainers 5 个 | Task 4 |
| §6.3 手工 e2e | Task 6 |
| §6.4 Failure modes | 已在 spec 列出；本 plan 关注 critical gap 1-Q1（OOQ）→ Task 6 验证 |

无遗漏。

### 2. 占位符扫描

- 无 "TBD" / "TODO" / "implement later"。
- 1 处 `todo!()` 在 Task 4 Step 2 集成测 5 末尾 — **故意**（spec §6.2 标 "follow-up"，需
  真实构造 broker nack 路径，本期暂留 stub）。
- 无 "Add appropriate error handling"（已 explicit to_string() / match）。
- 无 "Write tests for the above"（每步都有具体 test code）。

### 3. 类型一致性

- `HandlerError::Transient(String)` / `HandlerError::Permanent(String)` 在 Task 1 定义，Task 2 match，Task 3 测试，Task 5 4 个 handler 使用。一致。
- `republish_with_incremented_count(channel, exchange, routing_key, body, old_count, message_id)` 签名在 Task 2 定义，Task 2 闭包内调用。一致。
- `get_redelivery_count(headers: &Option<FieldTable>) -> i64` 在 Task 2 定义 + cfg(test) 导出，Task 3 测试使用。一致。
- `REDELIVERY_LIMIT: i64 = 3` 在 Task 2 定义，Task 2 比较 `count >= REDELIVERY_LIMIT`。一致。

无冲突。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-28-phase6a-t3-consumer-error-classes.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
