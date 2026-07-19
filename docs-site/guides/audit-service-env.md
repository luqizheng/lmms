# audit-service 环境变量

配置加载优先级：**代码默认值 ← 环境变量 ← docker-compose environment**

## 非配置字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 数据库连接 URL | `DATABASE_URL` | `${DATABASE_URL}` | `mysql://root:rootpass@127.0.0.1:3306/superpowers` |
| 监听地址 | `SERVER_ADDR` | `0.0.0.0:8084`（由 `SERVER_HOST`:`SERVER_PORT` 拼接） | `0.0.0.0:8084` |
| 日志级别 | `RUST_LOG` | `info` | `info` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 事件总线 URL | `EVENT_BUS_URL` | `amqp://guest:guest@rabbitmq:5672/%2f` | `amqp://guest:guest@localhost:5672/%2f` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `false`（注释默认值） | `false` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `audit-service` | `audit-service` |
| 服务实例 ID | `SERVICE_ID` | `audit-service` | `audit-service` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8084` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 配置字段（config.rs `default_*` 函数可覆盖）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 监听主机 | `SERVER_HOST` | `0.0.0.0`（注释默认值） | `0.0.0.0` |
| 监听端口 | `SERVER_PORT` | `8084`（注释默认值） | `8080` |
| Consul 地址 | `CONSUL_ADDR` | `http://consul:8500`（注释默认值） | `http://127.0.0.1:8500` |
| Consul 服务名 | `CONSUL_SERVICE_NAME` | `audit-service`（注释默认值） | `audit-service` |
| Redis 连接 URL | `REDIS_URL` | `redis://${REDIS_HOST}:${REDIS_PORT}` | `redis://127.0.0.1:6379` |
| 风控：登录失败告警阈值 | `LOGIN_RISK_THRESHOLD` | `5`（注释默认值） | `5` |
| 风控：滑动窗口秒数 | `LOGIN_RISK_WINDOW_SECS` | `300`（注释默认值） | `300` |

> **注意**：`SERVICE_BOOTSTRAP_TOKEN`、`TENANT_ID`、`TOKEN_SERVICE_URL` 在 audit-service 代码中**未使用**，已从 docker-compose 移除。