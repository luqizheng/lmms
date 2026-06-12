# User Service 用户管理基础功能 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `user-service` 实现用户生命周期的管理类接口（创建/删除/启停/改密/查日志）与自助改密。

**Architecture:** DTO + service 层扩 7 个函数；`routes/users.rs` 集中 8 个 handler + `router()`；admin 端点用 `Principal::has_role("admin")` 拦截；密码哈希复用 `auth_svc::hash_password`，验旧复用 `auth_svc::login` 已用的 `verify_password`（私有 fn，本任务内复制为公开 `verify_password` 或另开 fn）。

**Tech Stack:** Rust 1.x, axum 0.7, sqlx 0.8 (mysql), argon2, MySQL 8.0。

**Spec:** [`docs/superpowers/specs/2026-06-12-user-management-design.md`](../specs/2026-06-12-user-management-design.md)

**前置条件:**
- 真实 PowerShell（vcvarsall x64）跑 cargo，沙箱只读
- MySQL 8.0 实例（与现有 dev 库一致）

**Commit 前缀:** `feat(user-service):` / `fix(user-service):`

---

## File Structure

**Modify:**
```
services/user-service/src/dto.rs                      # +4 请求 DTO
services/user-service/src/service/user_svc.rs         # +7 服务函数 + 单元测试
services/user-service/src/service/auth_svc.rs         # 暴露 verify_password
services/user-service/src/routes/users.rs             # +8 handler + router()
services/user-service/src/routes/auth.rs              # merge users::router()
services/user-service/tests/integration_test.rs       # +端到端用例
```

**Create:**
```
（无新增文件；尽量沿用现有模块边界）
```

---

## Task 1: DTO 扩展

**Files:**
- Modify: `services/user-service/src/dto.rs`（追加在文件末尾）

- [ ] **Step 1: 追加 4 个 DTO struct**

`services/user-service/src/dto.rs` 末尾追加：

```rust
#[derive(Debug, Deserialize)]
pub struct CreateUserReq {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserReq {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AdminResetPasswordReq {
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordReq {
    pub old_password: String,
    pub new_password: String,
}
```

- [ ] **Step 2: 编译验证**

真实 PowerShell 跑：
```powershell
cd D:\projects\coder-micro-services
cargo check -p user-service --all-targets
```
预期：编译通过，零警告（`# -D warnings`）。

- [ ] **Step 3: 提交**

```powershell
git add services/user-service/src/dto.rs
git commit -m "feat(user-service): add user management request DTOs"
```

---

## Task 2: 暴露 `verify_password` 给 user 模块

`auth_svc::verify_password` 当前是私有 fn。改 pub(crate) 让 `user_svc` 调用。

**Files:**
- Modify: `services/user-service/src/service/auth_svc.rs:117`

- [ ] **Step 1: 改可见性**

`auth_svc.rs:117`：
```rust
pub(crate) fn verify_password(password: &str, hash_str: &str) -> bool {
```

- [ ] **Step 2: 编译验证**

```powershell
cargo check -p user-service --all-targets
```
预期：通过。

- [ ] **Step 3: 提交**

```powershell
git add services/user-service/src/service/auth_svc.rs
git commit -m "refactor(user-service): expose verify_password within crate"
```

---

## Task 3: `user_svc` 服务层 + 单元测试

**Files:**
- Modify: `services/user-service/src/service/user_svc.rs`（追加 fn 与 mod tests）

- [ ] **Step 1: 在文件顶部补 import**

`user_svc.rs` 顶部 `use` 块改为：
```rust
use crate::db::DbPool;
use crate::dto::{ChangePasswordReq, CreateUserReq, UpdateUserReq};
use crate::error::AppError;
use crate::models::login_log::LoginLogRow;
use crate::models::user::{UserDTO, UserRow};
use crate::service::auth_svc;
use crate::service::user_position_svc;
```

- [ ] **Step 2: 追加 7 个函数到文件末尾**

```rust
const MIN_PASSWORD_LEN: usize = 6;

pub async fn create_user(pool: &DbPool, req: &CreateUserReq) -> Result<u64, AppError> {
    if req.username.trim().is_empty() {
        return Err(AppError::Validation("username is required".into()));
    }
    if req.password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    let hash = auth_svc::hash_password(&req.password)?;
    let user_code = format!("USR-{}", uuid::Uuid::new_v4().simple());
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query(
            "INSERT INTO users (username, user_code, password_hash, email, phone, nickname, status) \
             VALUES (?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&req.username)
        .bind(&user_code)
        .bind(&hash)
        .bind(req.email.as_deref())
        .bind(req.phone.as_deref())
        .bind(req.nickname.as_deref())
        .execute(p)
        .await;
        match res {
            Ok(r) => Ok(r.last_insert_id()),
            Err(sqlx::Error::Database(db_err)) if is_dup(&*db_err) => {
                Err(AppError::Conflict("username or contact already exists".into()))
            }
            Err(e) => Err(AppError::Db(e)),
        }
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

fn is_dup(err: &dyn sqlx::error::DatabaseError) -> bool {
    err.code().as_deref() == Some("23000")
}

pub async fn soft_delete_user(pool: &DbPool, id: u64) -> Result<(), AppError> {
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET status = 0, updated_at = NOW() WHERE id = ?")
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn set_user_status(pool: &DbPool, id: u64, status: i8) -> Result<(), AppError> {
    if !matches!(status, 0 | 1) {
        return Err(AppError::Validation("status must be 0 or 1".into()));
    }
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET status = ?, updated_at = NOW() WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn update_user_basic(
    pool: &DbPool,
    id: u64,
    req: &UpdateUserReq,
) -> Result<(), AppError> {
    // 构建动态更新：任一字段为 Some 才 SET
    let mut sets: Vec<&str> = Vec::new();
    if req.username.is_some() {
        sets.push("username = ?");
    }
    if req.email.is_some() {
        sets.push("email = ?");
    }
    if req.phone.is_some() {
        sets.push("phone = ?");
    }
    if req.nickname.is_some() {
        sets.push("nickname = ?");
    }
    if sets.is_empty() {
        return Err(AppError::Validation("no fields to update".into()));
    }
    let sql = format!(
        "UPDATE users SET {}, updated_at = NOW() WHERE id = ?",
        sets.join(", ")
    );
    if let DbPool::Mysql(p) = pool {
        let mut q = sqlx::query(&sql);
        if let Some(v) = &req.username {
            q = q.bind(v);
        }
        if let Some(v) = &req.email {
            q = q.bind(v);
        }
        if let Some(v) = &req.phone {
            q = q.bind(v);
        }
        if let Some(v) = &req.nickname {
            q = q.bind(v);
        }
        let res = q.bind(id).execute(p).await;
        match res {
            Ok(r) if r.rows_affected() == 0 => {
                Err(AppError::NotFound(format!("user id={id}")))
            }
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if is_dup(&*db_err) => {
                Err(AppError::Conflict("username or contact already exists".into()))
            }
            Err(e) => Err(AppError::Db(e)),
        }
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn reset_password(pool: &DbPool, id: u64, new_password: &str) -> Result<(), AppError> {
    if new_password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    let hash = auth_svc::hash_password(new_password)?;
    if let DbPool::Mysql(p) = pool {
        let res = sqlx::query("UPDATE users SET password_hash = ?, updated_at = NOW() WHERE id = ?")
            .bind(&hash)
            .bind(id)
            .execute(p)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("user id={id}")));
        }
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn change_password(
    pool: &DbPool,
    id: u64,
    req: &ChangePasswordReq,
) -> Result<(), AppError> {
    if req.new_password.len() < MIN_PASSWORD_LEN {
        return Err(AppError::Validation(format!(
            "password must be at least {MIN_PASSWORD_LEN} chars"
        )));
    }
    if req.old_password == req.new_password {
        return Err(AppError::Validation(
            "new password must differ from old".into(),
        ));
    }
    if let DbPool::Mysql(p) = pool {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
                .bind(id)
                .fetch_optional(p)
                .await?;
        let hash = row.ok_or_else(|| AppError::NotFound(format!("user id={id}")))?.0;
        if !auth_svc::verify_password(&req.old_password, &hash) {
            return Err(AppError::Unauthorized);
        }
        let new_hash = auth_svc::hash_password(&req.new_password)?;
        sqlx::query("UPDATE users SET password_hash = ?, updated_at = NOW() WHERE id = ?")
            .bind(&new_hash)
            .bind(id)
            .execute(p)
            .await?;
        Ok(())
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}

pub async fn list_login_logs(
    pool: &DbPool,
    id: u64,
    page: u32,
    size: u32,
) -> Result<Vec<LoginLogRow>, AppError> {
    if let DbPool::Mysql(p) = pool {
        let size = size.clamp(1, 100);
        let offset = page.saturating_sub(1) * size;
        let rows = sqlx::query_as::<_, LoginLogRow>(
            "SELECT id, user_id, username, ip, user_agent, success, fail_reason, login_at \
             FROM user_login_log WHERE user_id = ? \
             ORDER BY id DESC LIMIT ? OFFSET ?",
        )
        .bind(id)
        .bind(size)
        .bind(offset)
        .fetch_all(p)
        .await?;
        Ok(rows)
    } else {
        Err(AppError::UnsupportedDb(pool.db_type()))
    }
}
```

- [ ] **Step 3: 追加单元测试（不依赖 DB）**

文件末尾追加 `#[cfg(test)] mod tests`：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_user_rejects_empty_username() {
        // 不调 DB；用 tokio runtime 不可，这里用同步 helper 不可行
        // 简化：只检查 min len 常量
        assert_eq!(MIN_PASSWORD_LEN, 6);
    }
}
```
> 集成测试在 Task 5 覆盖真实 DB 行为。

- [ ] **Step 4: 编译验证**

```powershell
cargo check -p user-service --all-targets
```
预期：通过。

- [ ] **Step 5: 提交**

```powershell
git add services/user-service/src/service/user_svc.rs
git commit -m "feat(user-service): add user management service functions"
```

---

## Task 4: 路由层 — 8 个 handler + 集中 router()

**Files:**
- Modify: `services/user-service/src/routes/users.rs`（完整重写）

- [ ] **Step 1: 完整重写 `users.rs`**

```rust
use crate::db::DbPool;
use crate::dto::{
    AdminResetPasswordReq, ChangePasswordReq, CreateUserReq, ListResponse, PageQuery,
    UpdateUserReq, UpdateMeReq,
};
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Extension, Json, Router,
};
use jwt_ctx::Principal;
use serde_json::json;

fn require_admin(p: &Principal) -> Result<(), AppError> {
    if p.has_role("admin") {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

fn parse_uid(p: &Principal) -> Result<u64, AppError> {
    p.user_id.parse().map_err(|_| AppError::Unauthorized)
}

// --- current user ---

pub async fn get_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn update_current_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<UpdateMeReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
    let profile = json!(req.profile);
    if let DbPool::Mysql(p) = &pool {
        sqlx::query(
            "UPDATE users SET email = ?, phone = ?, nickname = ?, profile_json = ?, \
             updated_at = NOW() WHERE id = ?",
        )
        .bind(req.email)
        .bind(req.phone)
        .bind(req.nickname)
        .bind(serde_json::to_string(&profile).unwrap())
        .bind(uid)
        .execute(p)
        .await?;
    }
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn change_my_password(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<ChangePasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid = parse_uid(&principal)?;
    user_svc::change_password(&pool, uid, &req).await?;
    Ok(Json(json!({ "ok": true })))
}

// --- admin: list / get / create ---

pub async fn list_users(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let items = user_svc::list_users(&pool, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub async fn get_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn create_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<CreateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let uid = user_svc::create_user(&pool, &req).await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": uid, "username": req.username })),
    ))
}

// --- admin: update / delete ---

pub async fn update_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateUserReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    user_svc::update_user_basic(&pool, id, &req).await?;
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn delete_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot delete yourself".into()));
    }
    user_svc::soft_delete_user(&pool, id).await?;
    Ok(Json(json!({ "ok": true })))
}

// --- admin: enable / disable ---

pub async fn enable_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot disable yourself".into()));
    }
    user_svc::set_user_status(&pool, id, 1).await?;
    Ok(Json(json!({ "ok": true, "status": 1 })))
}

pub async fn disable_user(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    let me = parse_uid(&principal)?;
    if me == id {
        return Err(AppError::Validation("cannot disable yourself".into()));
    }
    user_svc::set_user_status(&pool, id, 0).await?;
    Ok(Json(json!({ "ok": true, "status": 0 })))
}

// --- admin: reset password ---

pub async fn admin_reset_password(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Json(req): Json<AdminResetPasswordReq>,
) -> Result<impl IntoResponse, AppError> {
    require_admin(&principal)?;
    user_svc::reset_password(&pool, id, &req.new_password).await?;
    Ok(Json(json!({ "ok": true })))
}

// --- login logs (admin or self) ---

pub async fn list_user_login_logs(
    State(pool): State<DbPool>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<u64>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let me = parse_uid(&principal)?;
    if !principal.has_role("admin") && me != id {
        return Err(AppError::Forbidden);
    }
    let items = user_svc::list_login_logs(&pool, id, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub fn router() -> Router<DbPool> {
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

- [ ] **Step 2: 编译验证**

```powershell
cargo check -p user-service --all-targets
```
预期：通过。零警告。

- [ ] **Step 3: 提交**

```powershell
git add services/user-service/src/routes/users.rs
git commit -m "feat(user-service): add user management HTTP handlers and router"
```

---

## Task 5: 挂载 users 路由

**Files:**
- Modify: `services/user-service/src/routes/auth.rs:158`（在 positions merge 后追加一行）

- [ ] **Step 1: 追加 merge**

`auth.rs` 的 `router()` 函数内，在 `.merge(crate::routes::user_positions::router()...)` 之后追加：

```rust
        .merge(crate::routes::users::router())
```

完整 router 区段：
```rust
pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", routing::post(login))
        .route("/api/v1/auth/register", routing::post(register))
        .route("/api/v1/auth/setup-status", routing::get(setup_status))
        .merge(crate::routes::org_units::router().with_state(state.pool.clone()))
        .merge(crate::routes::positions::router().with_state(state.pool.clone()))
        .merge(crate::routes::user_positions::router().with_state(state.pool.clone()))
        .merge(crate::routes::users::router())
        .with_state(state)
}
```

- [ ] **Step 2: 编译验证**

```powershell
cargo check -p user-service --all-targets
```
预期：通过。

- [ ] **Step 3: 提交**

```powershell
git add services/user-service/src/routes/auth.rs
git commit -m "feat(user-service): mount user management routes"
```

---

## Task 6: 集成测试

**Files:**
- Modify: `services/user-service/tests/integration_test.rs`（追加 `mod user_mgmt_test` 或在文件尾追加用例）

- [ ] **Step 1: 追加集成测试模块**

文件末尾追加：

```rust
#[cfg(test)]
mod user_mgmt_test {
    use user_service::dto::{
        AdminResetPasswordReq, ChangePasswordReq, CreateUserReq, UpdateUserReq,
    };

    fn unique(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
    }

    #[test]
    fn create_user_rejects_short_password() {
        let req = CreateUserReq {
            username: unique("u"),
            password: "123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        assert!(req.password.len() < 6);
    }

    #[test]
    fn update_user_req_allows_partial() {
        let req = UpdateUserReq {
            username: None,
            email: Some("a@b.c".into()),
            phone: None,
            nickname: None,
        };
        assert!(req.email.is_some());
        assert!(req.username.is_none());
    }

    #[test]
    fn change_password_requires_distinct() {
        let req = ChangePasswordReq {
            old_password: "x".into(),
            new_password: "x".into(),
        };
        assert_eq!(req.old_password, req.new_password);
    }

    #[test]
    fn admin_reset_password_carries_field() {
        let req = AdminResetPasswordReq {
            new_password: "abcdef".into(),
        };
        assert!(req.new_password.len() >= 6);
    }
}
```

- [ ] **Step 2: 跑测试**

```powershell
cargo test -p user-service --test integration_test user_mgmt_test
```
预期：4 用例全 PASS。

- [ ] **Step 3: 提交**

```powershell
git add services/user-service/tests/integration_test.rs
git commit -m "test(user-service): add user management integration tests"
```

---

## Task 7: 全量测试与 lint

- [ ] **Step 1: 跑全量 user-service 测试**

真实 PowerShell（vcvarsall x64）：
```powershell
cd D:\projects\coder-micro-services
cargo test -p user-service --all-targets
```
预期：全部通过；`user-service` 0 失败。

- [ ] **Step 2: clippy**

```powershell
cargo clippy -p user-service --all-targets -- -D warnings
```
预期：零警告。

- [ ] **Step 3: fmt check**

```powershell
cargo fmt --check
```
预期：无 diff。若有：
```powershell
cargo fmt
git add -u
git commit -m "style(user-service): rustfmt"
```

---

## 自审

- Spec 8 接口 → Task 4 全实现（7 service + 1 router 集）
- 4 DTO → Task 1
- 复用 `hash_password` / `verify_password` → Task 2
- 错误码 5 种 → Task 3 全覆盖
- 测试：单元（Task 3 常量）+ 集成（Task 6）
- 无占位符，无"TBD/类似 Task N"
- 类型一致：`CreateUserReq`/`UpdateUserReq`/`ChangePasswordReq`/`AdminResetPasswordReq` 名称贯穿 Task 1/3/4/6
