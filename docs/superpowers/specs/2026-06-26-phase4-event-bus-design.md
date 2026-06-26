# Phase 4 — 异步事件总线（最小闭环）设计

> Status: Draft → Review
> Date: 2026-06-26
> Scope: Phase 4 of architect review roadmap (see `TODO.md`)
> Predecessor: 无（首阶段引入消息中间件）

---

## 1. 背景与目标

当前架构所有跨服务调用都走 HTTP（经 api-gateway 转发），同步串行。这导致：

1. **跨服务事务一致性脆弱** —— 例：iam-service 写用户 + 调 audit-service 写日志，audit 挂掉时业务回滚不掉。
2. **延迟累积** —— 一次登录请求需要串行触发 audit 写入；audit 慢则用户登录慢。
3. **耦合放大** —— audit-service 重启会影响 iam-service 可用性。
4. **缺少事件回放/分发能力** —— 未来 notification 想监听用户事件、风控想监听登录失败，都得改 iam-service 加 HTTP 调用。

Phase 4 引入异步事件总线，解决以上问题。本阶段只做**最小闭环**：iam-service → RabbitMQ → audit-service，验证整条链路工程上跑得通；其他服务在 Phase 4.x 子任务里逐个接入。

---

## 2. 范围

### 2.1 In Scope

- 引入 RabbitMQ 作为基础设施（infra docker-compose 加容器）。
- 新增 `libs/common-event` 共享库：完整的 Publisher / Consumer / Envelope / OutboxRepo 抽象 + RabbitMQ 实现。
- 启用 `contracts/events.yaml`，落地三个事件：`iam.user.created` / `iam.user.deleted` / `iam.login.succeeded`。
- 扩展 `scripts/contracts-gen.js`：从 events.yaml 生成 Rust event struct 与 routing-key 常量。
- iam-service 加 outbox 表 + 业务事务里写 outbox 行 + 独立 outbox publisher 二进制。
- audit-service 加 RabbitMQ consumer + 幂等写入 + 删除旧 HTTP 写入端点。
- e2e 用例覆盖事件链路。

### 2.2 Out of Scope（不在本 Phase）

- notification-service 接入事件（留给 Phase 4.x）。
- `iam.login.failed` / `iam.permission.changed` / `notification.announcement.published`（同上）。
- 多租户 `tenant_id` 实际下发（envelope 字段留好但置 null）。
- 自动重试退避 / 死信回放 UI（Phase 5 韧性三件套统一处理）。
- 跨服务 saga 编排。
- RabbitMQ 集群高可用（Phase 7 K3s 化时一起做）。

---

## 3. 拓扑

```
┌─────────────┐   tx commit     ┌────────────┐   FOR UPDATE   ┌──────────────────────┐
│ iam-service │ ──── 写 ───────►│ outbox 表  │◄── SKIP LOCKED │ outbox-publisher bin │
│ (handlers)  │                 │ (mysql)    │                │ (独立进程，可多副本) │
└─────────────┘                 └────────────┘                └─────────┬────────────┘
                                                                       │ publish + confirm
                                                                       ▼
                                              ┌────────────────────────────────────────┐
                                              │  RabbitMQ                              │
                                              │  exchange: events  (type=topic, dur)   │
                                              │  routing-keys:                         │
                                              │    iam.user.created                    │
                                              │    iam.user.deleted                    │
                                              │    iam.login.succeeded                 │
                                              │  DLX: events.dlx → queue events.dlq    │
                                              └─────────┬──────────────────────────────┘
                                                        │ binding "iam.#"
                                                        ▼
                                              ┌────────────────────────────────────────┐
                                              │ queue: audit.iam-events                │
                                              │ (durable, manual ack,                  │
                                              │  x-dead-letter-exchange = events.dlx)  │
                                              └─────────┬──────────────────────────────┘
                                                        │
                                                        ▼
                                              ┌────────────────────────────────────────┐
                                              │ audit-service consumer                 │
                                              │ INSERT … ON DUPLICATE KEY (event_id)   │
                                              └────────────────────────────────────────┘
```

### 关键拓扑决策

- **1 个 topic exchange `events`**：所有服务共用，靠 routing-key 命名空间隔离（`<service>.<resource>.<action>`）。
- **每个消费者一个具名 durable queue**：`<consumer>.<source>-events`，例 `audit.iam-events`。这样能独立 scale consumer 副本，也能在 management UI 观察堆积。
- **DLX 全局共用** `events.dlx` + 单个 `events.dlq`，简化运维（运维只盯一个死信队列）。
- **topology 通过 `definitions.json` 在 RabbitMQ 启动时声明**，不靠应用层 `queue_declare`，避免多副本竞态和漂移。

---

## 4. 事件 Envelope

### 4.1 JSON Schema

```jsonc
{
  "event_id":       "0190a0b1-...",        // UUIDv7，全局唯一，consumer 端幂等键
  "event_type":     "iam.user.created",    // 与 routing-key 一致
  "schema_version": 1,                     // payload schema 版本，向后兼容时递增
  "occurred_at":    "2026-06-26T12:00:00Z",// 业务发生时刻（不是 publish 时刻）
  "trace_id":       "4bf92f3577b34da6...", // W3C TraceContext trace-id（hex string，无 dashes）
  "tenant_id":      null,                  // Phase 8 启用，先保留字段
  "producer":       "iam-service",         // 排错用
  "payload":        { ... }                // 类型由 event_type 决定
}
```

### 4.2 首批 payload 定义

```yaml
# contracts/events.yaml 节选
events:
  iam.user.created:
    schema_version: 1
    payload:
      user_id:       { type: string,  required: true }
      username:      { type: string,  required: true }
      email:         { type: string,  required: false }
      org_unit_id:   { type: string,  required: false }

  iam.user.deleted:
    schema_version: 1
    payload:
      user_id:       { type: string,  required: true }
      deleted_by:    { type: string,  required: true }   # operator user_id

  iam.login.succeeded:
    schema_version: 1
    payload:
      user_id:       { type: string,  required: true }
      ip:            { type: string,  required: true }
      user_agent:    { type: string,  required: false }
      method:        { type: string,  required: true }   # password | sso | refresh
```

`contracts-gen.js` 扩展后产出：

- `libs/common-event/src/events.rs` —— 每个事件一个 `struct IamUserCreated { ... }` + `impl Event for ...`
- `libs/common-event/src/routing_keys.rs` —— `pub const IAM_USER_CREATED: &str = "iam.user.created";` 等常量
- `frontend/src/contracts/events.ts` —— 预留，但 Phase 4 不消费

---

## 5. `libs/common-event` API 设计

完整抽象，不只服务 iam→audit 一条链。

### 5.1 核心 trait

```rust
// libs/common-event/src/lib.rs

#[async_trait]
pub trait Event: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn event_type() -> &'static str;          // e.g. "iam.user.created"
    fn schema_version() -> u32 { 1 }
}

pub struct Envelope<P: Event> {
    pub event_id: Uuid,                       // UUIDv7
    pub occurred_at: DateTime<Utc>,
    pub trace_id: Option<String>,             // 从当前 OTel context 取
    pub tenant_id: Option<String>,
    pub producer: String,
    pub payload: P,
}

#[async_trait]
pub trait Publisher: Send + Sync {
    async fn publish<P: Event>(&self, env: &Envelope<P>) -> Result<(), PublishError>;
}

#[async_trait]
pub trait Consumer: Send + Sync {
    /// 订阅 routing-key 模式，handler 返回 Ok(()) 则 ack，返回 Err 则 nack → DLX。
    async fn subscribe<P, H, Fut>(&self, queue: &str, bindings: &[&str], handler: H)
        -> Result<(), ConsumeError>
    where
        P: Event,
        H: Fn(Envelope<P>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), HandlerError>> + Send;
}
```

### 5.2 Outbox 抽象

```rust
#[async_trait]
pub trait OutboxRepo<DB: sqlx::Database>: Send + Sync {
    /// 在业务事务里调用（共用 &mut Transaction），具体 DB 由实现选择（MySql / Sqlite）。
    async fn enqueue<P: Event>(
        &self,
        tx: &mut sqlx::Transaction<'_, DB>,
        env: &Envelope<P>,
    ) -> Result<(), OutboxError>;

    /// publisher 调用：抢一批未发送行，MySql 走 SELECT ... FOR UPDATE SKIP LOCKED，
    /// Sqlite 实现走普通 SELECT（单实例 dev 模式无竞态）。
    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;

    /// 标记已发送。
    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}

pub struct OutboxPublisher<R: OutboxRepo, P: Publisher> {
    repo: R,
    publisher: P,
    poll_interval: Duration,        // 默认 1s
    batch_size: u32,                // 默认 100
}

impl OutboxPublisher {
    /// 阻塞式 run（在独立二进制 main 里调用），SIGTERM 优雅退出。
    pub async fn run(self) -> Result<(), RunError>;
}
```

### 5.3 RabbitMQ 实现

- crate: `lapin = "2"` + `tokio-executor-trait`
- 连接：单连接 + 多 channel（publisher 与 consumer 各自一个 channel）
- publisher confirms 启用（`channel.confirm_select`）— 没收到 ack 不写 `published_at`
- consumer prefetch = 32（避免单消费者抢光）
- 重连：5s 退避，无限重试（让 outbox 堆积兜住）

### 5.4 配置

```toml
# config/config.yaml 新增 section
[event_bus]
url = "amqp://guest:guest@rabbitmq:5672/%2f"
exchange = "events"
publish_confirms = true

[outbox]
poll_interval_ms = 1000
batch_size = 100
backlog_warn_threshold = 10000
```

---

## 6. Outbox Publisher 独立二进制

### 6.1 为什么独立

- 业务进程重启不会丢正在发的事件（最差是堆积，恢复后追上）。
- 可独立 scale（高峰期跑多副本，无业务进程时也能继续发）。
- 故障域隔离：publisher 抖动不连累 HTTP 业务接口。

### 6.2 多副本协调

**用 MySQL `SELECT ... FOR UPDATE SKIP LOCKED` 天然分片** —— 多个 publisher 副本各抢各的批次，互不阻塞，无需 Consul leader 选举：

```sql
SELECT id, event_id, routing_key, envelope_json
FROM outbox_events
WHERE published_at IS NULL
ORDER BY id ASC
LIMIT 100
FOR UPDATE SKIP LOCKED;
```

SQLite 不支持 `FOR UPDATE`，但 SQLite 模式（仅 prue_backend dev）天然单实例，跳过加锁也安全。`OutboxRepo` 的 sqlx 实现里按 driver 走两套 SQL。

### 6.3 部署形态

- crate `services/iam-service-outbox-publisher`（独立 package，复用 iam-service 的 config / 表结构，但不依赖 iam-service crate）。或者 `services/iam-service` 加 `[[bin]] name = "outbox-publisher"`。
- **倾向后者**：少一个 crate 与 Dockerfile，复用同一份镜像（启动命令切换），符合 12-factor。
- Dockerfile entry 通过 `CMD` 切换：服务进程 vs publisher 进程。
- K3s/compose 里部署成两份 deployment / service（Phase 7 一起做；Phase 4 在 docker-compose-services.yml 加一个 `iam-outbox-publisher` 容器即可）。

### 6.4 关闭流程

`SIGTERM` → 等当前批次 publish 完成 + confirm 收到 + mark_published 写完 → 退出。最长等待 30s 后强制退出（已 publish 但未 mark 的会在重启后被另一个副本重发，consumer 幂等兜住）。

---

## 7. 数据库改动

### 7.1 iam-service 新迁移

`services/iam-service/migrations/mysql/V{NNN}__outbox_events.sql`：

```sql
CREATE TABLE outbox_events (
    id              BIGINT       NOT NULL AUTO_INCREMENT,
    event_id        CHAR(36)     NOT NULL,
    event_type      VARCHAR(128) NOT NULL,
    routing_key     VARCHAR(128) NOT NULL,
    envelope_json   JSON         NOT NULL,
    created_at      DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
    published_at    DATETIME(3)  NULL,
    PRIMARY KEY (id),
    UNIQUE KEY uk_outbox_event_id (event_id),
    KEY idx_outbox_unpublished (published_at, id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

iam-service 当前没有 sqlite 迁移分支，不需要补 sqlite 版本。

### 7.2 audit-service 新迁移

`services/audit-service/migrations/mysql/V{NNN}__event_id_idempotency.sql`：

```sql
ALTER TABLE audit_events
    ADD COLUMN event_id CHAR(36) NULL AFTER id,
    ADD UNIQUE KEY uk_audit_event_id (event_id);
```

（具体列名以 audit-service 当前 schema 为准；实施阶段再确认列名。）

### 7.3 publisher 归档（避免无限增长）

加一条 cron / 启动后台 task：每 1h 删除 `published_at < NOW() - INTERVAL 24 HOUR` 的行。常态下 outbox 表 < 几 MB。

---

## 8. iam-service 改动

### 8.1 业务事务里写 outbox

`services/iam-service/src/service/user_svc.rs::create_user` 类伪码：

```rust
let mut tx = pool.begin().await?;
let user = sqlx::query!("INSERT INTO users ...").execute(&mut *tx).await?;

let env = Envelope::new(IamUserCreated {
    user_id: user.id.to_string(),
    username: req.username,
    email: req.email,
    org_unit_id: req.org_unit_id,
});
outbox.enqueue(&mut tx, &env).await?;

tx.commit().await?;
```

要改的位置（grep `audit_client` / `AuditClient` / `record_login` / `record_op` 类）：
- `service/auth_svc.rs::login_success` → 写 `iam.login.succeeded`
- `service/user_svc.rs::create_user` → 写 `iam.user.created`
- `service/user_svc.rs::delete_user` → 写 `iam.user.deleted`

### 8.2 删除旧的 audit HTTP 客户端调用

一并删除 `services/iam-service/src/` 里所有同步调 audit 的代码（包括 reqwest 客户端、错误处理、配置 section）。

---

## 9. audit-service 改动

### 9.1 新增 consumer

`services/audit-service/src/consumer.rs`：

```rust
pub async fn run(state: AppState, consumer: impl Consumer) -> Result<()> {
    consumer.subscribe::<IamEventEnum, _, _>(
        "audit.iam-events",
        &["iam.#"],
        move |env| {
            let state = state.clone();
            async move {
                let row = AuditRow::from_envelope(&env);
                // 幂等：ON DUPLICATE KEY UPDATE updated_at = VALUES(updated_at)
                sqlx::query("INSERT INTO audit_events ... ON DUPLICATE KEY ...")
                    .bind(env.event_id)
                    .bind(...)
                    .execute(&state.db)
                    .await?;
                Ok(())
            }
        },
    ).await
}
```

`IamEventEnum` 是 contracts-gen 生成的 tagged enum，根据 `event_type` 字段反序列化。

`run` 由 `main.rs` 在 axum server 旁起一个 tokio task 一起跑（audit-service 不需要 outbox，所以不拆 bin）。

### 9.2 删除旧写入接口

audit-service 当前对外的 `POST /audit/...` 写入端点全部删除（保留只读查询 / 导出）：
- 删 handler
- 删 openapi 注解
- 跑 `scripts/dump-openapi-specs.sh` 重新生成 yaml
- 删 e2e 用例里走 HTTP 写 audit 的步骤

---

## 10. 基础设施改动

### 10.1 RabbitMQ 容器

`infrastructure/docker-compose-infra.yml` 加：

```yaml
rabbitmq:
  image: rabbitmq:3.13-management-alpine
  container_name: rabbitmq
  ports:
    - "5672:5672"
    - "15672:15672"
  volumes:
    - rabbitmq-data:/var/lib/rabbitmq
    - ./config/rabbitmq/definitions.json:/etc/rabbitmq/definitions.json:ro
    - ./config/rabbitmq/rabbitmq.conf:/etc/rabbitmq/rabbitmq.conf:ro
  healthcheck:
    test: ["CMD", "rabbitmq-diagnostics", "-q", "ping"]
    interval: 10s
    timeout: 5s
    retries: 5
```

### 10.2 topology 声明文件

`infrastructure/config/rabbitmq/definitions.json` 预声明：
- exchange `events`（topic, durable）
- exchange `events.dlx`（fanout, durable）
- queue `events.dlq`（durable, no DLX）
- queue `audit.iam-events`（durable, x-dead-letter-exchange=events.dlx）
- binding events → audit.iam-events on `iam.#`

`rabbitmq.conf` 启用 `management.load_definitions = /etc/rabbitmq/definitions.json`。

### 10.3 docker-compose-services.yml 新增 publisher 容器

```yaml
iam-outbox-publisher:
  image: <same as iam-service>
  command: ["outbox-publisher"]
  environment:
    - SERVICE_NAME=iam-outbox-publisher
  depends_on: [mysql, rabbitmq]
```

---

## 11. 可观测性

### 11.1 Metrics（Prometheus）

新增指标（在 `libs/common-event` 里集中暴露）：

- `event_publish_total{event_type, status}` —— counter，success / failure
- `event_consume_total{queue, event_type, status}` —— counter，success / handler_error / decode_error
- `outbox_backlog` —— gauge，每个 publisher 副本周期性更新
- `outbox_publish_latency_seconds` —— histogram，从 `created_at` 到 `published_at` 的延迟

### 11.2 Tracing（OTel）

- iam-service 写 outbox 时，把当前 span 的 `trace_id` 写进 envelope。
- publisher 发送时新起 span `event.publish`，链 envelope.trace_id 作为 link（不作为 parent，避免长 span）。
- consumer 收到时从 envelope.trace_id 重建 span context，新起 span `event.consume`，handler 内的 DB 写入挂在这个 span 下。
- audit-service Phase 4 顺手补上 OTel 接入（在 Phase 6 计划之内提前做一部分，因为不接 trace 就看不到端到端链路 — 这是 Phase 4 验收要素）。

### 11.3 日志

每条事件 publish / consume 输出结构化日志，字段：`event_id` / `event_type` / `trace_id` / `producer` / `consumer`。

---

## 12. 验收标准

- [ ] `docker compose up -d` 后 RabbitMQ 健康，management UI（15672）能看到 exchange / queue / binding。
- [ ] iam-service 调 `POST /v1/iam/users` 后：
  - outbox_events 表出现一行，几秒内 `published_at` 被填充。
  - audit_events 表出现对应行，`event_id` 与 envelope 一致。
  - Jaeger 上能从 iam handler span 跳转到 audit consumer span（trace_id 一致）。
- [ ] 停掉 RabbitMQ 容器：
  - iam-service HTTP 接口仍 200。
  - outbox 表堆积，`outbox_backlog` metric 上升。
  - 启回 RabbitMQ，publisher 在 < 10s 内追上。
- [ ] 让 audit consumer 故意 panic：消息进 `events.dlq`，手工 shovel 回 `audit.iam-events` 后被正确消费（幂等 — 不重复写）。
- [ ] 全量 e2e + smoke 通过；audit 旧 HTTP 写入端点已从 openapi.yaml 与代码中移除。
- [ ] `scripts/check-spec.js` / `contracts-gen.js --check` / `check-service-standard.js` 全绿。
- [ ] 单服务 cargo test 全绿（iam-service / audit-service / libs/common-event）。

---

## 13. 风险与缓解

| 风险 | 缓解 |
|---|---|
| RabbitMQ 单点 | Phase 4 单实例与 mysql/redis 同等可用性；outbox 兜底；Phase 7 升集群 |
| outbox 表写放大 | 每天归档 published rows；指标监控大小 |
| publisher 重复发送（多副本竞态）| SKIP LOCKED + consumer 幂等键双重保险 |
| 删除 audit 旧 HTTP 端点的破坏性 | spec 实施前先 grep 所有调用点，确认无外部依赖；删除与新通路上线同 commit |
| sqlx `sqlx::Any` driver 与 outbox 抽象兼容性 | iam-service 当前用具体 driver 而非 Any；OutboxRepo 设计成泛型 `<DB: Database>` 而非 Any，规避类型擦除带来的麻烦 |
| events.yaml schema → Rust struct 代码生成复杂 | 借鉴现有 contracts-gen 模式（error-codes / permissions 都是 yaml→rs），增量扩展不重写 |
| OTel trace context 跨 MQ 传播 | envelope 的 trace_id 字段是显式 carrier，不依赖 amqp header propagator（管控更清晰） |

---

## 14. 后续 Phase 4.x 子任务（Phase 4 验收后开新 spec）

- 4.1: notification-service 订阅 `iam.user.created` → 站内信欢迎
- 4.2: 落地 `iam.login.failed` + 简易风控消费者
- 4.3: 落地 `notification.announcement.published`，WebSocket 广播改由事件驱动
- 4.4: `permission_changed` 事件 + 各服务缓存失效

每个子任务复用 `libs/common-event` 与 outbox 模式，预期单 spec < 半天。

---

## 15. 决策记录

| 决策 | 选择 | 理由 |
|---|---|---|
| 消息中间件 | RabbitMQ | 用户选型；AMQP 路由模型成熟，lapin Rust 客户端稳定 |
| Topology | 单 topic exchange + routing-key 命名 | 标准 DDD 事件总线做法，未来加事件无需改拓扑 |
| 一致性 | Outbox + 每服务一张表 + 同进程事务 | 与本项目「服务拥有自己的 DB」原则一致 |
| Publisher 部署 | 独立二进制 + 同镜像切 CMD | 故障域隔离 + 复用镜像，符合 12-factor |
| 多副本协调 | `SELECT ... FOR UPDATE SKIP LOCKED` | 教科书方案，无外部 leader election 依赖 |
| 序列化 | JSON envelope | 可观测性好，Phase 4 量级下性能足够 |
| 幂等 | event_id (UUIDv7) + consumer 唯一键 | At-least-once + 幂等键 = 端到端 exactly-once |
| 首批范围 | iam→audit 三事件 | 验证最小闭环，风险可控 |
| 旧 HTTP 端点 | 直接删除，不留 fallback | 用户选型；事件链路验收通过后单 commit 删除 |
| common-event 完整度 | 完整抽象，仅 wire 首条链路 | 用户选型；避免 Phase 4.x 重构 |

---

_End of design._
