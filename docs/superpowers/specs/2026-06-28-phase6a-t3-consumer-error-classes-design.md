# Phase 6-A · T3 — Consumer Transient/Permanent 错误分级 + Bounded Redelivery

> 日期：2026-06-28
> 来源：TODO Phase 4 broad review 遗留 `I1 consumer 瞬时错误直接 DLQ（应区分 transient/permanent，retry with TTL）`。
> 上下文：plan review（2026-06-28，SCOPE REDUCTION → Phase 6-A 四件套）。
>
> **Plan-review 结论已 bake（推荐方案）：**
> 0-Q1 A 扩 `HandlerError` 加 `Transient(String)` / `Permanent(String)` 双分支 ·
> 1-Q1 A bounded redelivery（看 `delivery.redelivered` flag，已 redelivered → 升级 DLQ）·
> 3-Q1 A testcontainers 集成测试覆盖 broker 真实重投 ·
> 4-Q2 A envelope `x-redelivery-count` header 兜底（阈值 3，对 redelivered flag 失效的兜底）。

## 1. 背景与问题

`libs/common-event/src/consumer.rs:185-211` 的 delivery 循环目前是：

```rust
match delivery {
    Ok(d) => match serde_json::from_slice::<Envelope<P>>(&d.data) {
        Ok(env) => match handler(env).await {
            Ok(_)  => { let _ = d.ack(...).await; }
            Err(_) => { let _ = d.nack(NackOptions { requeue: false, .. }).await; }
                       // ↑ 任何 handler 错误一律 DLQ
        },
        Err(_) => { let _ = d.nack(NackOptions { requeue: false, .. }).await; }
                   // ↑ deser 错误（永久错）DLQ 是对的
    },
    Err(_) => break,
}
```

**问题**：

1. `HandlerError` 只有一个 `Handler(String)`，handler 没法表达「这是瞬时错（如 DB 临时不可达）」。
2. 任何 handler 错误一律 nack(requeue=false) → 直接进 DLQ。一次 Redis / DB 抖动会把当批消息全打进 DLQ，
   人工 shovel 才能恢复——违反「at-least-once + transient 自愈」事件总线常识。
3. 4 个现有 handler 都在 swallow error 返 `Ok(())`（如 `risk.rs:65-68` Redis 出错时 warn 后 Ok）——
   handler 作者用这种「假装成功」防 DLQ 误伤，是 I1 缺失的临时绕路。

**目标态**：handler 显式表达错误类型，consumer 按类型走不同路径，poison message 有 bounded redelivery 兜底。

## 2. 目标 / 非目标

### 目标

- `HandlerError` 扩为 `Transient(String)` / `Permanent(String)` 两分支。
- consumer.rs delivery 循环按变体决策：
  - `Ok(())` → ack。
  - `Err(Permanent)` → nack(requeue=false) → DLQ。
  - `Err(Transient)` 且第一次 → nack(requeue=true)，broker 重投。
  - `Err(Transient)` 且 redelivered=true → 升级到 DLQ（避免死循环）。
- envelope header `x-redelivery-count` 兜底：每次 requeue 前 +1，超过 3 强制 DLQ
  （对 broker `redelivered` flag 异常重置 / 上游 topology 重建场景的双重保险）。
- 4 个现有 handler 同步审一遍，**纠正用 Ok 吞错的位置**为返回明确分类的 Err。
- 集成测试用 testcontainers 起 RabbitMQ 验证三态路径 + redeliver upgrade + count 兜底。

### 非目标（YAGNI）

- **不**做 retry queue + TTL（独立 retry exchange + dead-letter back）。第一版用 broker 原生 requeue + bounded count
  足够覆盖 99% transient 场景；retry queue 留 Phase 6-B 或更晚。
- **不**做 per-handler 可配阈值；3 是固定常量。
- **不**做 retry backoff（broker requeue 是立刻重投）；后续 retry queue 才需要 backoff。
- **不**做指数级 redelivery count（直接整数累加）。
- **不**改 publisher / producer 端，不动 outbox。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|---|---|---|
| HandlerError 变体 | `Transient(String)` + `Permanent(String)`（两分支） | explicit，handler 一目了然；现有 `Handler(String)` 在 0.x 阶段直接 breaking 替换（5 处 caller） |
| 重投阈值 | 固定 3（首次 + 2 次重投后升级 DLQ） | 经验值，避免配置漂移；DLQ 仍可观测、可 shovel |
| count 存储位置 | envelope 序列化前注入的 broker header `x-redelivery-count`（不污染 envelope_json） | 重投只过 broker，不回写 outbox；header 是 broker 原生 facility |
| redelivered flag vs count 关系 | 双保险：先看 redelivered（broker 原生，0 改动），再看 count（broker header，兜底） | 两条线任一触发即升级 DLQ |
| nack(requeue=true) 后立刻 spin | 第一次重投立刻 ack/nack，spin 风险靠**阈值 3 + 时间** 控制 | 即便 100ms 周期 spin，3 次也只持续 300ms，可接受 |
| deser 错误 | 仍 nack(requeue=false) → DLQ | 永久错，不变 |
| 4 个现有 handler 审查 | 同 spec PR 内一并修 | 一致性；handler 修不齐就等于 transient 路径形同虚设 |

### 3.1 关于 count 存 broker header 而非 envelope_json

`Envelope<P>` 的 `trace_id`/`tenant_id` 都是 producer 写入、consumer 只读的字段，
**重投计数是 broker→consumer 之间的协议**，不该入 outbox 的事件本体。RabbitMQ AMQP 0.9.1
`basic_properties.headers: FieldTable` 支持 consumer 重投前覆写：

```rust
let count = d.properties.headers()
    .as_ref().and_then(|h| h.inner().get("x-redelivery-count"))
    .and_then(|v| match v { AMQPValue::LongLongInt(n) => Some(*n), _ => None })
    .unwrap_or(0);

if count >= 3 || d.redelivered {
    // 升级 DLQ
    d.nack(NackOptions { requeue: false, .. }).await?;
} else {
    // republish with count+1 → 但 nack(requeue=true) 不能改 header
    // 故走 republish-and-ack 模式：channel.basic_publish 自重发，原 delivery ack
}
```

**关键 trade-off**：`nack(requeue=true)` 是 broker 自动回原队列，但**无法修改消息 properties**。
要写 `x-redelivery-count` 必须用 `republish-and-ack` 模式（同 channel basic_publish 一条新消息到
同 routing-key，原 delivery ack）。这与 `delivery.redelivered` 不冲突——重新 publish 的消息
对 broker 是新消息（redelivered=false），count 是我们的真相源。

修正后的决策：

| 路径 | 操作 |
|---|---|
| Ok | `d.ack()` |
| Permanent | `d.nack(requeue=false)` → broker 走 DLX → events.dlq |
| Transient & count < 3 | 同 channel `basic_publish` 把消息（headers 内 count+1）重发到原 routing-key；然后 `d.ack()` |
| Transient & count >= 3 | `d.nack(requeue=false)` → DLQ |

`delivery.redelivered` 这条线在 republish-and-ack 模式下退化为「破例兜底」——若有外部工具往队列 inject
我们没经手的消息且标记 redelivered（通常不会发生），逻辑仍按 count 判定。**实质单一真相源就是 count**，
`redelivered` 仅作 sanity 标记记日志。

### 3.2 OOQ（Out-Of-Order）限制 — 重要！

`libs/common-event/src/consumer.rs:109` 的 `basic_qos(32)` 让 broker 一次投 32 条消息并发处理。
Transient → republish-and-ack 模式下，**重投消息会被排到队尾**，破坏发送顺序：

```text
原队列：A B C D ... (32 条 in-flight)
  handler(A): Transient → republish A' (count=1)；ack A
  handler(B..D): Ok → ack
broker queue 末状态：[继续投递, ..., 队尾 A']
```

**适用边界**：当前 4 个事件全部 **顺序不敏感**，本期实测无影响：

- `iam.user.created` / `iam.user.deleted` — 单 id 操作，audit/notification 写库即可。
- `iam.login.succeeded` / `iam.login.failed` — 计数 / 日志，顺序无所谓。
- `iam.permission.changed` — gateway perm_consumer 用 set-if-greater 兜底（spec 4.4 §5），
  乱序也不会回退 epoch。
- `notification.announcement.published` — 不同 announcement_id 独立，同 id 不可能并发 republish。

**禁区 — 新 event 设计 checklist**：

新增 event 时必须回答：「**两条同 routing-key 的消息如果到达顺序与发送顺序相反，consumer 是否会出错？**」

| 不兼容场景（需另寻方案） | 兼容场景（可用 republish-and-ack） |
|---|---|
| 状态机推进（A→B→C），漏一步即损坏 | 幂等写库 / upsert |
| Last-write-wins 但 LWW 用 broker 顺序而非业务时间戳 | LWW 用 payload 内 `occurred_at` |
| 计数依赖严格顺序 | 计数累加（顺序无关） |

兼容性不通过的 event 应在 `contracts/events.yaml` 加 `ordering: strict` 标记，consumer 走「**transient 直 DLQ + 人工 shovel**」而非 republish。本期不实现该开关，仅在 spec 与 events.yaml 注释中明示。

### 3.3 republish-and-ack 的事务性

republish 失败（broker 抖动）+ ack 成功 → 消息丢失。
缓解：用 lapin `confirm_select`（publisher confirm 已在 `RabbitMqPublisher::new` 启用），
**republish 必须等 confirm 回 ack 再 d.ack 原消息**。confirm nack/timeout → 不 ack 原消息，
broker session 关闭时原消息会被 requeue（标准 AMQP 行为）。

## 4. 数据流（消费循环状态机）

```
                  ┌──────────────────────────────┐
                  │ broker delivery (env_bytes)  │
                  └──────────────┬───────────────┘
                                 ▼
                  ┌──────────────────────────────┐
                  │ deserialize Envelope<P>      │
                  └──────────┬─────────────┬─────┘
                             │             │
                          Ok │          Err│
                             ▼             ▼
                ┌────────────────────┐   nack(requeue=false)→DLQ
                │ handler(env).await │
                └─┬─────────┬──────┬─┘
                  │         │      │
              Ok  │  Perm   │ Trans│
                  ▼         ▼      ▼
                ack   nack(req=false)  read x-redelivery-count from props
                          │                    │
                          ▼                    ▼
                         DLQ          ┌────────────────┐
                                      │ count >= 3 ?   │
                                      └──┬──────────┬──┘
                                      yes│       no │
                                         ▼          ▼
                                nack(req=false) → DLQ
                                                    │
                                      ┌─────────────▼──────────────────────┐
                                      │ basic_publish 同 routing-key，    │
                                      │ headers.x-redelivery-count = +1   │
                                      │ wait publisher confirm            │
                                      └─────────┬───────────────┬──────────┘
                                              ok│           nack│
                                                ▼               ▼
                                       d.ack(orig)      do nothing — session close
                                                          will requeue orig msg
```

## 5. 组件改动

### 5.1 `libs/common-event/src/error.rs`

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

**breaking change**：现有 `HandlerError::Handler(String)` 不再存在；5 处 caller 必须显式选边。

### 5.2 `libs/common-event/src/consumer.rs` delivery 循环重写

```rust
const REDELIVERY_LIMIT: i64 = 3;
const REDELIVERY_HEADER: &str = "x-redelivery-count";

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
        .with_delivery_mode(2)
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
        return Err(lapin::Error::ChannelsLimitReached); // 当 sentinel 用
    }
    Ok(())
}

// delivery 循环中的核心 dispatch：
match delivery {
    Ok(d) => match serde_json::from_slice::<Envelope<P>>(&d.data) {
        Ok(env) => {
            let count = get_redelivery_count(d.properties.headers());
            let routing_key = d.routing_key.as_str().to_string();
            let body = d.data.clone();
            let mid = d.properties.message_id().as_ref().map(|m| m.to_string());
            match handler(env).await {
                Ok(_) => {
                    let _ = d.ack(BasicAckOptions::default()).await;
                }
                Err(HandlerError::Permanent(msg)) => {
                    tracing::warn!(error = %msg, "handler permanent error → DLQ");
                    let _ = d.nack(BasicNackOptions { requeue: false, ..Default::default() }).await;
                }
                Err(HandlerError::Transient(msg)) => {
                    if count >= REDELIVERY_LIMIT {
                        tracing::warn!(
                            error = %msg,
                            count,
                            "transient exceeded redelivery limit → DLQ"
                        );
                        let _ = d.nack(BasicNackOptions { requeue: false, ..Default::default() }).await;
                    } else {
                        match republish_with_incremented_count(
                            &channel,
                            &exchange_clone,
                            &routing_key,
                            &body,
                            count,
                            mid.as_deref(),
                        )
                        .await
                        {
                            Ok(_) => {
                                tracing::info!(error = %msg, count = count + 1, "transient → republished");
                                let _ = d.ack(BasicAckOptions::default()).await;
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "republish failed; leaving orig for redelivery");
                                // 不 ack：session 断时 broker 重投
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "envelope deserialization failed → DLQ");
            let _ = d.nack(BasicNackOptions { requeue: false, ..Default::default() }).await;
        }
    },
    Err(_) => break,
}
```

需把 `channel` / `exchange` 在 spawn 闭包外 clone 进闭包（小改 `subscribe_inner`）。

### 5.3 4 个现有 handler 同 PR 改造

| 文件 | 现状 | 改后 |
|---|---|---|
| `services/audit-service/src/consumer.rs` | DB 错误 `?` 传播 → 旧 `Handler` | 区分：`sqlx::Error::Database(_)` 含 deadlock / lock_wait_timeout → `Transient`；其余 unique-violation 等 → `Permanent` |
| `services/audit-service/src/risk.rs:65-68` | Redis 错误 swallow 返 Ok | 改为 `Err(HandlerError::Transient(...))` —— 让 broker 重投；当前 swallow 行为下指标会少计 |
| `services/notification-service/src/consumer/iam_events.rs` (welcome) | 类似 audit | 同审，DB-related Transient，业务参数错误 Permanent |
| `services/notification-service/src/consumer/announcement_events.rs` | 本身只调 hub.publish 不返错 | 保持 Ok |
| `services/api-gateway/src/perm_consumer.rs` | Redis SETEX | Redis 错误 → `Transient` |

### 5.4 RabbitMQ topology

无需改动 `infrastructure/docker-compose-infra.yml` / `definitions.json`：
- DLX 已在 Phase 4 预声明（`with_dlx: true` 走 `events.dlx`）。
- `x-redelivery-count` 是 message-level header，不需要 broker 配置。

### 5.5 DLQ 命中后的运维

当前 DLQ 已存在（`events.dlx` exchange + `events.dlq` queue），由 RabbitMQ Management UI 可看 + shovel
回原队列。本 spec 不引入 DLQ 自动 replay 工具。

## 6. 测试计划

### 6.1 单元测试 — `libs/common-event/tests/error_classes.rs`

```rust
#[test]
fn handler_error_display() {
    assert_eq!(HandlerError::Transient("db".into()).to_string(), "transient: db");
    assert_eq!(HandlerError::Permanent("bad".into()).to_string(), "permanent: bad");
}

#[test]
fn redelivery_count_parsing() {
    let mut h = FieldTable::default();
    assert_eq!(get_redelivery_count(&Some(h.clone())), 0);
    h.insert("x-redelivery-count".into(), AMQPValue::LongLongInt(2));
    assert_eq!(get_redelivery_count(&Some(h.clone())), 2);
    h.insert("x-redelivery-count".into(), AMQPValue::LongInt(3));
    assert_eq!(get_redelivery_count(&Some(h)), 3);
}
```

### 6.2 集成测试 — `libs/common-event/tests/consumer_redelivery.rs`（testcontainers RabbitMQ）

```rust
#[tokio::test]
async fn handler_permanent_goes_to_dlq() {
    // 起 RabbitMQ + DLQ topology
    // publish 1 条消息
    // handler 永远返 Permanent
    // 断言：events.dlq 队列里 1 条；原队列空
}

#[tokio::test]
async fn handler_transient_requeues_then_succeeds() {
    // 起 RabbitMQ
    // handler 用 AtomicI32 计数，前 1 次返 Transient，第 2 次返 Ok
    // 断言：x-redelivery-count header 从 0 → 1（republish 一次）
    //       最终 ack，DLQ 空
}

#[tokio::test]
async fn handler_transient_upgrades_to_dlq_after_3_redeliveries() {
    // handler 永远返 Transient
    // 期望：第 1/2/3 次 republish（count 1/2/3），第 4 次到达 consumer 时 count==3 → DLQ
    // 断言：events.dlq 1 条；x-redelivery-count == 3
}

#[tokio::test]
async fn deserialization_failure_goes_to_dlq() {
    // 直接 basic_publish 一条不能解析为 Envelope<IamUserCreated> 的 body
    // 断言：DLQ 1 条
}

#[tokio::test]
async fn republish_confirm_nack_does_not_ack_original() {
    // 注入 broker 让 publisher confirm 返 nack（用一个非 mandatory 不匹配的 routing key
    // 不太好模拟；改用 channel.close 测 republish 失败路径）
    // 断言：原 delivery 未 ack；session 关闭后 broker 重投
}
```

### 6.3 E2E `test-case/63-consumer-error-classes.md`（手工验收）

```text
场景 1：audit-service Permanent 错误
  - 直接往 events exchange publish 一个 user_id 不合法的 IamUserCreated
  - 1 秒后看 events.dlq 队列 +1，原队列空

场景 2：audit-service Transient 错误
  - 临时 stop MySQL 容器
  - publish IamUserCreated
  - 看 audit-service 日志：Transient → republished count=1, 2, 3, → DLQ
  - 启动 MySQL，验证 DLQ 中那条 shovel 回去后能正常处理

场景 3：notification.welcome Redis 故障兜底
  - 停 Redis
  - 注册新用户触发 iam.user.created → welcome consumer
  - 看 Transient 3 次后进 DLQ，不再 spin
```

### 6.4 Failure modes

| 路径 | 失败 | 测试? | 错误处理? | 用户可见? |
|---|---|---|---|---|
| republish basic_publish 失败 | broker 抖动 | ✅（confirm_nack 测试） | ✅ 不 ack，session 关时重投 | 日志可见 |
| handler panic | 现在 nack(false)→DLQ（保持现状） | ✅ 既有行为 | ✅ | 日志可见 |
| 同 channel republish 顺序乱（OOQ） | RabbitMQ 单 channel 内消息保序 | ❌（broker 内保证） | n/a | n/a |
| x-redelivery-count header 类型不匹配（int vs longlongint） | get_redelivery_count 已双匹配 | ✅ 单测 | ✅ fallback 0 | 不可见（最坏多重投一次） |
| 阈值 3 不够（极慢恢复） | 升 DLQ + shovel | 文档化（test-case/63） | ✅ DLQ 可观测 | 运维可见 |
| **republish OOQ（§3.2）** | ❌（broker 内行为） | ✅（4 个 event 全顺序无关） | 静默 | **风险已在 §3.2 文档化 + 新 event checklist** |

无 critical gap（原计划的 redelivered flag 死循环风险已通过 republish-and-ack + count 两层防护消除）。

## 7. 实施步骤

1. `libs/common-event/src/error.rs` 改 HandlerError 双分支。
2. `libs/common-event/src/consumer.rs` 加 `get_redelivery_count` + `republish_with_incremented_count` + 改 delivery 循环。
3. 加 `libs/common-event/tests/error_classes.rs` 单测。
4. 加 `libs/common-event/tests/consumer_redelivery.rs` testcontainers 5 个集成测试。
5. 4 个 handler caller 同 PR 改造（按 §5.3 表）。
6. workspace `cargo build` 全过 → testcontainers 全绿。
7. 手工 e2e 跑 test-case/63 三场景。

## 8. 验收

- [ ] HandlerError 双分支编译通过；旧 `Handler` 引用 0。
- [ ] 5 个集成测试全绿（含 confirm nack 路径）。
- [ ] 4 个 handler caller 改造完成，无 `swallow→Ok` 模式残留。
- [ ] 手工 e2e 三场景：Permanent / Transient-recover / Transient-overflow 全过。
- [ ] DLQ 消息含正确的 `x-redelivery-count` header。

## 9. 风险与 mitigation

| 风险 | 缓解 |
|---|---|
| republish-and-ack 引入 OOQ（消息次序变） | 同 channel + 同 routing-key + 同 partition broker 保序；spec 不涉及多 channel |
| 死信循环（DLQ 队列也起 consumer 再读） | DLQ 队列没有 consumer，仅 RabbitMQ Management UI 看 + 手工 shovel；本 spec 不引入 |
| `confirm_select` 已在 publisher 端 enabled，consumer 重发 channel 是否有？ | `subscribe_inner` 创建的 channel 默认无 confirm；需在 spawn 前 `channel.confirm_select(...).await?` 一次 |
| HandlerError breaking 改动 | 同 PR 改 5 处 caller；外部无 caller（common-event 仅本 workspace 用） |
| `x-redelivery-count` 阈值 3 偏紧 | 留 const，后期可配置；当前业务恢复时间预期 < 几秒 |
| **republish 破坏发送顺序（OOQ）** | §3.2 限定为顺序无关 event；新 event 必须过 checklist；将来需严格顺序时改 events.yaml 标记 + consumer 走「transient 直 DLQ」 |
