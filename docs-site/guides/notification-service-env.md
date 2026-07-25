# notification-service 环境变量

配置加载优先级：**env > yaml（首启自动生成） > fail-fast（无 hardcode fallback）**

## 启动流程

1. 读 `CONFIG_PATH`（默认 `/etc/notification-service/config.yaml`）
2. 若文件不存在 → 用编译期内嵌的 `config/config.default.yaml` 原子写入
3. 读 yaml 反序列化到 `AppConfig` struct
4. 按表 2 的 env 名逐个覆盖字段
5. `validate_all_fields_present`：任一必需字段空 → 启动失败并列出缺失项

## 1. 非 yaml 字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认 |
|------|-----|-------------------|---------|
| 服务启动令牌 | `SERVICE_BOOTSTRAP_TOKEN` | `${NOTIFICATION_DEV_BT}` | **必填** |
| 租户 ID | `TENANT_ID` | `${TENANT_ID}` | `1`（解析失败时） |
| Token 服务地址 | `TOKEN_SERVICE_URL` | `${TOKEN_SERVICE_URL}` | `http://token-service:8082` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| yaml 路径 | `CONFIG_PATH` | `/etc/notification-service/config.yaml` | 同左 |
| Swagger UI 开关 | `SWAGGER_ENABLED` | `true` | `true` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名 | `SERVICE_NAME` | `notification-service` | `notification-service` |
| 服务实例 ID | `SERVICE_ID` | `notification-service` | `notification-service` |
| 注册探测端口 | `SERVICE_PORT` | `8083` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 2. 覆盖 yaml 的 env（yaml 缺失或留空时必填）

| env | yaml 键 | seed 模板默认 | 必需？ |
|-----|---------|--------------|--------|
| `DATABASE_URL` | `mysql.url` | `""` | **必填** |
| `REDIS_URL` | `redis.url` | `""` | **必填** |
| `SERVER_HTTP_ADDR` | `server.http_addr` | `0.0.0.0:8083` | 否 |
| `SERVER_PORT` | （只改 `server.http_addr` 的 port 部分） | `8083` | 否 |
| `DATABASE_MAX_CONNECTIONS` | `mysql.max_connections` | `10` | 否 |
| `SCHEDULER_TICK_SECS` | `scheduler.tick_secs` | `60` | 否 |
| `RUST_LOG` | `log.level` | `info` | 否 |
| `CONSUL_ADDR` | `consul.addr` | `http://consul:8500` | 否 |
| `CONSUL_SERVICE_NAME` | `consul.service_name` | `notification-service` | 否 |
| `CONSUL_SERVICE_ID_FILE` | `consul.service_id_file` | `/tmp/notification-service-id` | 否 |
| `EVENT_BUS_URL` | `event_bus.url` | `amqp://guest:guest@rabbitmq:5672/%2f` | 否 |
| `IAM_BASE_URL` | `iam_base_url` | `http://iam-service:8081` | 否 |

> `SERVER_PORT` 只重绑 port 部分，保留 host；跟 `SERVER_HTTP_ADDR` 同时存在时 env 覆盖顺序按代码内的 `apply_env_overrides`（HTTP_ADDR 先、PORT 后）。

## 3. ntf-outbox-publisher 独有 env

| env | 默认 | 说明 |
|-----|------|------|
| `EVENT_BUS_EXCHANGE` | `events` | outbox publisher 目标 exchange |
| `OUTBOX_POLL_INTERVAL_MS` | `1000` | 空轮询/失败重试退避 |
| `OUTBOX_BATCH_SIZE` | `100` | 单批 `claim_batch` 行数 |
| `DATABASE_URL` | **必填** | MySQL 连接串（跟主服务共享同一 yaml/env 语义） |

## 4. dev 与分发包两侧挂载

| 环境 | compose 挂载 | 说明 |
|------|-------------|------|
| dev | `../services/notification-service/config:/etc/notification-service`（无 `:ro`） | 首启 seed 落回宿主机（gitignore） |
| 分发包 | `./config/notification:/etc/notification-service` | 内聚 `infrastructure/config/notification/` |

## 5. 失败语义速查

| 场景 | 行为 |
|------|------|
| yaml 不存在 | seed 写入内嵌 default → 继续 |
| yaml 在但敏感字段空 + env 未设 | 启动失败：`config missing required fields after env merge: mysql.url, redis.url` |
| `SERVER_PORT` 非数字 | warn 保留 yaml 值 |
| 挂载 `:ro` 导致 seed 失败 | warn 后用内嵌 default 作 yaml 输入，继续 |
