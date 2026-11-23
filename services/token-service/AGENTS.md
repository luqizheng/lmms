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
| TOKEN_JWT_KEYSTORE_PATH | jwt_keystore_path | 密钥库 JSON 路径；空 = 与 jwt_private_key_path 同目录的 `keystore.json`（2026-11-23 起为 JWT 签名密钥单一真相源） |
| TOKEN_JWT_MIN_RETENTION_SEC | 2592000 | 密钥 retired 后最短保留秒数（默认 30 天 = refresh TTL），到期才允许物理删除 |
| TOKEN_DEFAULT_ACCESS_TTL_SEC | 3600 | access token 默认 TTL |
| TOKEN_DEFAULT_REFRESH_TTL_SEC | 2592000 | refresh token 默认 TTL |
| TOKEN_DEFAULT_RESOURCE_TTL_SEC | 300 | resource token 默认 TTL |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| RUST_LOG | info | 日志级别 |
| SERVICE_BOOTSTRAP_TOKENS | 空 | 各业务服务的逗号分隔 bootstrap token。**可选**——与 OWNERS 都为空时回退到内置 dev 默认值（`models::bootstrap_token::DEFAULT_DEV_BOOTSTRAP`） |
| SERVICE_BOOTSTRAP_OWNERS | 空 | 与 SERVICE_BOOTSTRAP_TOKENS 等长、顺序一一对应的服务名。seed 时写入 `token_bootstrap_tokens.owner`，JWT 的 `sub` 即该值。**长度不一致或缺失 → token-service 拒绝启动**（fail-closed，不回退 dev 默认值） |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /tokens/access | POST | 签发 access_token（authorization_code 签发 access+refresh；client_credentials 只签 access、不签 refresh，RFC 6749 §4.4.3。**2026-11-05 P0-2：iam 不可达时两条授权流均 fail-closed**——authorization_code 签发空 permissions（不再兜底 admin 角色），client_credentials 同；对 iam 的 claims 调用改持自签 service-jwt `aud=iam-service`） |
| /tokens/refresh | POST | 刷新 token（refresh_token 单次使用，防重放） |
| /tokens/revoke | POST | 撤销 token（写入 Redis 黑名单） |
| /tokens/resource | POST | 签发受限资源令牌（自定义 TTL 与 scope） |
| /tokens/user-jwt | POST | 签发用户身份 JWT |
| /tokens/service-jwt | POST | 签发 1h 服务间 JWT（Authorization: Bearer <SERVICE_BOOTSTRAP_TOKENS> 准入） |
| /tokens/.well-known/jwks.json | GET | JWKS 公钥（active + retiring 全部密钥，各自 kid，RS256） |
| /.well-known/jwks.json | GET | JWKS 别名 |
| /api-keys | POST / GET | API Key 创建 / 列表（service-jwt `aud=token-service` 准入） |
| /admin/oauth-clients | POST / GET | OAuth 客户端创建（响应中一次性返回明文 client_secret；service-jwt 准入） |
| /admin/oauth-clients/{client_id} | PUT / DELETE | 更新 / 删除（service-jwt 准入） |
| /admin/oauth-clients/{client_id}/reset-secret | POST | 重置并返回新明文 client_secret（service-jwt 准入） |
| /admin/bootstrap-tokens | POST / GET | bootstrap token 新建（返回明文一次）/ 列表（**不含明文**，service-jwt 准入） |
| /admin/bootstrap-tokens/{owner} | GET / DELETE | 查单条元数据 / 删除（service-jwt 准入） |
| /admin/bootstrap-tokens/{owner}/rotate | POST | 换新 token（返回明文一次；`?grace_period_sec=N` 给旧 token 留宽限期，service-jwt 准入） |
| /admin/bootstrap-tokens/{owner}/status | PUT | 启用 / 禁用（service-jwt 准入） |
| /v1/admin/bootstrap-tokens | POST / GET | **Web UI 用**：新建 / 列表（user-jwt + 权限码 `TOKEN_BOOTSTRAP_CREATE` / `_READ`） |
| /v1/admin/bootstrap-tokens/{owner} | GET / DELETE | Web UI 用：查单条 / 删除（权限码 `_READ` / `_DELETE`） |
| /v1/admin/bootstrap-tokens/{owner}/rotate | POST | Web UI 用：换新 token（body `{"grace_period_sec":N}`，权限码 `_UPDATE`） |
| /v1/admin/bootstrap-tokens/{owner}/status | PUT | Web UI 用：启用 / 禁用（body `{"status":"active\|disabled"}`，权限码 `_UPDATE`） |
| /admin/keys | GET | JWT 签名密钥列表（kid/status/时间戳，**不含密钥材料**，service-jwt 准入） |
| /admin/keys/rotate | POST | 轮换签名密钥：新钥签发、旧钥转 retiring（仍在 JWKS，旧 token 继续可验证，无人掉线）。body 可选 `{"expect_active_kid": "..."}` 防双轮换竞态（不匹配 409） |
| /admin/keys/{kid}/retire | POST | retiring → retired（移出 JWKS；仅 retiring 状态可退役，service-jwt 准入） |
| /admin/keys/{kid} | DELETE | 删除 retired 密钥；须超过 `TOKEN_JWT_MIN_RETENTION_SEC` 保留期，否则 409（service-jwt 准入） |
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

## JWT 签名密钥生命周期（2026-11-23 起）

签名密钥单一真相源是 `keystore.json`（默认 `/app/keys/keystore.json`，compose 已挂
`token-keys` named volume）。要点：

- **fail-closed**：keystore 存在但损坏（JSON/PEM/公私钥不匹配/权限过宽）→ 拒绝启动，
  绝不静默重生成覆盖。旧 PEM 对存在时一次性导入（kid 保留 `"default"`，旧 token 不掉线）。
- **kid**：新钥 = SHA-256(公钥 DER) 前 16 字节 hex；导入旧钥 = `"default"`。
- **状态机**：active（签发）→ rotate → retiring（仅验证，仍在 JWKS）→ retire → retired
  （不在 JWKS）→ 超过 `TOKEN_JWT_MIN_RETENTION_SEC` 才可 DELETE。
- **原子写 + 权限**：`.tmp`+fsync+rename；文件 0600 / 目录 0700（unix 加载时校验）。
- 管理端点 `/admin/keys*`（service-jwt `aud=token-service`）；离线工具 `keytool`
  （inspect/rotate/retire/delete/backup/restore，容器内 `/app/keytool`）。
- 备份/恢复/轮换操作手册见 README「JWT 签名密钥」章节。
- 设计与 fail-closed 矩阵：`docs/superpowers/specs/2026-11-23-token-jwt-key-persistence-design.md`。

## Bootstrap token 存储（2026-11-09 起）

`POST /tokens/service-jwt` 的准入凭据持久化在 `token_bootstrap_tokens` 表（V003 迁移）：

| 列 | 说明 |
|---|---|
| `owner` | 主键。服务名（`iam-service` 等），JWT 的 `sub` 即此值 |
| `token_hash` | 当前存**明文**；列名预留以便后续切 SHA-256（只改写入路径，索引与查询 SQL 不变） |
| `status` | `active` / `disabled`；非 active 一律拒绝 |
| `expires_at` | 可空。NULL = 永久；非空且已过期则拒绝 |
| `last_used_at` | 最近命中时间（预留给运维审计） |

启动时自动 seed（`models/bootstrap_token.rs::seed_defaults`）：

- **来源优先级**：`SERVICE_BOOTSTRAP_TOKENS` + `SERVICE_BOOTSTRAP_OWNERS` 两个 env CSV → 内置 dev 默认值 `DEFAULT_DEV_BOOTSTRAP`（两个 env 都为空时才用）。
- **fail-closed**：env 提供了 tokens 但 owners 缺失 / 两列长度不一致 → **拒绝启动**（`sqlx::Error::Protocol`）。绝不回退到内置默认值——否则生产环境会静默用上硬编码 dev token。
- **语义**：按 `owner` 补插——表里没有该 owner 才 INSERT，已有的行不动。因此幂等（重复启动不重复插），也不会覆盖运维已 rotate 过的 token。
- 新增服务时的完整步骤见 `docs-site/guides/service-to-service-auth.md`「新增一个服务时怎么加 token」。

## AI 检查时机

修改 services/token-service/ 下文件时自动检查
