# notification-service 环境变量

配置加载优先级：**代码默认值 ← config.yaml ← 环境变量 ← docker-compose environment**

## 非配置字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 服务启动令牌 | `SERVICE_BOOTSTRAP_TOKEN` | `${NOTIFICATION_DEV_BT}` | **必填**（无默认值） |
| 租户 ID | `TENANT_ID` | `${TENANT_ID}` | `1`（解析失败时） |
| Token 服务地址 | `TOKEN_SERVICE_URL` | `${TOKEN_SERVICE_URL}` | `http://token-service:8082` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 事件总线 URL | `EVENT_BUS_URL` | `amqp://guest:guest@rabbitmq:5672/%2f`（先读 env，fallback 到 config.yaml） | `config.yaml: event_bus.url` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `true`（注释默认值） | `true` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `notification-service` | `notification-service` |
| 服务实例 ID | `SERVICE_ID` | `notification-service` | `notification-service` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8083` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 配置字段（config.yaml 读取）

以下字段由 `config.yaml` 文件加载，**不直接读取环境变量**：

| 中文 | config.yaml 路径 | docker-compose 值 | 代码默认值 |
|------|-----------------|-------------------|-----------|
| 监听地址 | `server.http_addr` | `0.0.0.0:8083`（bind mount config） | `0.0.0.0:8083` |
| MySQL 连接 URL | `mysql.url` | `${DATABASE_URL}`（bind mount config） | — |
| Redis 连接 URL | `redis.url` | `redis://redis:6379`（bind mount config） | — |
| Consul 地址 | `consul.addr` | `http://consul:8500`（bind mount config） | `http://localhost:8500` |

## ntf-outbox-publisher 特有环境变量

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 事件总线 URL | `EVENT_BUS_URL` | `amqp://guest:guest@rabbitmq:5672/%2f` | — |
| 目标 Exchange | `EVENT_BUS_EXCHANGE` | `events` | `events` |
| 轮询间隔（毫秒） | `OUTBOX_POLL_INTERVAL_MS` | `1000` | `1000` |
| 每批行数 | `OUTBOX_BATCH_SIZE` | `100` | `100` |