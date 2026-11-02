# audit-service 服务规范

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

配置走 `AppConfig::from_env()` 纯 env 模式（无 yaml 文件）。**`DATABASE_URL` 必填**（无 fallback，缺失或空白启动 fail-fast）。其它变量有 operational default。

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | **必填** | 数据库连接串（`mysql://` 或 `postgres://`，按 URL 前缀判方言）；缺失或空白启动 fail-fast |
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| EVENT_BUS_URL | amqp://guest:guest@localhost:5672/%2f | RabbitMQ 连接（事件消费） |
| REDIS_URL | redis://127.0.0.1:6379 | 风控滑动窗口计数存储（best-effort，连接失败仅 warn 不阻塞主流程） |
| LOGIN_RISK_THRESHOLD | 5 | 窗口内登录失败次数告警阈值 |
| LOGIN_RISK_WINDOW_SECS | 300 | 风控滑动窗口秒数 |

## 事件消费（Phase 4 / 4.2）

| 队列 | 绑定 routing-key | 用途 |
|------|------------------|------|
| audit.iam-events | iam.# | 全部 iam 事件落库 `audit_events`（含 `iam.login.failed`） |
| audit.login-risk | iam.login.failed | 风控：按 username/IP 滑动窗口计数，越线告警 `login_risk_alerts_total{dimension}` |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/operation-logs | GET / POST | 操作审计 |
| /audit/login-logs | GET / POST | 登录审计 |
| /audit/compliance/* | GET | 合规导出 |
| /audit/retention/* | GET / PUT | 留存策略 |
| /audit/health | GET | 业务健康 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 内部端点 /audit/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /audit/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 1 个 V001 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=audit-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `audit_` 前缀（MySQL/PostgreSQL 共享 `superpowers` 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/audit-service/ 下文件时自动检查
