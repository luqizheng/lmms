# 通知服务（notification-service）设计

> 设计日期: 2026-06-10
> 状态: Draft → 待审核
> 关联: `2025-07-11-token-server-api-gateway-design.md`、`2026-06-10-microservice-jwt-verification-design.md`

---

## 1. 概述

为 coder-micro-services 提供系统内消息能力：登录后弹窗 / 顶部公告栏；支持定时发布、置顶、已读未读。

**关键决策**
- 接收范围：按 **user / role / dept** 三种 scope 定向，可混用
- 服务归属：新建独立 `services/notification-service`（Rust + Axum）
- 存储：MySQL 主存 + Redis 缓存未读计数
- 推送：WebSocket（Axum `axum::extract::ws`）
- 定时：服务内置 tokio interval(60s) 扫描调度表

---

## 2. 总体架构

```
   浏览器 / 客户端
     │  HTTPS REST
     │  WSS  /ws
     ▼
  api-gateway (Rust)              ← 现有入口
     │  验签 + 注入 X-User-Id / X-User-Roles / X-User-Dept
     ▼
  notification-service (Rust)     ← 新增
     │  ┌──────────┐ ┌──────────┐ ┌──────────────┐
     │  │ REST API │ │ WS Hub   │ │ Scheduler    │
     │  └──────────┘ └──────────┘ └──────────────┘
     │       │             │              │
     │       └──────┬──────┴──────────────┘
     │              │
     │         ┌────▼────┐        ┌────────┐
     │         │  MySQL  │        │ Redis  │
     │         │  主存   │        │ 未读   │
     │         └─────────┘        └────────┘
     │
     │  scope 展开时调用 user-service
     └──→ user-service (HTTP, internal)
```

端口 `:8083`，Consul 注册名 `notification-service`。

---

## 3. 数据模型

复用 `mysql:8.0` 容器，DB 名 `notification`。

```sql
-- 消息主表
announcements (
  id              BIGINT UNSIGNED PK AUTO_INCREMENT,
  title           VARCHAR(200)  NOT NULL,
  body            MEDIUMTEXT    NOT NULL,
  publish_at      DATETIME      NOT NULL,
  expire_at       DATETIME      NULL,
  pinned          TINYINT(1)    NOT NULL DEFAULT 0,
  popover         TINYINT(1)    NOT NULL DEFAULT 0,
  status          ENUM('draft','scheduled','published','revoked') NOT NULL,
  created_by      BIGINT        NOT NULL,
  created_at      DATETIME      NOT NULL,
  KEY idx_status_publish (status, publish_at)
)

-- 接收范围（原始 scope，可多行多 scope_type）
announcement_recipients (
  announcement_id BIGINT       NOT NULL,
  scope_type      ENUM('user','role','dept') NOT NULL,
  scope_id        BIGINT       NOT NULL,
  PRIMARY KEY (announcement_id, scope_type, scope_id)
)

-- 展开后的 user 列表（发布时由 recipient_resolver 写入）
announcement_deliveries (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  delivered_at    DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user (user_id, announcement_id)
)

-- 已读
announcement_reads (
  announcement_id BIGINT  NOT NULL,
  user_id         BIGINT  NOT NULL,
  read_at         DATETIME NOT NULL,
  PRIMARY KEY (announcement_id, user_id),
  KEY idx_user_unread (user_id, read_at)
)
```

**Redis 键**
- `notif:unread:{user_id}` → INTEGER，未读数（≥0）
- `notif:ws:{user_id}` → SET，活跃 WS 连接 ID（用于广播去重）

---

## 4. 接口

路径前缀 `/api/notifications`，全部经 api-gateway 转发；网关已注入 `X-User-*` 头。

| Method | Path | 角色 | 说明 |
|---|---|---|---|
| POST | `/announcements` | admin | 创建（status=draft 或 scheduled） |
| GET | `/announcements/{id}` | any | 详情（仅当 user 在 deliveries 内） |
| GET | `/announcements?pinned=&active=` | any | 当前用户可见列表（pinned 优先，按 publish_at desc） |
| POST | `/announcements/{id}/publish` | admin | 立即发布（status=published） |
| POST | `/announcements/{id}/revoke` | admin | 撤销（status=revoked） |
| POST | `/announcements/{id}/read` | any | 标记已读（幂等） |
| GET | `/unread-count` | any | 未读数（Redis 直读） |

**管理员判定**：调用 `user-service` 的 `/internal/permissions/check`（service-token 模式），或复用现有 `users.roles.permissions` 表本地 join（v1 简化：写操作要求 `X-User-Roles` 含 `notification.admin`）。

---

## 5. WebSocket 协议

`wss://{gateway}/api/notifications/ws?token={access_token}`

**鉴权**：gateway 侧 WS 路由配置 strip prefix + 添加头部透传 query token；notification-service 收到后向 token-server 拉取 `/oauth2/introspect` 校验，提取 `sub`（user_id）、`roles`、`dept_id`。

**服务端 → 客户端**
```json
{ "type": "new", "payload": { "id": 1001, "title": "...", "pinned": true, "popover": true } }
{ "type": "unread_count", "payload": { "count": 3 } }
{ "type": "revoked", "payload": { "id": 1001 } }
{ "type": "pong" }
```

**客户端 → 服务端**
```json
{ "type": "ping" }
```

**心跳**：客户端 30s ping，服务端 60s 未收 ping 关闭。

**关闭码**：
- 4001 token 失效
- 4003 禁止访问
- 1011 内部错误

---

## 6. 关键流程

### 6.1 创建并发布
1. admin POST `/announcements` body 包含 `recipients: [{scope_type, scope_id}]`
2. 写 `announcements`（status=scheduled 或 draft）
3. 写 `announcement_recipients`
4. 若 `status=published` 立即触发 6.3；否则等 scheduler

### 6.2 定时发布（Scheduler）
- tokio interval 60s
- 查 `announcements` where `status=scheduled AND publish_at <= NOW()`
- 逐条调用 6.3，更新 `status=published`

### 6.3 展开与投递
1. `recipient_resolver` 拉取 `announcement_recipients`
2. 调 user-service 内部接口展开：
   - `scope_type=user` → `users` 表
   - `scope_type=role` → `users.roles` join
   - `scope_type=dept` → `users.dept_id`
   - 三者并集去重
3. 写 `announcement_deliveries`
4. Redis：`INCRBY notif:unread:{uid} 1`（pipeline 批量）
5. WS 推 `new` + `unread_count`（仅在线用户）

### 6.4 标记已读
1. `INSERT IGNORE INTO announcement_reads`
2. 若实际写入（affected=1）：`DECR notif:unread:{uid}`，边界保护不小于 0
3. WS 推 `unread_count`（仅当前连接）

### 6.5 撤销
1. `UPDATE announcements SET status='revoked'`
2. 广播 `{type:"revoked"}` 给所有 deliveries 内在线用户

---

## 7. 错误处理

| 场景 | 行为 |
|---|---|
| token 失效 / 过期 | WS 关闭 4001；REST 401 |
| 非 admin 调写接口 | 403 |
| 已读重复 | 幂等返回 200 |
| Redis 不可用 | 未读数从 MySQL 兜底统计（慢路径） |
| MySQL 不可用 | 5xx + 日志告警；WS 心跳正常但投递延迟 |
| 撤销后查询 | 列表过滤 `status=published` |
| 过期消息 | `expire_at < NOW()` 视为不再展示，但保留 deliveries/reads |

---

## 8. 测试

| 层 | 覆盖 |
|---|---|
| 单元 | `recipient_resolver` 三种 scope 展开与去重；未读计数边界（DECR 不到 0） |
| 集成（in-process） | scheduler 命中、幂等已读、撤销后过滤、pinned 排序 |
| 集成（HTTP） | REST 全路径、admin 鉴权 |
| E2E | gateway → notification WS：发布 → 收到 `new` → 标记已读 → 收到 `unread_count` |
| 规格 | `tests/specs/notification-service/announcement.yaml`（YAML 行为规格） |

测试运行复用 `scripts/test-orchestrator.ps1`，新增 `-Service notification-service`。

---

## 9. 部署与配置

**Cargo.toml 依赖**
- `axum` 0.7 + `tower` + `tower-http`
- `tokio` 1（full）
- `sqlx` 0.8（mysql / runtime-tokio-rustls）
- `redis` 0.27（tokio-comp）
- `serde` / `serde_json`
- `tracing` / `tracing-subscriber`
- 与 api-gateway / token-server 保持一致

**config.toml**
```toml
[server]
http_addr = "0.0.0.0:8083"

[mysql]
url = "mysql://notif:notif@mysql:3306/notification"

[redis]
url = "redis://redis:6379"

[consul]
addr = "consul:8500"
service_name = "notification-service"

[user_service]
base_url = "http://user-service:8080"
service_token = "${NOTIF_SERVICE_TOKEN}"

[scheduler]
tick_secs = 60
```

**目录结构**
```
services/notification-service/
├── Cargo.toml
├── Dockerfile
├── config/
│   └── config.yaml
├── migrations/
│   └── 2026_06_10_init.sql
└── src/
    ├── main.rs
    ├── config.rs
    ├── db.rs
    ├── redis.rs
    ├── models.rs
    ├── rest.rs
    ├── ws.rs
    ├── ws_hub.rs
    ├── scheduler.rs
    ├── recipient_resolver.rs
    └── auth.rs
```

**路由配置**（api-gateway 新增 `notification.router.toml`，前缀 `/api/notifications`，WS 单独子段 `route_upgrade`）。

---

## 10. 后续可演进（不在本次范围）

- 消息模板（变量替换、Markdown 渲染）
- 邮件 / 短信多通道
- 用户级免打扰时段
- 部门 / 租户级配置
- 历史消息搜索（ES 索引）
