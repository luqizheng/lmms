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

配置加载遵循三段优先级：**env > yaml > (fail fast)**。合并后任一必需字段仍为空 → 服务启动失败并列出缺失字段。yaml 模板从二进制 `include_str!` 内嵌（`config/config.default.yaml`），首次启动时若 `CONFIG_PATH` 不存在则原子写入（`create_new` 抗并发 race）。

### 直接读取的 env（非 yaml 覆盖）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| CONFIG_PATH | `/etc/notification-service/config.yaml` | yaml 目标路径；不存在则 seed 内嵌 default |
| SERVICE_BOOTSTRAP_TOKEN | 必填 | 服务间调用启动令牌 |
| TENANT_ID | 必填（i64） | 租户 ID |
| TOKEN_SERVICE_URL | `http://token-service:8082` | token 服务地址 |
| JWKS_URL | `http://token-service:8082/.well-known/jwks.json` | JWKS 拉取地址 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| RUST_LOG | info | 日志级别 |

### 覆盖 `config.yaml` 同名字段的 env

| 变量 | yaml 键 | yaml 默认值 |
|------|---------|-------------|
| SERVER_HTTP_ADDR | server.http_addr | 0.0.0.0:8083 |
| SERVER_PORT | server.http_addr 的 port 部分 | 8083（保留 host，只重绑 port） |
| DATABASE_URL | mysql.url | 空（必须由 env 或 yaml 提供，否则 fail） |
| DATABASE_MAX_CONNECTIONS | mysql.max_connections | 10 |
| REDIS_URL | redis.url | 空（必须由 env 或 yaml 提供，否则 fail） |
| SCHEDULER_TICK_SECS | scheduler.tick_secs | 60 |
| CONSUL_ADDR | consul.addr | http://consul:8500 |
| CONSUL_SERVICE_NAME | consul.service_name | notification-service |
| CONSUL_SERVICE_ID_FILE | consul.service_id_file | /tmp/notification-service-id |
| EVENT_BUS_URL | event_bus.url | amqp://guest:guest@rabbitmq:5672/%2f |
| EVENT_BUS_EXCHANGE | （outbox publisher 独有）events | ntf-outbox-publisher 目标 exchange |
| OUTBOX_POLL_INTERVAL_MS | （outbox publisher 独有）1000 | ntf-outbox-publisher 轮询间隔 |
| OUTBOX_BATCH_SIZE | （outbox publisher 独有）100 | ntf-outbox-publisher 每批行数 |
| IAM_BASE_URL | iam_base_url | http://iam-service:8081 |

数字类 env 解析失败（如 `SERVER_PORT=abc`）会 warn 后保持 yaml 值；空字符串 / 全空白等同"未设置"。

## 二进制 / 事件生产消费（Phase 4.3）

| 组件 | 说明 |
|------|------|
| `notification-service`（主） | HTTP + WS + scheduler + 三个事件 consumer |
| `ntf-outbox-publisher`（同镜像切 CMD） | 轮询 `ntf_outbox_events` 发布到 `events` exchange |
 
| 事件 | 方向 | 机制 |
|------|------|------|
| `iam.user.created` | 消费 | 队列 `notification.iam-events` → 欢迎站内信（Phase 4.1） |
| `iam.user.role.assigned` / `iam.user.role.unassigned` / `iam.user.dept.changed` | 消费 | 队列 `notification.membership-events`（durable, DLX=events.dlx）→ 更新本地 `ntf_user_projection`（Phase 5B-fu3） |
| `notification.announcement.published` | **生产** | `publish_announcement` 同事务写 `ntf_outbox_events` → ntf-outbox-publisher |
| `notification.announcement.published` | 消费 | **每副本 ephemeral 队列**（fanout）→ 本副本 WS 推送 |
 
## Phase 5B-fu3 — 本地 user membership 投影

- 启动时一次性 `GET /iam/internal/membership-bootstrap`（service-jwt aud=iam-service,
  tenant_id 来自 JWT tid claim）拉全量 user→{role_ids, dept_id} 写入
  `ntf_user_projection`。失败记 error log + 继续（consumer 后续会按事件增量填）。
- 持续消费 `notification.membership-events` 维护投影（apply_role_assigned/
  unassigned/apply_dept_changed 三方法，ts 单调非减 out-of-order 兜底）。
- `publish_announcement` 解析 role/dept scope 改走 `db.resolve_users_by_scopes`
  本地 SQL（`JSON_CONTAINS` by role / `WHERE dept_id=?` by dept）。
- `IamMemberClient::{by_role, by_dept, expand_scopes}` **DEPRECATED**
  （#[deprecated] 注解 + warn log；删除在 Roadmap 2.0 清理 phase）。
  CrossTenant（platform admin）路径仍走旧 HTTP（本地投影只覆盖当前 tenant）。

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

## 内部端点 /ntf/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /ntf/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 3 个 V001-V003 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=notification-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `ntf_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/notification-service/ 下文件时自动检查
