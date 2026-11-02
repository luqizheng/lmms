# token-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

配置加载走 `digua_common_config::Loader`（`TOKEN_` 前缀）。合并 env + yaml 之后由 `validate_all_fields_present` 逐字段校验，任一 deployment-critical 字段为空即启动失败。

**必需字段（env 或 yaml 至少提供一份，否则 fail fast）：**

| 变量 | yaml 键 | 说明 |
|------|---------|------|
| TOKEN_DATABASE_URL | database.url | 数据库连接串（`mysql://` 或 `postgres://`，按 URL 前缀判方言） |
| TOKEN_JWT_PRIVATE_KEY_PATH | jwt_private_key_path | RS256 私钥文件路径 |
| TOKEN_JWT_PUBLIC_KEY_PATH | jwt_public_key_path | RS256 公钥文件路径 |
| TOKEN_ISSUER | issuer | JWT `iss` claim |
| TOKEN_USER_SERVICE_URL | user_service_url | iam-service 地址 |

**可选字段（有非敏感 `Default`）：**

| 变量 | 默认值 | 说明 |
|------|--------|------|
| TOKEN_SERVER_HOST | 0.0.0.0 | 监听主机 |
| TOKEN_SERVER_PORT | 4000 | 监听端口（compose 里显式设 8082） |
| TOKEN_DATABASE_MAX_CONNECTIONS | 16 | 连接池上限 |
| TOKEN_DATABASE_DB_TYPE | mysql | 目前仅支持 mysql |
| TOKEN_CONSUL_ADDR | http://localhost:8500 | Consul 地址（服务当前走 registrator sidecar 注册，此字段仅日志） |
| TOKEN_CONSUL_SERVICE_NAME | token-service | Consul 服务名 |
| TOKEN_CONSUL_SERVICE_ID_FILE | /tmp/token-service-id | Consul service id 文件 |
| TOKEN_DEFAULT_ACCESS_TTL_SEC | 3600 | access token 默认 TTL |
| TOKEN_DEFAULT_REFRESH_TTL_SEC | 2592000 | refresh token 默认 TTL |
| TOKEN_DEFAULT_RESOURCE_TTL_SEC | 300 | resource token 默认 TTL |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| RUST_LOG | info | 日志级别 |
| SERVICE_BOOTSTRAP_TOKENS | 空 | 各业务服务的逗号分隔 bootstrap token；未设置则 POST /tokens/service-jwt 拒绝所有调用方 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /tokens/access | POST | 签发 access_token（authorization_code 签发 access+refresh；client_credentials 只签 access、不签 refresh，RFC 6749 §4.4.3） |
| /tokens/refresh | POST | 刷新 token（refresh_token 单次使用，防重放） |
| /tokens/revoke | POST | 撤销 token（写入 Redis 黑名单） |
| /tokens/resource | POST | 签发受限资源令牌（自定义 TTL 与 scope） |
| /tokens/user-jwt | POST | 签发用户身份 JWT |
| /tokens/service-jwt | POST | 签发 1h 服务间 JWT（Authorization: Bearer <SERVICE_BOOTSTRAP_TOKENS> 准入） |
| /tokens/.well-known/jwks.json | GET | JWKS 公钥（kid="default"，RS256） |
| /.well-known/jwks.json | GET | JWKS 别名 |
| /api-keys | POST / GET | API Key 创建 / 列表（service-jwt `aud=token-service` 准入） |
| /admin/oauth-clients | POST / GET | OAuth 客户端创建（响应中一次性返回明文 client_secret；service-jwt 准入） |
| /admin/oauth-clients/{client_id} | PUT / DELETE | 更新 / 删除（service-jwt 准入） |
| /admin/oauth-clients/{client_id}/reset-secret | POST | 重置并返回新明文 client_secret（service-jwt 准入） |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（仅 SWAGGER_ENABLED=true 时暴露） |

## 内部端点 /token/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /token/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 1 个 V001 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=token-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs-site/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `token_` 前缀（MySQL/PostgreSQL 共享 `superpowers` 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/token-service/ 下文件时自动检查
