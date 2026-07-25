# token-service 环境变量

配置加载优先级：**env（`TOKEN_` 前缀） > yaml > fail-fast（敏感字段清空 default 后由 validate 把关）**

## 启动流程

1. `common_config::Loader::new("TOKEN").with_file("config/config.yaml")`（`required(false)`，文件缺失不报错）
2. 合并 yaml + env 后调用 `validate_all_fields_present`：5 个敏感字段任一为空立即 panic
3. `Config::default` 把 `database.url` / `jwt_private_key_path` / `jwt_public_key_path` / `issuer` / `user_service_url` 全部留空，**不再有 dev fallback**——遗漏 env 不会静默错配

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

## 3. main.rs 直接读取（非 Loader）

| env | 默认 | 说明 |
|-----|------|------|
| `SERVICE_BOOTSTRAP_TOKENS` | 空 | 逗号分隔 bootstrap token，空则 `POST /tokens/service-jwt` 拒绝所有调用方 |
| `TENANT_ID` | `1` | 租户 ID |
| `SWAGGER_ENABLED` | `true` | Swagger UI 开关 |
| `CONFIG_SERVICE_URL` | 空 | 可选：远程配置中心地址 |
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
| yaml 不存在 | Loader `required(false)` → 仅靠 env + default → validate 把关敏感字段 |
| `TOKEN_SERVER_PORT` 写错 | Loader 反序列化报错，启动失败 |
