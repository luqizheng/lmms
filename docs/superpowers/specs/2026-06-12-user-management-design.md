# User Service 用户管理基础功能设计

## 概述

完善 `user-service` 用户管理基础能力：在现有 `me / list / get / patch_extends` 之外，补齐**创建、删除、启停、基础字段维护、密码重置/修改、登录日志查询**等管理类接口。

## 范围

- 管理员对用户生命周期的 CRUD 端点
- 用户自助改密
- 登录日志查询
- 沿用现有 `Principal` 鉴权模型

## 不在范围

- 用户批量导入/导出
- 多租户隔离
- 密码复杂度策略引擎（仅做最小长度校验）
- 邮件/短信通知

## 接口清单

所有路径挂在现有 `/api/v1/users` 路由下。

| Method | Path | 鉴权 | 说明 |
|---|---|---|---|
| POST | `/api/v1/users` | role=`admin` | 创建用户，生成初始密码 |
| DELETE | `/api/v1/users/:id` | role=`admin` | 软删（status=0） |
| POST | `/api/v1/users/:id/enable` | role=`admin` | 启用 status=1 |
| POST | `/api/v1/users/:id/disable` | role=`admin` | 停用 status=0 |
| PATCH | `/api/v1/users/:id` | role=`admin` | 改 username/email/phone/nickname |
| POST | `/api/v1/users/:id/reset-password` | role=`admin` | 重置为新密码（不验旧） |
| POST | `/api/v1/users/me/password` | self | 改自己密码（需验旧） |
| GET | `/api/v1/users/:id/login-logs` | role=`admin` or self | 查登录日志（分页） |

## 数据 / 服务层

### `user_svc.rs` 新增

```rust
pub async fn create_user(pool, req: CreateUserReq) -> Result<u64, AppError>
pub async fn soft_delete_user(pool, id: u64) -> Result<(), AppError>
pub async fn set_user_status(pool, id: u64, status: i8) -> Result<(), AppError>
pub async fn update_user_basic(pool, id: u64, req: UpdateUserReq) -> Result<(), AppError>
pub async fn reset_password(pool, id: u64, new_password: &str) -> Result<(), AppError>
pub async fn change_password(pool, id: u64, old: &str, new: &str) -> Result<(), AppError>
pub async fn list_login_logs(pool, id: u64, page: u32, size: u32) -> Result<Vec<LoginLogRow>, AppError>
```

### 复用

- `auth_svc::hash_password`（Argon2）— 创建、重置、改密
- `auth_svc::verify_password` — `change_password` 验旧
- `user_svc::fetch_user_with_roles` — 创建后回包
- `LoginLogRow` — 日志查询结果

### DTO（`dto.rs` 新增）

```rust
pub struct CreateUserReq { username, password, email?, phone?, nickname? }
pub struct UpdateUserReq { username?, email?, phone?, nickname? }
pub struct AdminResetPasswordReq { new_password }
pub struct ChangePasswordReq { old_password, new_password }
```

## 业务规则

- 用户名唯一，软删后 username 占用（避免复用）
- 邮箱/手机可空，若非空需唯一
- 密码长度 ≥ 6（与 `register_admin` 一致）
- 不能停用/删除当前登录用户本身（防自锁）
- `change_password` 旧密码错 → `Unauthorized`
- 用户不存在 → `NotFound`

## 错误码

| 场景 | 错误 |
|---|---|
| username/email/phone 重复 | `Conflict` (409) |
| 密码 < 6 | `BadRequest` (400) |
| 旧密码错 | `Unauthorized` (401) |
| 非 admin | `Forbidden` (403) |
| 用户不存在 / 已停用 | `NotFound` (404) |

## 鉴权

- 网关层 JWT 校验后注入 `Principal`（含 `user_id` 与 role 码）
- handler 用 `Extension(Principal)` + 服务内 `require_admin(&principal)?` 辅助函数（判定 `principal.roles` 含 `"admin"`，否则 `Forbidden`）
- `me/password` 走 `principal.user_id` 自匹配，无 admin 校验

## 路由注册

`routes/users.rs` 内集中注册，避免散落在 `mod.rs`：

```rust
pub fn router() -> Router {
    Router::new()
        .route("/users", post(create_user).get(list_users))
        .route("/users/me", get(get_current_user).patch(update_current_user))
        .route("/users/me/password", post(change_my_password))
        .route("/users/:id", get(get_user).patch(update_user).delete(delete_user))
        .route("/users/:id/enable", post(enable_user))
        .route("/users/:id/disable", post(disable_user))
        .route("/users/:id/reset-password", post(admin_reset_password))
        .route("/users/:id/login-logs", get(list_user_login_logs))
}
```

## 测试

### 单元（`tests/user_svc_test.rs`）

- `create_user_duplicate_username_returns_conflict`
- `change_password_wrong_old_returns_unauthorized`
- `soft_delete_then_set_status_round_trip`

### 集成（`tests/integration_test.rs`）

- 8 个端点 happy path
- 非 admin 调 admin 端点 → 403
- 用户名冲突 → 409
- 软删后不能再 enable（业务后续讨论；本期仅记录行为，不强约束）
- 登录日志分页

## 迁移

- 无 schema 变更，复用现有 `users` / `login_logs` 表
- `extends_json` 列已存在（`2026-06-07-user-service-extends-design` 引入）

## 风险

- 软删不释放 username，依赖应用层避免误用
- admin 自删/自停未拦截时会导致系统无 admin；保留为后续加固项
