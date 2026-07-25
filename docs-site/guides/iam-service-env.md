# iam-service 环境变量

配置加载优先级：**env > yaml（首启自动生成） > fail-fast（无 hardcode fallback）**

## 启动流程

1. 读 `CONFIG_PATH`（默认 `/etc/iam-service/config.yaml`）
2. 若文件不存在 → 用编译期内嵌的 `config/config.default.yaml` 原子写入（`create_new` race-safe，与 iam-outbox-publisher 竞争只会有一个赢）
3. 读 yaml 反序列化到 `Config` struct
4. 按表 2 的 env 名逐个覆盖字段（数字解析失败 warn 后保留 yaml 值）
5. `validate_all_fields_present`：任一必需字段空 → 启动失败并列出缺失项

## 1. 非 yaml 字段（main.rs 直接读取）

| 中文 | env | docker-compose 值 | 代码默认 |
|------|-----|-------------------|---------|
| 服务启动令牌 | `SERVICE_BOOTSTRAP_TOKEN` | `${IAM_DEV_BT}` | **必填** |
| 租户 ID | `TENANT_ID` | `${TENANT_ID}` | `1`（解析失败时） |
| Token 服务地址 | `TOKEN_SERVICE_URL` | `${TOKEN_SERVICE_URL}` | `http://token-service:8082` |
| JWKS 地址 | `JWKS_URL` | `${JWKS_URL}` | `http://token-service:8082/.well-known/jwks.json` |
| 私钥 PEM 路径 | `IAM_PRIVATE_KEY_PEM_PATH` | —（可选） | 空（当前未用） |
| yaml 路径 | `CONFIG_PATH` | `/etc/iam-service/config.yaml` | 同左 |
| Swagger UI 开关 | `SWAGGER_ENABLED` | `true`（当前因 SIGSEGV 禁用，见 main.rs TODO） | `true` |
| OpenTelemetry 端点 | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://otel-collector:4317` | — |
| 服务名 | `SERVICE_NAME` | `iam-service` | `iam-service` |
| 服务实例 ID | `SERVICE_ID` | `iam-service` | `iam-service` |
| 注册探测端口 | `SERVICE_PORT` | `8081` | `8080` |
| 健康检查路径 | `SERVICE_HEALTH_PATH` | `/health` | `/health` |

## 2. 覆盖 yaml 的 env（yaml 缺失或留空时必填）

yaml 由首启 seed 自 `services/iam-service/config/config.default.yaml`，合并后任何空白字段会触发启动失败。

| env | yaml 键 | seed 模板默认 | 必需？ |
|-----|---------|--------------|--------|
| `DATABASE_URL` | `database.url` | `""` | **必填** |
| `REDIS_URL` | `redis.url` | `""` | **必填** |
| `SERVER_HOST` | `app.host` | `0.0.0.0` | 否 |
| `SERVER_PORT` | `app.port` | `8081` | 否 |
| `DATABASE_MAX_CONNECTIONS` | `database.max_connections` | `20` | 否 |
| `DATABASE_MIN_CONNECTIONS` | `database.min_connections` | `5` | 否 |
| `REDIS_TTL_SEC` | `redis.ttl_sec` | `86400` | 否 |
| `REDIS_PREFIX` | `redis.prefix` | `superpowers:` | 否 |
| `JWT_PRIVATE_KEY_PATH` | `jwt.private_key_path` | `/app/keys/private.pem` | 否 |
| `JWT_PUBLIC_KEY_PATH` | `jwt.public_key_path` | `/app/keys/public.pem` | 否 |
| `JWT_ACCESS_TOKEN_TTL_MINUTES` | `jwt.access_token_ttl_minutes` | `15` | 否 |
| `JWT_REFRESH_TOKEN_TTL_DAYS` | `jwt.refresh_token_ttl_days` | `7` | 否 |
| `CONSUL_HOST` | `consul.host` | `consul` | 否 |
| `CONSUL_PORT` | `consul.port` | `8500` | 否 |
| `CONSUL_SERVICE_NAME` | `consul.service_name` | `iam-service` | 否 |

> `REDIS_URL` 是敏感字段（perm_epoch 直写 Redis），也跟 `database.url` 一样**必须由 env 或 yaml 提供**，yaml 模板里留空。其它字段都有非敏感 operational 默认值。

## 3. dev 与分发包两侧挂载

| 环境 | compose 挂载 | 说明 |
|------|-------------|------|
| dev (`infrastructure/docker-compose-services.yml`) | `../services/iam-service/config:/etc/iam-service`（无 `:ro`） | 首启把 seed 后的 yaml 落回宿主机 `services/iam-service/config/config.yaml`（已 gitignore） |
| 分发包（release.sh sed 后） | `./config/iam:/etc/iam-service` | 内聚到 `infrastructure/config/iam/`；运维可编辑，env 仍最上层 |

改 yaml 重启容器立即生效（env 依然覆盖之）；要重新走 seed 删掉 yaml 即可。

## 4. 失败语义速查

| 场景 | 行为 |
|------|------|
| yaml 不存在 | seed 写入内嵌 default → 继续 |
| yaml 在但敏感字段空 + env 未设 | 启动失败：`config missing required fields after env merge: database.url, redis.url` |
| 数字 env 写错（如 `SERVER_PORT=abc`） | warn 保留 yaml 值，不 crash |
| `:ro` 挂载导致 seed 写不进 | warn 后用内嵌 default 作 yaml 输入，继续走 env 覆盖 |
| iam-outbox-publisher 与 iam-service 并发启动 | 仅一个 seed 成功（`create_new`），另一个正常读 |
