# api-gateway 环境变量

配置加载优先级：**代码默认值 ← config/api-gateway.toml ← `GW_*` 环境变量 ← docker-compose environment**

## `GW_` 前缀字段（config crate 加载，`__` 分隔嵌套）

通过 `config::Environment::with_prefix("GW").prefix_separator("_").separator("__")` 加载。

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 监听主机 | `GW_SERVER__HOST` | `0.0.0.0` | `0.0.0.0` |
| 监听端口 | `GW_SERVER__PORT` | `3000` | `3000` |
| Redis 连接 URL | `GW_REDIS__URL` | `redis://redis:6379` | `redis://127.0.0.1:6379` |
| Redis key 前缀 | `GW_REDIS__PREFIX` | `superpowers:` | `superpowers:` |
| Consul 地址 | `GW_CONSUL_ADDR` | `http://consul:8500` | `http://127.0.0.1:8500` |
| JWKS 地址 | `GW_JWKS_URL` | `http://token-service:8082/.well-known/jwks.json` | `http://127.0.0.1:4000/.well-known/jwks.json` |
| JWT 签发者 | `GW_ISSUER` | `superpowers-token-service` | `superpowers-token-service` |
| 路由目录 | `GW_ROUTES_DIR` | `/app/config/api-gateway/routes` | `/app/config/routes` |
| 事件总线 URL | `GW_EVENT_BUS_URL` | `amqp://guest:guest@rabbitmq:5672/%2f` | `amqp://guest:guest@localhost:5672/%2f` |
| perm_epoch TTL（秒） | `GW_PERM_EPOCH_TTL_SEC` | `900`（注释默认值） | `900` |

## 非配置字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认值 |
|------|-----|-------------------|-----------|
| 日志级别 | `RUST_LOG` | `api_gateway=trace,info` | `info` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名（通用标识） | `SERVICE_NAME` | `api-gateway` | `api-gateway` |
| 服务实例 ID | `SERVICE_ID` | `api-gateway` | `api-gateway` |
| 服务端口（注册探测） | `SERVICE_PORT` | `3000` | `3000` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

> **注意**：`SERVICE_BOOTSTRAP_TOKEN`、`TENANT_ID`、`TOKEN_SERVICE_URL`、`JWKS_URL` 在 api-gateway 代码中**未使用**（使用 `GW_*` 前缀替代），已从 docker-compose 移除。
>
> `GW_ROUTES_DIR` 环境变量可在运行时覆盖 CLI `--routes` 参数，方便 k8s ConfigMap 等场景无需重建镜像。