# 41 — notification 欢迎站内信（Phase 4.1）

**链路**：iam 创建用户 → 发 `iam.user.created`（outbox）→ iam-outbox-publisher → RabbitMQ
`notification.iam-events` 队列 → notification consumer → 按 welcome 模板生成个人公告 + 落 delivery。

前置：完成 `00-setup.md`，已有 `$BASE` / `$TOKEN` / 工具函数。

## 场景 41-A：新建用户触发欢迎站内信

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| A1 | 创建一个新用户：<br>`authpost /v1/iam/users '{"username":"alice","password":"P@ssw0rd!","nickname":"Alice"}'` | 200/201，返回用户，记下其 `id`（=`$UID`） | | |
| A2 | 查 iam outbox 是否写入事件：<br>`mysqlq "SELECT event_type,created_at,published_at FROM iam_outbox_events WHERE event_type='iam.user.created' ORDER BY id DESC LIMIT 3"` | 有一行 `iam.user.created`，`published_at` 几秒内被填（publisher 已发出） | | |
| A3 | 等 1~3s，查该用户是否收到欢迎公告：<br>`mysqlq "SELECT a.id,a.title,a.status FROM ntf_announcements a JOIN ntf_announcement_deliveries d ON d.announcement_id=a.id WHERE d.user_id='$UID'"` | 至少一条 `status=published` 的欢迎公告 | | |
| A4 | 校验幂等表：<br>`mysqlq "SELECT event_id,event_type FROM ntf_event_idempotency WHERE event_type='iam.user.created' ORDER BY 1 DESC LIMIT 3"` | 该事件 event_id 已记录 | | |
| A5 | 校验占位符已渲染（标题/正文含 username，非 `{{username}}`）：<br>`mysqlq "SELECT title,LEFT(body,120) FROM ntf_announcements WHERE created_by='system' ORDER BY id DESC LIMIT 1"` | 出现 `alice`（或 nickname），无残留 `{{...}}` | | |

## 场景 41-B：幂等（事件重投不重复建公告）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| B1 | 记下 A3 中欢迎公告条数 N | | | |
| B2 | ⚠️ 在 RabbitMQ 管理台（15672）→ Queues → `notification.iam-events`，或重启 notification consumer：<br>`docker compose -f infrastructure/docker-compose-services.yml restart notification-service` | 服务重启，consumer 重新订阅 | | |
| B3 | 再查 A3 同一 SQL | 欢迎公告仍是 N 条（未因重连/重投翻倍） | | |

## 验收判定

- ✅ 通过：新用户自动收到欢迎公告、可在 `list_for_user` 路径查到、占位符已渲染、重投不重复。
- ❌ 失败常见原因：outbox `published_at` 一直为 NULL（publisher 没起来/连不上 RabbitMQ）；
  consumer 未订阅（看 notification 日志 `subscribed to notification.iam-events`）。

## 排查命令

```bash
# notification 是否订阅成功
docker compose -f infrastructure/docker-compose-services.yml logs notification-service | grep -i "subscribed\|welcome"
# publisher 是否在发
docker compose -f infrastructure/docker-compose-services.yml logs iam-outbox-publisher | grep -i "published"
# RabbitMQ 队列堆积？管理台 Queues 看 notification.iam-events 的 messages_ready
```
