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

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| REDIS_URL | 服务自定义 | perm_epoch 直写（Phase 4.4）|
| REDIS_PREFIX | superpowers: | perm_epoch key 前缀，**必须与 api-gateway `GW_REDIS__PREFIX` 一致** |

## 事件生产 / 令牌失效（Phase 4.4）

| 事件 | 触发 | 机制 |
|------|------|------|
| `iam.permission.changed` | assign_permissions / role 禁用·删除 / permission 禁用·删除 | 同事务展开受影响 `user_ids` + 写 `iam_outbox_events`（outbox）；commit 后直写 Redis `{prefix}perm_epoch:{user_id}=now`（best-effort，失败由 gateway consumer 兜底）|

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

### 用户 /iam/users/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/users | GET | 查询用户列表 |
| /iam/users | POST | 创建用户 |
| /iam/users/:id | GET | 获取用户详情 |
| /iam/users/:id | PUT | 更新用户 |
| /iam/users/:id | DELETE | 删除用户 |

### 角色 /iam/roles/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /iam/roles | GET | 查询角色列表 |
| /iam/roles | POST | 创建角色 |
| /iam/roles/:id | GET | 获取角色详情 |
| /iam/roles/:id | PUT | 更新角色 |
| /iam/roles/:id | DELETE | 删除角色 |
| /iam/roles/:id/permissions | PUT | 绑定权限 |

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

支持 MySQL 和 SQLite。

## 表命名约定

所有表必须以 `iam_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## 豁免

（无）

## 状态说明

本服务整合了 token-service、credential-login-service、user-service 的功能。

## AI 检查时机

修改 services/iam-service/ 下文件时自动检查
