# config-service 环境变量

配置加载优先级：**代码默认值 ← 环境变量 ← docker-compose environment**

**`DATABASE_URL` 必填**（本轮起无 dev-DB fallback；缺失或空白时启动 fail-fast 报 `DATABASE_URL required — config-service has no fallback`）。

## 非配置字段（main.rs / config.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 数据库连接 URL | `DATABASE_URL` | `${DATABASE_URL}` | **必填**（无 fallback） |
| 监听地址 | `SERVER_ADDR` | `0.0.0.0:8081` | `0.0.0.0:8081` |
| 日志级别 | `RUST_LOG` | `info` | `info` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `false`（注释默认值） | `false` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `config-service` | `config-service` |

> **注意**：`SERVICE_BOOTSTRAP_TOKEN`、`TENANT_ID`、`TOKEN_SERVICE_URL` 在 config-service 代码中**未使用**，已从 docker-compose 移除。