---
name: coder-contract-trace
description: |
  Trace cross-service business events flowing through RabbitMQ + transactional outbox.
  Use when user says "trace event", "outbox", "event flow", "事件链路", "RabbitMQ",
  "跨服务追踪", "事件没消费", "事件丢了", "consumer not running", "event published
  but not consumed", "跨服务事件".
---

# coder-contract-trace Skill

端到端追踪跨服务业务事件（RabbitMQ + transactional outbox）。

## 何时用

用户说「事件丢了」「事件没消费」「outbox 卡住」「RabbitMQ 队列积压」「trace event」「事件链路」「跨服务事件」「consumer not running」时调用本 skill。

## 前置

- MySQL 可连接（`superpowers` 库）
- RabbitMQ management 端口 15672 可访问（`guest/guest`）
- `docker compose` 容器运行中
- 日志可查（`docker compose logs -f <svc>`）

## 端到端事件生命周期（4 阶段）

```
Producer DB事务 ──▶ outbox表 ──▶ outbox_publisher ──▶ RabbitMQ ──▶ Consumer处理
  阶段1              阶段2              阶段3            阶段4
```

### 阶段 1：业务事务内 enqueue

**数据落点**：MySQL `superpowers` 库，`iam_outbox_events` 或 `ntf_outbox_events` 表。

**关键代码位置**：
- `libs/digua_common_event/src/outbox.rs` — `MySqlOutboxRepo::enqueue`
- `services/iam-service/src/service/perm_change.rs` — `MySqlOutboxRepo::enqueue` 调用处
- `services/notification-service/src/service/announcement_service.rs` — `publish_announcement` 内 enqueue

**故障症状**：业务操作成功提交但事件未出现在 outbox 表。

**排查命令**：
```sql
-- 查询 outbox 表未发布条目（pending 状态）
SELECT id, event_id, event_type, routing_key, published_at,
       TIMESTAMPDIFF(SECOND, id, NOW()) AS pending_seconds
FROM iam_outbox_events
WHERE published_at IS NULL
ORDER BY id ASC
LIMIT 20;

-- 同理 notification-service
SELECT id, event_id, event_type, routing_key, published_at
FROM ntf_outbox_events
WHERE published_at IS NULL
ORDER BY id ASC
LIMIT 20;

-- 查最近已发布（确认 enqueue 写进去了）
SELECT id, event_type, routing_key, published_at
FROM iam_outbox_events
ORDER BY id DESC
LIMIT 10;

-- 按 event_type 统计 pending 数量
SELECT event_type, COUNT(*) AS pending_count
FROM iam_outbox_events
WHERE published_at IS NULL
GROUP BY event_type;
```

**Fail mode → grep one-liner**：
- 事件根本没写入 outbox：`grep -E "enqueue|outbox" services/iam-service/src/service/perm_change.rs` 确认 `MySqlOutboxRepo::enqueue` 有被调用
- 事务回滚导致 outbox 行消失：检查 MySQL binlog / 确认业务操作是否真正 commit

---

### 阶段 2：outbox publisher 投递

**数据落点**：MySQL outbox 表（`published_at` 列从 NULL 变为时间戳）；RabbitMQ `events` exchange（topic）。

**关键代码位置**：
- `services/iam-outbox-publisher/src/main.rs` — `iam_outbox_publisher` 主循环
- `services/notification-service/src/bin/outbox_publisher.rs` — `ntf_outbox_publisher` 主循环
- `libs/digua_common_event/src/outbox.rs` — `claim_batch`（`FOR UPDATE SKIP LOCKED`）、`mark_published`

**故障症状**：outbox 表中 `published_at` 始终为 NULL；publisher 进程不存在或崩溃重启。

**排查命令**：
```bash
# 确认 publisher 容器是否在跑
docker compose -f infrastructure/docker-compose.yml ps iam-outbox-publisher
docker compose -f infrastructure/docker-compose.yml ps ntf-outbox-publisher

# 查看 publisher 日志（观察 "published outbox batch" / "claim_batch failed" / "reconnect" 等关键字）
docker compose -f infrastructure/docker-compose.yml logs -f iam-outbox-publisher
docker compose -f infrastructure/docker-compose.yml logs -f ntf-outbox-publisher

# 手动模拟一次 claim + publish（验证 DB 连接正常）
# 在 MySQL 客户端执行：确认 pending 条目数是否持续减少
SELECT COUNT(*) FROM iam_outbox_events WHERE published_at IS NULL;
SELECT COUNT(*) FROM ntf_outbox_events WHERE published_at IS NULL;

# 查看已发布但未归档的行（published_at 非 NULL）
SELECT id, event_type, published_at,
       TIMESTAMPDIFF(HOUR, published_at, NOW()) AS published_hours_ago
FROM iam_outbox_events
WHERE published_at IS NOT NULL
ORDER BY published_at DESC
LIMIT 10;
```

**Fail mode → grep one-liner**：
- publisher 未运行：`docker compose ps | grep outbox` 确认两个 publisher 都在 Up
- publisher 连不上 RabbitMQ：`docker compose logs iam-outbox-publisher | grep -i "connect\|rabbitmq\|error"` 
- publisher 连不上 MySQL：`docker compose logs iam-outbox-publisher | grep -i "mysql\|pool\|connect" | head -5`
- 批次发布失败（broker 端）：日志中搜索 `broker_failed` / `publish failed`

---

### 阶段 3：RabbitMQ 路由

**数据落点**：RabbitMQ `events` exchange（topic）→ 队列绑定；`events.dlx` → `events.dlq`（死信）。

**关键代码位置**：
- `infrastructure/config/rabbitmq/definitions.json` — exchange/queue/binding 拓扑（预声明）
- `libs/digua_common_event/src/publisher.rs` — `RabbitMqPublisher::publish_raw`

**RabbitMQ 拓扑（来自 definitions.json）**：

| Exchange | Type | 用途 |
|---|---|---|
| `events` | topic | 主事件总线 |
| `events.dlx` | fanout | 死信交换机 |

| Queue | Routing Key | 消费者 |
|---|---|---|
| `audit.iam-events` | `iam.#` | audit-service |
| `notification.iam-events` | `iam.user.created` | notification-service |
| `notification.membership-events` | `iam.user.role.assigned`, `iam.user.role.unassigned`, `iam.user.dept.changed` | notification-service |
| `audit.login-risk` | `iam.login.failed` | audit-service |
| `events.dlq` | (DLX) | 死信队列 |

**故障症状**：事件已发布（`published_at` 非 NULL）但消费者未收到；队列消息积压。

**排查命令**：
```bash
# 进入 rabbitmq 容器
docker compose -f infrastructure/docker-compose.yml exec rabbitmq rabbitmqadmin list queues

# 查看队列消息数（messages = 积压，publishers = 生产者数量）
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin list queues name messages consumers

# 查看指定队列详情
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin get queue=notification.iam-events ackmode=ack_requeue_false count=5

# 查看指定队列详情（membership events）
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin get queue=notification.membership-events ackmode=ack_requeue_false count=5

# 查看 DLQ 积压（消息在 consumer 失败后进入此处）
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin list queues name messages | grep dlq

# 列出所有 exchange
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin list exchanges name type
```

**Fail mode → grep one-liner**：
- 队列无消费者：`docker compose exec rabbitmq rabbitmqadmin list queues | grep -v "0 0"` 找 consumers=0 的队列
- 路由键不匹配（事件进了错误队列）：`docker compose exec rabbitmq rabbitmqadmin get queue=notification.iam-events count=0` 确认是否有 `iam.user.created` 进去
- DLQ 有积压：`docker compose exec rabbitmq rabbitmqadmin list queues | grep dlq` — 说明 consumer 处理失败了

---

### 阶段 4：下游 consumer 处理

**数据落点**：consumer 服务内业务逻辑；MySQL 业务表；`events.dlq`（处理失败时）。

**关键代码位置**：
- `services/notification-service/src/consumer/iam_events.rs` — `iam.user.*` 事件处理（`IamEventConsumer`）
- `services/audit-service/src/` — 审计事件消费者（订阅 `audit.iam-events` / `audit.login-risk`）
- `libs/digua_common_event/src/consumer.rs` — `RabbitMqConsumer` 实现

**故障症状**：队列有消息（`messages > 0`）但业务未生效；consumer 报错退出。

**排查命令**：
```bash
# 查看 consumer 服务日志（搜索 "handling\|consumed\|error\|panic\|failed"）
docker compose -f infrastructure/docker-compose.yml logs -f notification-service | grep -i "event\|consumer\|error"
docker compose -f infrastructure/docker-compose.yml logs -f audit-service | grep -i "event\|consumer\|error"

# 确认 consumer 容器进程还在
docker compose -f infrastructure/docker-compose.yml ps notification-service
docker compose -f infrastructure/docker-compose.yml ps audit-service

# 从 consumer 视角直接消费一条消息（验证处理逻辑，dry-run 模式）
# 先找 consumer 绑定的是哪个队列
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin list consumers

# 查看 DLQ 消息内容（从这里看到失败事件的原始 payload）
docker compose -f infrastructure/docker-compose.yml exec rabbitmq \
  rabbitmqadmin get queue=events.dlq ackmode=ack_requeue_false count=3
```

**Fail mode → grep one-liner**：
- consumer panic：`docker compose logs notification-service | grep -iE "panic|thread.*panicked|assertion"`
- 事件处理异常（未进 DLQ 但业务未生效）：`docker compose logs notification-service | grep -iE "error|handler.*failed"`
- 幂等去重失效（重复消费）：业务表查同一 `event_id` 是否被多次处理

---

## 调试清单（按阶段）

| 阶段 | 检查点 | 命令 |
|---|---|---|
| 1 | outbox 行是否写入 | `SELECT COUNT(*) FROM iam_outbox_events WHERE published_at IS NULL` |
| 1 | 事务是否真正 commit | `SHOW ENGINE INNODB STATUS` 看活跃事务 |
| 2 | publisher 进程是否存活 | `docker compose ps \| grep outbox` |
| 2 | claim_batch 是否有 pending | 同上，pending count 持续为 0 则 publisher 正常 |
| 2 | RabbitMQ 连接是否建立 | `docker compose logs iam-outbox-publisher \| grep connected` |
| 3 | 队列是否有积压 | `rabbitmqadmin list queues name messages` |
| 3 | 路由键是否匹配 binding | `rabbitmqadmin list bindings source_name routing_key` |
| 3 | DLQ 是否有消息 | `rabbitmqadmin list queues \| grep dlq` |
| 4 | consumer 是否注册 | `rabbitmqadmin list consumers` |
| 4 | consumer 日志是否有错 | `docker compose logs <svc> \| grep -iE "error\|failed\|panic"` |
| 4 | 业务表是否被正确更新 | 查业务表对应 record 的 updated_at / version |

---

## 验证

### 端到端时间线（iam.user.created 发起到 notification-service 消费完毕）

```
T0  业务请求进入 iam-service handler
    │
T1  DB事务开始
    ├─ 写入 iam_users 行（业务数据）
    └─ 调用 MySqlOutboxRepo::enqueue(&mut tx, &Envelope)
       └─ 写入 iam_outbox_events（event_id=UUIDv7, event_type=iam.user.created）
    └─ COMMIT（原子提交：业务行 + outbox 行同时落盘）
           │
T2  iam_outbox_publisher 轮询（默认 1000ms）
    ├─ claim_batch(): SELECT ... FOR UPDATE SKIP LOCKED
    │   └─ 找到 pending 行，返回 Vec<OutboxRow>
    └─ 对每行调用 RabbitMqPublisher::publish_raw(routing_key, event_id, envelope_json)
       └─ 路由到 exchange=events, routing_key=iam.user.created
           │
T3  RabbitMQ 根据 binding 路由
    └─ queue=notification.iam-events（binding: iam.user.created）
           │
T4  notification-service consumer 接收（RabbitMqConsumer）
    ├─ 解析 envelope_json → Envelope<IamUserCreatedPayload>
    ├─ 查 event_id 幂等（已处理过？未处理则继续）
    └─ 执行业务逻辑（写入 ntf_user_projection 等）
           │
T5  consumer ack 消息
    └─ 消息从 queue 中移除
```

**验证完整性**（从 T1 到 T5 全链路）：
```bash
# T1 前：outbox 应无 pending
SELECT COUNT(*) FROM iam_outbox_events WHERE published_at IS NULL AND event_type='iam.user.created';

# T2 后（publisher 跑过）：published_at 变为非 NULL
SELECT published_at FROM iam_outbox_events WHERE event_type='iam.user.created' ORDER BY id DESC LIMIT 1;

# T3 后（RabbitMQ）：队列 messages 计数应 > 0（积压）或 = 0（已被消费）
docker compose exec rabbitmq rabbitmqadmin list queues name messages | grep notification.iam-events

# T5 后（消费完毕）：DLQ 应为空，业务表有新 record
SELECT * FROM ntf_user_projection WHERE user_id=? ORDER BY updated_at DESC LIMIT 1;
```

### 常用日志关键字（在 docker compose logs 中过滤）

| 关键字 | 含义 | 适用服务 |
|---|---|---|
| `outbox_enqueued`（若使用） | 阶段1完成，事件写入outbox | iam-service |
| `claim_batch` | 阶段2 publisher取批次 | iam-outbox-publisher, ntf-outbox-publisher |
| `published outbox batch` | 阶段2发布成功 | iam-outbox-publisher, ntf-outbox-publisher |
| `publish failed` | 阶段2发布到RabbitMQ失败 | iam-outbox-publisher, ntf-outbox-publisher |
| `event_consumed`（若使用） | 阶段4消费成功 | notification-service, audit-service |
| `event_handler_failed` | 阶段4处理异常 | notification-service, audit-service |
| `DLX\|dlq\|dead.letter` | 消息进了死信 | rabbitmq |

### 快速定位「事件卡在哪」决策树

```
事件丢失？
  │
  ├─ outbox表查不到该event_type的pending行
  │     → 阶段1失败：业务handler未调用enqueue
  │     → grep "MySqlOutboxRepo::enqueue" 查producer代码
  │
  ├─ outbox有pending但published_at为NULL
  │     → 阶段2失败：publisher未运行或未取到该批次
  │     → docker compose ps | grep outbox
  │     → docker compose logs iam-outbox-publisher | grep -i "claim\|failed"
  │
  ├─ published_at已更新但消费者未收到
  │     → 阶段3失败：RabbitMQ路由问题
  │     → docker compose exec rabbitmq rabbitmqadmin list queues
  │     → 查 routing_key 是否匹配 binding
  │
  └─ 队列有消息但业务未生效
        → 阶段4失败：consumer处理异常
        → docker compose logs notification-service | grep -i "error\|panic"
        → docker compose exec rabbitmq rabbitmqadmin get queue=events.dlq
```

---

## 参考文件索引

| 资源 | 路径 |
|---|---|
| outbox 表结构 + enqueue 实现 | `libs/digua_common_event/src/outbox.rs` |
| envelope 结构（event_id, trace_id, payload） | `libs/digua_common_event/src/envelope.rs` |
| iam outbox publisher 主循环 | `services/iam-outbox-publisher/src/main.rs` |
| ntf outbox publisher 主循环 | `services/notification-service/src/bin/outbox_publisher.rs` |
| RabbitMQ 拓扑（exchange/queue/binding） | `infrastructure/config/rabbitmq/definitions.json` |
| 事件类型定义（routing key） | `contracts/events.yaml` |
| notification-service consumer | `services/notification-service/src/consumer/iam_events.rs` |
| iam-service 生产事件调用处 | `services/iam-service/src/service/perm_change.rs` |
