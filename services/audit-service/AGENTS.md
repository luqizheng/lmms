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

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接串 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| EVENT_BUS_URL | amqp://guest:guest@localhost:5672/%2f | RabbitMQ 连接（事件消费） |
| REDIS_URL | redis://127.0.0.1:6379 | 风控滑动窗口计数存储 |
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

## 豁免

（无）

## 表命名约定

所有表必须以 `audit_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/audit-service/ 下文件时自动检查
