# prue_backend 环境变量

配置加载优先级：**代码默认值 ← 环境变量 ← docker-compose environment**

## 非配置字段（main.rs / config.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 数据库连接 URL | `DATABASE_URL` | `${DATABASE_URL}` | `sqlite::memory:` |
| 监听地址 | `SERVER_ADDR` | `0.0.0.0:8087`（由 `SERVER_HOST`:`SERVER_PORT` 拼接） | `0.0.0.0:8087` |
| 日志级别 | `RUST_LOG` | `info` | `info` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| Consul 地址 | `CONSUL_ADDR` | `http://consul:8500`（注释默认值） | `http://consul:8500` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | — | — |
| 服务名（通用标识） | `SERVICE_NAME` | `prue_backend` | `prue_backend` |
| 服务实例 ID | `SERVICE_ID` | `prue_backend` | `prue_backend` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8087` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 配置字段（config.rs `default_*` 函数可覆盖）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 监听主机 | `SERVER_HOST` | `0.0.0.0`（注释默认值） | `0.0.0.0` |
| 监听端口 | `SERVER_PORT` | `8087`（注释默认值） | `8080` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `false`（注释默认值） | `false` |

> **注意**：`SERVICE_BOOTSTRAP_TOKEN`、`TENANT_ID`、`TOKEN_SERVICE_URL` 在 prue_backend 代码中**未使用**，已从 docker-compose 移除。