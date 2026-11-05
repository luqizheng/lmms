# iam-service 服务规范

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

配置加载遵循三段优先级：**env > yaml > (fail fast)**。合并后任一必需字段仍为空 → 服务启动失败并列出缺失字段。yaml 模板从二进制 `include_str!` 内嵌（`config/config.default.yaml`），首次启动时若 `CONFIG_PATH` 不存在则原子写入（`create_new` 抗 iam-service / iam-outbox-publisher 并发 race）。

### 直接读取的 env（非 yaml 覆盖）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| CONFIG_PATH | `/etc/iam-service/config.yaml` | yaml 目标路径；不存在则 seed 内嵌 default |
| SERVICE_BOOTSTRAP_TOKEN | 必填 | 服务间调用启动令牌 |
| TENANT_ID | 必填 | 租户 ID（i64） |
| TOKEN_SERVICE_URL | `http://token-service:8082` | token 服务地址 |
| JWKS_URL | `http://token-service:8082/.well-known/jwks.json` | JWKS 拉取地址 |
| IAM_PRIVATE_KEY_PEM_PATH | 空 | 可选：预留私钥路径（当前未使用） |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| RUST_LOG | info | 日志级别 |
| REDIS_URL | 服务自定义 | perm_epoch 直写（Phase 4.4；见下方 yaml 覆盖表也可写） |
| REDIS_PREFIX | superpowers: | perm_epoch key 前缀，**必须与 api-gateway `GW_REDIS_PREFIX` 一致** |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址（未在 Config struct，直接读取用于注册） |

### 覆盖 `config.yaml` 同名字段的 env

| 变量 | yaml 键 | yaml 默认值 |
|------|---------|-------------|
| SERVER_HOST | app.host | 0.0.0.0 |
| SERVER_PORT | app.port | 8081 |
| DATABASE_URL | database.url | 空（必须由 env 或 yaml 提供，否则 fail） |
| DATABASE_MAX_CONNECTIONS | database.max_connections | 20 |
| DATABASE_MIN_CONNECTIONS | database.min_connections | 5 |
| REDIS_URL | redis.url | 空（必须由 env 或 yaml 提供，否则 fail） |
| REDIS_TTL_SEC | redis.ttl_sec | 86400 |
| REDIS_PREFIX | redis.prefix | superpowers: |
| JWT_PRIVATE_KEY_PATH | jwt.private_key_path | /app/keys/private.pem |
| JWT_PUBLIC_KEY_PATH | jwt.public_key_path | /app/keys/public.pem |
| JWT_ACCESS_TOKEN_TTL_MINUTES | jwt.access_token_ttl_minutes | 15 |
| JWT_REFRESH_TOKEN_TTL_DAYS | jwt.refresh_token_ttl_days | 7 |
| CONSUL_HOST | consul.host | consul |
| CONSUL_PORT | consul.port | 8500 |
| CONSUL_SERVICE_NAME | consul.service_name | iam-service |
| IAM_SMTP_HOST | smtp.host | 空（关闭邮件） |
| IAM_SMTP_PORT | smtp.port | 1025（MailHog） |
| IAM_SMTP_USERNAME | smtp.username | 空（无认证） |
| IAM_SMTP_PASSWORD | smtp.password | 空 |
| IAM_SMTP_FROM | smtp.from | no-reply@ai-pm.local |
| IAM_SMTP_STARTTLS | smtp.starttls | false |
| IAM_PUBLIC_BASE_URL | public.base_url | http://127.0.0.1:8848 |

数字类 env 解析失败（如 `SERVER_PORT=abc`）会 warn 后保持 yaml 值；空字符串 / 全空白值等同"未设置"，触发 fail-fast 校验。

## 事件生产 / 令牌失效（Phase 4.4）

| 事件 | 触发 | 机制 |
|------|------|------|
| `iam.permission.changed` | assign_permissions / assign_user_roles / role 禁用·删除 / permission 禁用·删除（actor_id 来自 Principal.user_id） | 同事务展开受影响 `user_ids` + 写 `iam_outbox_events`（outbox）；commit 后直写 Redis `{prefix}perm_epoch:{user_id}=now`（best-effort，失败由 gateway consumer 兜底）|

- perm_epoch TTL = `jwt.access_token_ttl_minutes * 60`。gateway 比对 `token.iat < perm_epoch` → 401 强制重认证。
- iam Redis 连接为 `Option`：Redis 不可用时 iam 仍启动，直写降级为 no-op。

## 端点域

### 认证 /iam/auth/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/auth/login | POST | 用户登录 |
| /iam/auth/refresh | POST | 刷新 Token |
| /iam/auth/logout | POST | 用户登出 |
| /iam/auth/password | POST | 修改密码 |
| /iam/auth/sessions | GET | 获取会话列表 |
| /iam/auth/sessions/:id | DELETE | 撤销会话 |
| /iam/auth/forgot-password | POST | 发送重置密码邮件（公开，恒 204 防枚举）|
| /iam/auth/reset-password | POST | 用一次性令牌重置密码（公开，成功后撤销全部会话）|
| /iam/auth/invitations/accept | POST | 接受邀请并设置密码（公开，返回登录态）|

### 用户 /iam/users/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/users | GET | 查询用户列表 |
| /iam/users | POST | 创建用户（`password` 缺省时发邀请邮件）|
| /iam/users/me | GET | 当前登录用户资料 |
| /iam/users/me | PUT | 修改本人 `nickname` / `avatar`（邮箱手机号只读）|
| /iam/users/:id | GET | 获取用户详情 |
| /iam/users/:id | PUT | 更新用户 |
| /iam/users/:id | DELETE | 删除用户 |
| /iam/users/:id/roles | PUT | 全量替换用户角色，触发 iam.permission.changed（Phase 4.4-fu1）|

### 角色 /iam/roles/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/roles | GET | 查询角色列表 |
| /iam/roles | POST | 创建角色 |
| /iam/roles/:id | GET | 获取角色详情 |
| /iam/roles/:id | PUT | 更新角色 |
| /iam/roles/:id | DELETE | 删除角色 |
| /iam/roles/:id/permissions | PUT | 绑定权限 |
| /iam/roles/:id/users | GET | 按角色查用户（含完整信息，bearer_auth）|

### 权限 /iam/permissions/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/permissions | GET | 查询权限列表 |
| /iam/permissions | POST | 创建权限 |
| /iam/permissions/:id | GET | 获取权限详情 |
| /iam/permissions/:id | PUT | 更新权限 |
| /iam/permissions/:id | DELETE | 删除权限 |

### 组织单元 /iam/org-units/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/org-units | GET | 查询组织列表（树形） |
| /iam/org-units | POST | 创建组织 |
| /iam/org-units/:id | GET | 获取组织详情 |
| /iam/org-units/:id | PUT | 更新组织 |
| /iam/org-units/:id | DELETE | 删除组织 |

### 系统

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 数据库方言

支持 MySQL 和 PostgreSQL（`migrations/{mysql,postgres}/`，同一份业务 SQL 源码经 `db_*!` 宏按方言渲染；PG 迁移禁 `$$`/`DO`/触发器，时间列一律 `TIMESTAMPTZ`）。

## 表命名约定

所有表必须以 `iam_` 前缀（MySQL/PostgreSQL 共享 `superpowers` 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## 内部服务调用 /iam/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/internal/users/by-role/:role_id | GET | 按角色查 user_id（service-jwt only，tenant-isolated）|
| /iam/internal/users/by-dept/:dept_id | GET | 按部门查 user_id（service-jwt only，tenant-isolated）|
| /iam/internal/migrations/state | GET | schema 状态查询（service-jwt only，Phase 10 G3）|
| /internal/users/:user_id/claims | GET | 查用户 claims（token-service 调用；service-jwt `aud=iam-service`。2026-11-05 P0-3：从 users.rs 迁入本受保护路由组；`tenant_id` query 可选，缺省按用户行真实租户解析角色/权限）|

- service-jwt 必须有 `aud` 含 `service` + `tid` claim
- `page >= 1`, `size ∈ [1, 1000]`；超 1000 截断
- 用于 notification-service 等下游跨服务成员查询
- `/iam/internal/migrations/state` 返回 5 个 V001-V005 migration 的 marker-table 探测结果；鉴权 service-jwt `aud=iam-service`。详见 `docs-site/guides/migrations-state.md`。

## 豁免

（无）

## 状态说明

本服务整合了 token-service、credential-login-service、user-service 的功能。

## AI 检查时机

修改 services/iam-service/ 下文件时自动检查
