# T3: Consumer Transient/Permanent + Bounded Redelivery — 手工 e2e 验收

## 前置

```bash
docker compose -f infrastructure/docker-compose-infra.yml up -d
# RabbitMQ + outbox-publisher + 各服务
```

## 场景 1：Permanent → DLQ

构造一个不合法 payload 触发 audit consumer 返 Permanent。

观察：
- audit-service 日志: `"handler permanent error → DLQ"`
- RabbitMQ Management UI (http://localhost:15672):
  - Queue `events.dlq` 深度 +1
  - Queue `audit.iam-events` 深度 = 0

## 场景 2：Transient 自愈

正常创建用户；audit consumer 短暂返 Transient（可以用 iptables drop MySQL 5s 模拟）。

观察：
- audit-service 日志: `"transient → republished"` count=1
- iptables 解封后：count 1/2/3 之前 handler 应已成功 Ok
- 最终 DLQ 0 条，业务成功

## 场景 3：Transient 升级 DLQ

停 MySQL 容器；触发业务事件。

观察：
- audit-service 日志: `"transient → republished"` count=1, 2, 3
- 第 4 次处理时: `"transient exceeded redelivery limit → DLQ"` count=3
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
