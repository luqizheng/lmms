# iam-service 环境变量

配置加载优先级：**代码默认值 ← config.yaml ← 环境变量 ← docker-compose environment**

## 非配置字段（main.rs / config.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 服务启动令牌 | `SERVICE_BOOTSTRAP_TOKEN` | `${IAM_DEV_BT}` | **必填**（无默认值） |
| 租户 ID | `TENANT_ID` | `${TENANT_ID}` | `1`（解析失败时） |
| Token 服务地址 | `TOKEN_SERVICE_URL` | `${TOKEN_SERVICE_URL}` | `http://token-service:8082` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 私钥 PEM 文件路径 | `IAM_PRIVATE_KEY_PEM_PATH` | —（可选） | 空字符串（当前未使用） |
| 数据库连接 URL | `DATABASE_URL` | `${DATABASE_URL}` | `config.yaml` 中读取 |
| 配置路径 | `CONFIG_PATH` | `/etc/iam-service/config.yaml` | `config/config.yaml` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `iam-service` | `iam-service` |
| 服务实例 ID | `SERVICE_ID` | `iam-service` | `iam-service` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8081` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 配置字段（config.yaml 读取，环境变量可覆盖 `default_*` 函数）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 监听主机 | `SERVER_HOST` | `0.0.0.0`（注释默认值） | `0.0.0.0` |
| 监听端口 | `SERVER_PORT` | `8081`（注释默认值） | `8080` |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `true`（注释默认值，当前因 SIGSEGV 禁用） | `true` |