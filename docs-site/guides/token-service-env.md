# token-service 环境变量

配置加载优先级：**env（`TOKEN_` 前缀） > yaml > fail-fast（敏感字段清空 default 后由 validate 把关）**

## 启动流程

1. `config.rs::load_config` 读 `config/config.yaml`（缺失时降级为 `Config::default()`，敏感字段留空）
2. `apply_env_overrides` 逐字段用 `TOKEN_*` env 覆盖 yaml；`_` 一律视作字段名的一部分，不再当嵌套分隔符
3. `validate_all_fields_present` 检查 5 个敏感字段（database.url / 2 个 jwt 路径 / issuer / user_service_url）任一为空立即 panic

## 1. 必需字段（env 或 yaml 至少给一份，否则启动失败）

| env | yaml 键 | 说明 |
|-----|---------|------|
| `TOKEN_DATABASE_URL` | `database.url` | MySQL 连接串 |
| `TOKEN_JWT_PRIVATE_KEY_PATH` | `jwt_private_key_path` | RS256 私钥文件路径 |
| `TOKEN_JWT_PUBLIC_KEY_PATH` | `jwt_public_key_path` | RS256 公钥文件路径 |
| `TOKEN_ISSUER` | `issuer` | JWT `iss` claim |
| `TOKEN_USER_SERVICE_URL` | `user_service_url` | iam-service 地址 |

## 2. 非敏感 operational 默认

| env | 默认 | 说明 |
|-----|------|------|
| `TOKEN_SERVER_HOST` | `0.0.0.0` | 监听主机 |
| `TOKEN_SERVER_PORT` | `4000`（compose 显式设 `8082`） | 监听端口 |
| `TOKEN_DATABASE_DB_TYPE` | `mysql` | 目前仅支持 mysql |
| `TOKEN_DATABASE_MAX_CONNECTIONS` | `16` | 连接池上限 |
| `TOKEN_CONSUL_ADDR` | `http://localhost:8500` | 服务走 registrator sidecar 注册，此字段仅日志 |
| `TOKEN_CONSUL_SERVICE_NAME` | `token-service` | Consul 服务名 |
| `TOKEN_CONSUL_SERVICE_ID_FILE` | `/tmp/token-service-id` | Consul service id 文件 |
| `TOKEN_DEFAULT_ACCESS_TTL_SEC` | `3600` | access token 默认 TTL |
| `TOKEN_DEFAULT_REFRESH_TTL_SEC` | `2592000` | refresh token 默认 TTL |
| `TOKEN_DEFAULT_RESOURCE_TTL_SEC` | `300` | resource token 默认 TTL |

## 3. main.rs 直接读取（非 apply_env_overrides）

| env | 默认 | 说明 |
|-----|------|------|
| `SERVICE_BOOTSTRAP_TOKENS` | 空 | 逗号分隔 bootstrap token，空则 `POST /tokens/service-jwt` 拒绝所有调用方 |
| `TENANT_ID` | `1` | 租户 ID |
| `SWAGGER_ENABLED` | `true` | Swagger UI 开关 |
| `CONFIG_SERVICE_URL` | 空 | 可选：远程配置中心地址（fetch 失败降级为无 remote） |
| `CONFIG_SERVICE_NAMESPACE` | 空 | 可选：远程配置中心命名空间 |
| `RUST_LOG` | `info` | 日志级别 |

## 4. dev 与分发包两侧挂载

| 环境 | compose 挂载 | 说明 |
|------|-------------|------|
| dev | `../services/token-service/config:/app/config:ro` | **保留 `:ro`**（dephase 历史包袱，compose 行 191-193 注释）；源码 `services/token-service/config/config.yaml` 已清掉 mysql 密码 |
| 分发包 | `./config/token:/app/config:ro` | 内聚 `infrastructure/config/token/`；yaml 里 `database.url: ""` 靠 `TOKEN_DATABASE_URL` 补 |

## 5. 失败语义速查

| 场景 | 行为 |
|------|------|
| 5 个敏感字段任一为空 | panic：`token-service config missing required fields after env merge: ...` |
| yaml 不存在 | 降级为 `Config::default()` → 仅靠 env → validate 把关敏感字段 |
| `TOKEN_SERVER_PORT` 写错 | 数字解析失败 `warn` 后保持 yaml 值（不再 panic）|

## 6. 2026-07 变更：告别自动 env 绑定

此前使用 `digua_common_config::Loader`（内部 `config::Environment::separator("_")`）自动 env 绑定。
该行为把每个 `_` 都当嵌套分隔符，导致 `TOKEN_DATABASE_DB_TYPE` 被解析成
`database.db.type`，与 struct 字段 `database.db_type` 不匹配，运行时静默丢弃。
从此版起改为 `apply_env_overrides` 手写映射（与 iam-service / notification-service 一致），
env 名不再有拆分歧义。上表所有 env 名保持不变，兼容原 compose 配置。
