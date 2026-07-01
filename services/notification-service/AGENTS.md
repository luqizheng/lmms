# notification-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接 |
| REDIS_URL | 服务自定义 | Redis 连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| EVENT_BUS_URL | amqp://guest:guest@localhost:5672/%2f | RabbitMQ 连接（事件生产/消费） |
| EVENT_BUS_EXCHANGE | events | outbox publisher 目标 exchange（ntf-outbox-publisher） |
| OUTBOX_POLL_INTERVAL_MS | 1000 | ntf-outbox-publisher 轮询间隔 |
| OUTBOX_BATCH_SIZE | 100 | ntf-outbox-publisher 每批行数 |

## 二进制 / 事件生产消费（Phase 4.3）

| 组件 | 说明 |
|------|------|
| `notification-service`（主） | HTTP + WS + scheduler + 两个事件 consumer |
| `ntf-outbox-publisher`（同镜像切 CMD） | 轮询 `ntf_outbox_events` 发布到 `events` exchange |

| 事件 | 方向 | 机制 |
|------|------|------|
| `iam.user.created` | 消费 | 队列 `notification.iam-events` → 欢迎站内信（Phase 4.1） |
| `notification.announcement.published` | **生产** | `publish_announcement` 同事务写 `ntf_outbox_events` → ntf-outbox-publisher |
| `notification.announcement.published` | 消费 | **每副本 ephemeral 队列**（fanout）→ 本副本 WS 推送 |

`publish_announcement` 现在通过 `IamMemberClient` 调用 `/iam/internal/users/by-{role,dept}` 解析 role/dept scope 为具体 user_id 列表后写入 delivery 表。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /notification/announcements | POST | 新建公告 |
| /notification/announcements/:id/publish | POST | 发布 |
| /notification/announcements/:id/revoke | POST | 撤销 |
| /notification/announcements/:id | GET | 详情 |
| /notification/mine | GET | 个人通知列表 |
| /notification/mine/:id/read | POST | 标记已读 |
| /notification/mine/unread-count | GET | 未读数 |
| /notification/ws | GET (WS) | WebSocket 推送 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## 表命名约定

所有表必须以 `ntf_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/notification-service/ 下文件时自动检查
