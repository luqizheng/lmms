# User Service Rust 重做 — Design

日期: 2026-06-10
父 plan: [2026-06-10-credential-login-user-rust-plan.md](2026-06-10-credential-login-user-rust-plan.md)
对应 plan (子 plan A): `2026-06-10-user-service-rust-plan.md` (待写)

## 目的

将 Go `services/user-service/` 全部删去，重头用 Rust 写 user-service。
不复用 Go 代码、不保留任何 Go API 兼容。endpoint 划分走"中 user-service"路线（不含 register）。

## 范围

**在范围内**:
- 新建 `services/user-service-rust/` (Rust crate)
- 14 业务端点 (user / role / permission CRUD)
- 1 内部端点 (claims)
- 1 health 端点
- 复用 `libs/digua_common` / `libs/jwt-ctx` / `libs/digua_authz`
- MySQL 数据模型与 Go 版保持字段一致（schema 不变）
- spec-to-code: 已有 `tests/specs/user-service/*.yaml` 改 host 指向 user-service-rust

**不在范围内**:
- 注册端点 `POST /api/users` → credential-login-service
- Login / refresh / logout → credential-login-service
- Go user-service 源保留在 `services/user-service/` 暂不删（待 plan 中 explicit step 处理）

## 架构

### Crate 结构

```
services/user-service-rust/
├── Cargo.toml
├── Dockerfile
├── config/
│   └── config.yaml
├── migrations/
│   └── (使用 libs/digua_common 共享 DDL)
├── src/
│   ├── main.rs          # 入口 / 启动 axum
│   ├── lib.rs
│   ├── config.rs       # serde_yaml 加载 config
│   ├── error.rs        # AppError + IntoResponse
│   ├── db.rs           # sqlx MySqlPool
│   ├── redisx.rs       # redis::aio::ConnectionManager (via digua_common)
│   ├── auth.rs         # require_service_token 中间件 (via digua_authz lib)
│   ├── models/
│   │   ├── mod.rs
│   │   ├── user.rs     # UserRow, UserDTO, UserExtends
│   │   ├── role.rs     # RoleRow, RoleDTO
│   │   └── permission.rs  # PermissionRow
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── users.rs    # /api/users/* 5 端点
│   │   ├── roles.rs    # /api/roles/* 7 端点
│   │   ├── permissions.rs  # /api/permissions 1 端点
│   │   ├── user_roles.rs   # /api/users/:id/roles 2 端点
│   │   ├── internal.rs # /internal/users/:id/claims
│   │   └── health.rs   # /health
│   ├── service/
│   │   ├── mod.rs
│   │   ├── user_svc.rs
│   │   ├── role_svc.rs
│   │   └── claim_svc.rs
│   └── dto.rs          # 请求/响应 DTO (serde)
└── tests/
    ├── user_test.rs
    ├── role_test.rs
    └── integration_test.rs
```

### 依赖

- `axum 0.7` (HTTP)
- `tokio 1`
- `sqlx 0.8` MySQL + `runtime-tokio-rustls`
- `redis 0.27` (via `digua_common::redisx`)
- `serde` / `serde_json` / `serde_yaml`
- `thiserror` / `anyhow`
- `tracing` + `tracing-subscriber`
- workspace libs: `digua_common`, `jwt-ctx`, `digua_authz`
- 工具链: 沿用 `aws_lc_rs` (不引入 ring 0.17)

### 端点 (15 路由)

| Method | Path | 鉴权 | Handler |
|---|---|---|---|
| GET | /api/users/me | user-jwt | get_current_user |
| PUT | /api/users/me | user-jwt | update_current_user |
| GET | /api/users | service-token | list_users |
| GET | /api/users/:id | service-token | get_user |
| PATCH | /api/users/:id/extends | service-token | patch_user_extends |
| GET | /api/roles | service-token | list_roles |
| POST | /api/roles | service-token | create_role |
| GET | /api/roles/:id | service-token | get_role |
| PUT | /api/roles/:id | service-token | update_role |
| DELETE | /api/roles/:id | service-token | delete_role |
| POST | /api/roles/:id/permissions | service-token | assign_role_permissions |
| GET | /api/permissions | service-token | list_permissions |
| POST | /api/users/:id/roles | service-token | assign_user_roles |
| DELETE | /api/users/:id/roles | service-token | remove_user_roles |
| GET | /internal/users/:id/claims | service-token (internal) | get_user_claims |
| GET | /health | none | health_check |

注: `POST /api/users` (register) **不在** user-service Rust, 在 credential-login-service Rust。

## 数据流

```
HTTP Request
   ↓
axum::Router
   ↓
middleware::require_service_token (digua_authz lib)
   ↓
route handler (axum extractors: Path, Query, Json, State, Extension<Principal>)
   ↓
service::xxx_svc (业务逻辑, 包含 db 错误 → AppError 转换)
   ↓
sqlx::query_as::<_, UserRow>(...)
   ↓
DB / Redis
   ↓
DTO 序列化 → JSON response
```

### 关键决策

1. **me 端点**: `Extension<Principal>` 从 axum 提取, 用 `principal.user_id` (string) 直接查 DB
2. **service-token 校验**: 走 `digua_authz::require_service_token` middleware, 已签发 token 中含 `aud=service-name`, 验证 issuer + signature
3. **user-jwt 校验**: 由 api-gateway 提前校验, 传入 `Principal` 即可 (与 Go 版 `principalBridge` 等价但走 axum Extension 而非 gin Context)
4. **分页**: 列表端点 `?page=1&size=20`, 默认 size=20, max=100
5. **错误格式**: `{"code": "USER_NOT_FOUND", "message": "..."}`, 4xx/5xx 共用 `AppError`

## 错误处理

### AppError (thiserror)

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("validation: {0}")] Validation(String),
    #[error("not found: {0}")] NotFound(String),
    #[error("conflict: {0}")] Conflict(String),
    #[error("forbidden")] Forbidden,
    #[error("unauthorized")] Unauthorized,
    #[error("db: {0}")] Db(#[from] sqlx::Error),
    #[error("internal: {0}")] Internal(#[from] anyhow::Error),
}
```

### HTTP 状态码映射

- `Validation` → 400
- `Unauthorized` → 401
- `Forbidden` → 403
- `NotFound` → 404
- `Conflict` → 409
- `Db` (sqlx::Error::RowNotFound) → 404, 其他 → 500
- `Internal` → 500

### 错误响应体

```json
{ "code": "USER_NOT_FOUND", "message": "user id=123 not found" }
```

## 测试

### 单元 (per module)

- `models/user.rs`: UserRow ↔ UserDTO 字段映射 / extends 字段过滤
- `dto.rs`: 序列化 / 反序列化 (golden json)
- `service/user_svc.rs`: 业务规则 (角色冲突检测 / extends merge 规则)

### 集成 (用 testcontainers-mysql)

`tests/integration_test.rs` 启动 MySQL 容器, migrate, 跑端到端测试:

1. `POST /api/roles` → 创建角色
2. `POST /api/users` (由 credential-login-service 测试) → 创建用户
3. `POST /api/users/:id/roles` → 分配角色
4. `GET /api/users/:id/claims` → 验证返回 roles

### Spec-to-code

现有 `tests/specs/user-service/claims-endpoint.yaml` 改 host/user → user-service-rust。
新增:
- `tests/specs/user-service-rust/users-me.yaml` (me 端点 2 个 case)
- `tests/specs/user-service-rust/roles-crud.yaml` (role 7 端点)

## 配置

`config/config.yaml`:

```yaml
server:
  host: "0.0.0.0"
  port: 8082
database:
  url: "mysql://user:pass@mysql:3306/kudi"
redis:
  url: "redis://redis:6379"
auth:
  service_token_issuer: "token-service"
  service_token_audience: "user-service"
```

`main.rs` 启动时: `load_config()` → `init_db()` → `init_redis()` → `axum::serve(...)`。

## 不做什么

- ❌ 不实现 register / login / refresh / logout (credential-login-service 负责)
- ❌ 不写 Dockerfile 多阶段 build 优化 (用现有 api-gateway Dockerfile 模式)
- ❌ 不集成 OpenTelemetry / metrics (后续 plan)
- ❌ 不迁移 Go 旧测试 (废弃)

## 风险

1. **MySQL 测试环境**: testcontainers-mysql 需 docker, CI 上需 docker-in-docker
2. **axum 0.7 vs 0.8**: 项目用 axum 0.7 (主 plan), 与 jwt-ctx lib 保持一致
3. **数据迁移**: 字段一致, 无需 ALTER TABLE, DDL 已在 `migrations/2026_06_10_add_user_code.sql`

## 关联

- 主 plan: [2026-06-10-credential-login-user-rust-plan.md](2026-06-10-credential-login-user-rust-plan.md)
- credential-login-service Rust design (待写)
- spec 源: `tests/specs/user-service/claims-endpoint.yaml`
