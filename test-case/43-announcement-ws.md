# 43 — 公告发布 → 事件驱动 WebSocket 广播（Phase 4.3）

**链路**：发布公告（REST `/publish` 或定时到点）→ `Db::publish_announcement` 单事务
（行锁 + 解析 user-scope 接收人 + 插 delivery + 翻 published + 写 `ntf_outbox_events`）→
ntf-outbox-publisher → RabbitMQ `events` exchange（rk=`notification.announcement.published`）→
**每个 notification 副本各自的 ephemeral 队列**消费 → 对本副本连着的、命中 user_ids 的用户推
`WsOut::New`。本期仅 **user-scope** 接收人（role/dept 仅 warn 跳过）。

前置：`00-setup.md` 完成。WebSocket 端点：`ws://localhost:3000/v1/notification/ws`（经 gateway）
或直连 `ws://localhost:6003/notification/ws`，握手需带 `x-user-id` 头（gateway 会从 JWT 注入；
直连测试时手动带）。

## 场景 43-A：单实例 — 发布即收到 WS 推送 + 可在列表查到

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| A1 | 取一个目标用户 id（用 0.2 的 admin，或 41 创建的 alice），记为 `$UID` | | | |
| A2 | 开一个 WS 客户端连上（见下方「WS 连接片段」），订阅 `$UID` | 连接成功，等待消息 | | |
| A3 | 创建 user-scope 公告：<br>`authpost /v1/notification/announcements '{"title":"hello","body":"hi","publish_at":"2020-01-01T00:00:00Z","pinned":false,"popover":false,"recipients":[{"scope_type":"user","scope_id":"'$UID'"}]}'` | 200/201，记下公告 `id`（=`$AID`） | | |
| A4 | 发布：`authpost /v1/notification/announcements/$AID/publish` | 200 | | |
| A5 | 看 WS 客户端 | 收到一条 `{"type":"new","payload":{"id":<AID>,"title":"hello",...}}` | | |
| A6 | 查 outbox：<br>`mysqlq "SELECT event_type,published_at FROM ntf_outbox_events WHERE event_type='notification.announcement.published' ORDER BY id DESC LIMIT 1"` | 有行，`published_at` 已填 | | |
| A7 | 查 delivery（reload 兜底）：<br>`mysqlq "SELECT announcement_id,user_id FROM ntf_announcement_deliveries WHERE announcement_id=$AID"` | 有 `$UID` 的 delivery 行 | | |
| A8 | 不依赖 WS，直接拉列表（模拟刷新）：<br>`authget /v1/notification/announcements` | 列表含 `$AID`（delivery 持久兜底生效） | | |

## 场景 43-B：多副本 — 跨副本广播（核心，验证事件驱动的意义）

单实例下 in-process WsHub 也能推；本场景验证**用户 WS 连在副本 A、发布发生在副本 B 时仍能收到**。

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| B1 | ⚠️ 扩到 2 副本：<br>`docker compose -f infrastructure/docker-compose-services.yml up -d --scale notification-service=2` | 两个 notification 容器 Up | | |
| B2 | 确认两副本各自声明了 ephemeral 队列：RabbitMQ 管理台 → Queues，应看到 **2 个** server-named（随机名）队列绑定 `notification.announcement.published` | 2 个 ephemeral 队列 | | |
| B3 | WS 客户端连到**副本 1**（可临时给某副本映射独立端口，或多连几次确保落在某副本） | 连接成功 | | |
| B4 | 经 gateway 发布一条含该用户的 user-scope 公告（gateway 轮询到哪个副本不确定，正是要验证的点） | | | |
| B5 | WS 客户端收到 `new` | ✅ 收到（无论发布请求落在哪个副本，事件 fanout 到所有副本） | | |
| B6 | 收尾：`docker compose -f infrastructure/docker-compose-services.yml up -d --scale notification-service=1` | 回到 1 副本 | | |

## 场景 43-C：role/dept scope 本期跳过（负向）

| 步骤 | 操作 | 期望结果 | 实测 | P/F |
|------|------|---------|------|-----|
| C1 | 创建 role-scope 公告并发布（recipients 用 `{"scope_type":"role","scope_id":"<某role>"}`） | 发布 200 | | |
| C2 | 看 notification 日志 | 出现 `role/dept recipient scopes skipped`（user-scope only） | | |
| C3 | 事件 user_ids 为空 → 无人收到 WS（符合本期范围，role/dept 是 follow-up 4.3-fu2） | | | |

## WS 连接片段（任选其一）

**websocat**（`brew install websocat` / `cargo install websocat`）：
```bash
# 直连 notification（带 x-user-id 头）
websocat -H="x-user-id: $UID" ws://localhost:6003/notification/ws
# 连上后按协议发送握手（若服务要求 token 帧，参考 ws.rs；当前实现读 x-user-id 头即可）
```

**浏览器 Console**（经 gateway，自动带 JWT）：
```js
const ws = new WebSocket("ws://localhost:3000/v1/notification/ws");
ws.onmessage = e => console.log("WS:", e.data);
ws.onopen = () => ws.send(JSON.stringify({ token: "<贴上 access_token>" }));
```

## 验收判定

- ✅ 通过：发布后 WS 收到 `new`、delivery 落库、刷新列表可见；2 副本时跨副本仍收到（B2 看到 2 个 ephemeral 队列是关键证据）；role/dept 被跳过。
- ❌ 失败常见原因：outbox `published_at` 为空（publisher）；副本未声明 ephemeral 队列（看日志 `announcement consumer subscribed (ephemeral)`）；WS 握手未带 user 身份。
