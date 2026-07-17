# Phase 6-A Follow-up Design

> **Date:** 2026-06-29  
> **Scope:** T3-fu1 / T3-fu2 / T2-fu1 / T2-fu3 / T3-fu3 / T2-fu2 / T3-fu4 / T4-fu1  
> **Status:** Implemented  

## 1. Background

Phase 6-A (2026-06-29) 完成了四个主任务：

- T1: trace_id 自动注入
- T2: OTel 全覆盖 + tracing_mw 提取
- T3: Consumer 错误分级 + bounded redelivery
- T4: Outbox 24h 归档

Plan-review 阶段发现 10 个 follow-up，按优先级分为 P0~P3。本 design 文档覆盖其中 8 个已实现项（P2 指标项推迟）。

## 2. Scope

| ID | Priority | Item | File(s) | LOC |
|----|----------|------|---------|-----|
| T3-fu1 | P0 | Basic.Return 静默丢消息修复 | `libs/common-event/src/consumer.rs` | ~15 |
| T3-fu2 | P0 | republish 失败路径集成测 | `libs/common-event/tests/consumer_redelivery.rs` | ~60 |
| T2-fu1 | P1 | span_from_envelope_trace_id 统一 | `libs/common-event/src/span.rs` + 3 consumer | ~30 |
| T2-fu3 | P1 | outbox-publisher 独立 crate | `services/iam-outbox-publisher/` | ~170 |
| T3-fu3 | P3 | sleep→poll 消除 CI flake | `consumer_redelivery.rs` | ~20 |
| T2-fu2 | P3 | 删 docs 中 tracing_mw 旧说明 | `docs-site/USER_MANUAL.md` 等 | ~10 |
| T3-fu4 | P3 | perm_consumer 行为变更发布说明 | `services/api-gateway/AGENTS.md` | ~3 |
| T4-fu1 | P3 | USER_MANUAL archive 说明 | `docs-site/USER_MANUAL.md` | ~1 |

## 3. Design

### 3.1 T3-fu1 — Basic.Return 处理

**Problem:** `republish_with_incremented_count` 使用 `mandatory: true` 但未处理 `Basic.Return`。当 exchange 被误删或 routing-key 不匹配时，broker 返回 publisher confirm (ack) 但消息实际 unroutable → 原 delivery 被 ack → **消息静默丢失**。

**Root Cause:** `basic_publish(...).await?.await?` 只等待 confirm，不等待 Basic.Return。

**Solution:** 使用 `channel.wait_for_confirms()` 替代双层 await：

```
basic_publish(mandatory=true)
  → wait_for_confirms()    ← 同时等待 confirm + 收集 Basic.Return
    ├── confirm nack? → Err
    ├── confirm ack + returns empty? → Ok
    └── confirm ack + returns non-empty? → Err (unroutable)
```

**Race handling:** Basic.Return 可能在 confirm ack 之后、wait_for_confirms  drain 之前到达。增加一次 100ms 后的二次 poll：

```rust
let mut returns = channel.wait_for_confirms().await?;
if returns.is_empty() {
    sleep(100ms).await;
    returns.extend(channel.wait_for_confirms().await?);
}
```

**Caller contract:** `republish_with_incremented_count` 返回 `Err` 时，caller (consumer delivery loop) **不得** ack 原 delivery。当前代码已满足（Err 分支只有 log，无 ack）。

### 3.2 T3-fu2 — 集成测

**Test 1: `republish_confirm_nack_does_not_ack_original`**

验证当 republish 失败时，消息未被静默丢弃。

```
1. 声明队列 QUEUE，绑定 routing-key TMP_BIND
2. publish Envelope → 消息进入 QUEUE
3. 启动 consumer，handler 返回 Transient
4. consumer 收到消息后，unbind TMP_BIND
5. handler 返回 Transient → republish 使用 TMP_BIND → Basic.Return → Err
6. republish 失败，原 delivery 未 ack
7. drop consumer → 连接关闭 → 消息 requeued
8. 断言：消息在队列中（msg_count ≥ 1）或在 DLQ 中
```

**Test 2: `republish_fails_on_unroutable_routing_key`**

直接调用 `republish_with_incremented_count` 到未绑定队列的 routing-key，断言返回 Err。

**Test infrastructure:**
- 所有测试共享一个 RabbitMQ 容器（`tokio::sync::OnceCell`）
- 串行执行（`std::sync::Mutex`）避免队列冲突
- 每测开始前 `queue_delete` + `queue_purge` 清理
- `std::mem::forget(container)` 保持容器生命周期

### 3.3 T2-fu1 — span parent 统一

**Problem:** `span_from_envelope_trace_id` 定义在 `audit-service/src/consumer_span.rs`，notification / gateway 消费者各自需要复制。

**Solution:** 移动到 `libs/common-event/src/span.rs`，从 `lib.rs` re-export。

**Dependencies:** `common-event` 新增直接依赖 `opentelemetry = "0.24"` 和 `tracing-opentelemetry = "0.25"`（之前通过 `common-telemetry` 间接依赖，但 trait items 需要直接 access）。

**Call sites:**

| Service | File | Before | After |
|---------|------|--------|-------|
| audit | `consumer.rs:16` | `crate::consumer_span::...` | `common_event::span_from_envelope_trace_id` |
| audit | `risk.rs:16` | `crate::consumer_span::...` | `common_event::span_from_envelope_trace_id` |
| notification | `consumer/mod.rs` | (none) | `.instrument(span_from_envelope_trace_id(&env, "..."))` |
| gateway | `perm_consumer.rs` | (none) | `.instrument(span_from_envelope_trace_id(&env, "..."))` |

### 3.4 T2-fu3 — outbox-publisher 独立 crate

**Problem:** `outbox-publisher` 作为 `iam-service` 的 `[[bin]]`，共享 `Cargo.toml` 依赖，被迫编译 axum / tower / utoipa / common-telemetry-axum —— 一个纯轮询循环不需要这些。

**Solution:** 新建 `services/iam-outbox-publisher/` 独立 crate。

**Dependencies (minimal):**
```toml
[dependencies]
anyhow = "1"
common-event = { path = "../../libs/common-event" }
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp"] }
serde = "1"
serde_json = "1"
serde_yaml = "0.9"
sqlx = { features = ["runtime-tokio", "mysql"] }
tokio = { features = ["full"] }
tracing = "0.1"
```

**Config loading:** 不依赖 `iam-service` 库。`DATABASE_URL` 优先；fallback 用内联 YAML 解析（`load_db_url_from_yaml`），只读取 `database.url` 字段。

**Binary name:** `iam-outbox-publisher`（与 crate 名一致）。

**Dockerfile:**
```dockerfile
# Before:
cargo build --bin iam-service --bin outbox-publisher
# After:
cargo build --bin iam-service -p iam-outbox-publisher
```

### 3.5 T3-fu3 — sleep→poll

**Problem:** 固定 `sleep(2s/3s/4s)` 在慢 CI 环境可能不够，在快环境浪费。

**Solution:**
- DLQ 等待：新增 `poll_dlq(conn, expected, deadline)`，每 200ms 轮询直到满足或超时
- 计数器等待：直接轮询 `AtomicI32` 值

### 3.6 T2-fu2 — docs 清理

更新三份文档，将 "copy tracing_mw.rs" 改为 "use common-telemetry-axum crate"。

## 4. Data Flow

```
┌─────────────────────┐
│ Producer (iam-svc)  │
│ enqueue → outbox    │
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│ iam-outbox-publisher│  ← T2-fu3: 独立 crate，无 axum
│ basic_publish       │
│ wait_for_confirms   │  ← T3-fu1: Basic.Return 检测
└──────────┬──────────┘
           │
┌──────────▼──────────┐
│   RabbitMQ Exchange │
└──┬──────┬──────┬────┘
   │      │      │
┌──▼──┐ ┌─▼─────┐┐ ┌──────────┐
│audit│ │notify ││ │ gateway  │
│ ✓   │ │ T2-fu1││ │ T2-fu1   │
│span │ │ +span ││ │ +span    │
└─────┘ └───────┘┘ └──────────┘
```

## 5. Test Coverage

| Test | What | Type |
|------|------|------|
| `republish_fails_on_unroutable_routing_key` | 直接调用 `republish_with_incremented_count`，验证 Basic.Return → Err | Unit (testcontainers) |
| `republish_confirm_nack_does_not_ack_original` | 完整 consumer 流：unbind → Transient → republish 失败 → drop consumer → 消息 requeued | Integration (testcontainers) |
| `handler_permanent_goes_to_dlq` | Permanent 错误直接进 DLQ | Integration |
| `handler_transient_requeues_then_succeeds` | Transient 重投后成功 | Integration |
| `handler_transient_upgrades_to_dlq_after_3_redeliveries` | 3 次 Transient 后升 DLQ | Integration |
| `deserialization_failure_goes_to_dlq` | 反序列化失败进 DLQ | Integration |

## 6. Failure Modes

| Scenario | Impact | Mitigation |
|----------|--------|------------|
| T3-fu1 race: Basic.Return 在 100ms 后仍未到达 | 消息仍可能静默丢失 | 100ms 是经验值；若仍复现，可延长至 500ms 或改用 channel.close 检测 |
| T2-fu3: YAML config 解析失败 | outbox-publisher 启动失败 | 生产环境使用 `DATABASE_URL` env，不依赖 YAML |
| T3-fu3: poll timeout | 测试失败而非产品 bug | 超时仅影响测试，产品逻辑不变 |

## 7. Metrics (Deferred)

`iam_outbox_archived_total` + `iam_outbox_purged_total` 推迟原因：
- outbox-publisher 现为独立 CLI binary，无 HTTP `/metrics` 端点
- Prometheus counter 注册了也无法被抓取
- **Future:** 若需要，可在 binary 内嵌极简 TCP listener 暴露 `/metrics`，或将指标写入共享 DB 由 iam-service 的 `/metrics` 聚合

## 8. References

- Phase 6-A T3 spec: `docs/superpowers/specs/2026-06-28-phase6a-t3-consumer-error-classes-design.md`
- Phase 6-A T2 spec: `docs/superpowers/specs/2026-06-28-phase6a-t2-otel-coverage-design.md`
- Phase 6-A T4 spec: `docs/superpowers/specs/2026-06-28-phase6a-t4-outbox-archive-design.md`
