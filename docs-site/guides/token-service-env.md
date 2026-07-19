# token-service 环境变量

配置加载优先级：**代码默认值 ← config.yaml ← `TOKEN_*` 环境变量 ← docker-compose environment**

## 配置结构字段（`TOKEN_` 前缀覆盖）

通过 `common_config::Loader::new("TOKEN")` 加载，环境变量名 = 前缀 `TOKEN_` + 蛇形配置路径。

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 监听主机 | `TOKEN_SERVER_HOST` | `0.0.0.0` | `0.0.0.0` |
| 监听端口 | `TOKEN_SERVER_PORT` | `8082` | `4000` |
| 数据库类型 | `TOKEN_DATABASE_DB_TYPE` | `mysql` | `mysql` |
| 数据库连接 URL | `TOKEN_DATABASE_URL` | `${DATABASE_URL}` → `mysql://root:rootpass@mysql:3306/superpowers` | `mysql://token_service:token_pwd@localhost:3306/token_service` |
| 数据库最大连接数 | `TOKEN_DATABASE_MAX_CONNECTIONS` | `16` | `16` |
| Consul 地址 | `TOKEN_CONSUL_ADDR` | `http://consul:8500` | `http://localhost:8500` |
| Consul 服务名 | `TOKEN_CONSUL_SERVICE_NAME` | `token-service` | `token-service` |
| Consul 服务 ID 文件 | `TOKEN_CONSUL_SERVICE_ID_FILE` | `/tmp/token-service-id` | `/tmp/token-service-id` |
| JWT 私钥路径 | `TOKEN_JWT_PRIVATE_KEY_PATH` | `/app/keys/private.pem` | `keys/private.pem` |
| JWT 公钥路径 | `TOKEN_JWT_PUBLIC_KEY_PATH` | `/app/keys/public.pem` | `keys/public.pem` |
| JWT 签发者 | `TOKEN_ISSUER` | `superpowers-token-service` | `superpowers-token-service` |
| Access Token 有效期（秒） | `TOKEN_DEFAULT_ACCESS_TTL_SEC` | `3600` | `3600` |
| Refresh Token 有效期（秒） | `TOKEN_DEFAULT_REFRESH_TTL_SEC` | `2592000` | `2592000` |
| Resource Token 有效期（秒） | `TOKEN_DEFAULT_RESOURCE_TTL_SEC` | `300` | `300` |
| 用户服务地址 | `TOKEN_USER_SERVICE_URL` | `http://iam-service:8081` | `http://127.0.0.1:9081` |

## 非配置字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 默认值 |
|------|-----|-------------------|--------|
| 服务启动令牌（逗号分隔） | `SERVICE_BOOTSTRAP_TOKENS` | `${SERVICE_BOOTSTRAP_TOKENS}` | 空（拒绝所有调用方） |
| 租户 ID | `TENANT_ID` | `${TENANT_ID}` | — |
| 是否启用 Swagger UI | `SWAGGER_ENABLED` | `true` | `true` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `token-service` | `token-service` |
| 服务实例 ID | `SERVICE_ID` | `token-service` | `token-service` |
| 服务端口（注册探测） | `SERVICE_PORT` | `8082` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |
| 配置中心地址（可选） | `CONFIG_SERVICE_URL` | — | — |
| 配置中心命名空间（可选） | `CONFIG_SERVICE_NAMESPACE` | — | — |
| 日志级别 | `RUST_LOG` | `info` | `info` |

> **注意**：`SERVICE_NAME` 与 `TOKEN_CONSUL_SERVICE_NAME` 不是同一个设置。
> `SERVICE_NAME` 被遥测/日志库（`common_telemetry`）读取，作为服务通用标识；`TOKEN_CONSUL_SERVICE_NAME` 是 Consul 注册专用配置，由 `config.yaml` 的 `consul.service_name` 控制。两者值都是 `token-service`，但用途不同。