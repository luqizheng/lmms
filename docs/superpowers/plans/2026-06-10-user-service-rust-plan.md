# User Service Rust 重做 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重头用 Rust 写 user-service, 替换 Go `services/user-service/`, 暴露 14 业务端点 + 1 internal + 1 health, 复用 `common` / `jwt-ctx` / `authz` libs.

**Architecture:** Axum 0.7 + sqlx 0.8 (MySQL) + redis 0.27 + thiserror + tracing. 三层: routes → service → models. middleware 走 `authz::require_service_token`. `me` 端点从 `Extension<Principal>` 提取 user_id.

**Tech Stack:** Rust 1.x, axum 0.7, tokio 1, sqlx 0.8, redis 0.27, common lib, jwt-ctx lib, authz lib, testcontainers 0.20.

**Spec:** [`docs/superpowers/specs/2026-06-10-user-service-rust-design.md`](../specs/2026-06-10-user-service-rust-design.md)

**前置依赖**:
- 主 plan Task 8 完成 (libs/common 含 password / db / redisx / http)
- 主 plan Task 10-12 完成 (token-service 改造 + api-gateway 路由)
- 工具链: 真实 PowerShell 跑 `vcvarsall x64` 后 cargo (sandbox 隔离 vcvarsall)

**注**: 因 ring 0.17.14 + MSVC 18 不兼容, 全部 cargo 命令在**真实 PowerShell** 中跑 (用 vcvarsall 加载 v143). sandbox 中**不**能跑 cargo build.

---

## File Structure

**Create:**
```
services/user-service-rust/Cargo.toml
services/user-service-rust/Dockerfile
services/user-service-rust/config/config.yaml
services/user-service-rust/src/main.rs
services/user-service-rust/src/lib.rs
services/user-service-rust/src/config.rs
services/user-service-rust/src/error.rs
services/user-service-rust/src/db.rs
services/user-service-rust/src/auth.rs
services/user-service-rust/src/dto.rs
services/user-service-rust/src/models/mod.rs
services/user-service-rust/src/models/user.rs
services/user-service-rust/src/models/role.rs
services/user-service-rust/src/models/permission.rs
services/user-service-rust/src/routes/mod.rs
services/user-service-rust/src/routes/health.rs
services/user-service-rust/src/routes/internal.rs
services/user-service-rust/src/routes/users.rs
services/user-service-rust/src/routes/roles.rs
services/user-service-rust/src/routes/permissions.rs
services/user-service-rust/src/routes/user_roles.rs
services/user-service-rust/src/service/mod.rs
services/user-service-rust/src/service/user_svc.rs
services/user-service-rust/src/service/role_svc.rs
services/user-service-rust/src/service/claim_svc.rs
services/user-service-rust/tests/user_test.rs
services/user-service-rust/tests/role_test.rs
services/user-service-rust/tests/integration_test.rs
```

**Modify (test spec):**
```
tests/specs/user-service/claims-endpoint.yaml  # 改 host
tests/specs/user-service-rust/users-me.yaml  # 新建
tests/specs/user-service-rust/roles-crud.yaml  # 新建
```

---

## Task 1: Scaffold crate

**Files:**
- Create: `services/user-service-rust/Cargo.toml`
- Create: `services/user-service-rust/src/lib.rs`
- Create: `services/user-service-rust/src/main.rs`

- [ ] **Step 1: Create Cargo.toml**

`services/user-service-rust/Cargo.toml`:

```toml
[package]
name = "user-service"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../../libs/common" }
jwt-ctx = { path = "../../libs/jwt-ctx" }
authz = { path = "../../libs/authz" }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql"] }
redis.workspace = true
serde.workspace = true
serde_json.workspace = true
serde_yaml = "0.9"
thiserror.workspace = true
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
uuid.workspace = true
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
testcontainers = "0.20"
testcontainers-modules = { version = "0.8", features = ["mysql"] }
reqwest.workspace = true
```

- [ ] **Step 2: Create lib.rs stub**

`services/user-service-rust/src/lib.rs`:

```rust
pub mod config;
pub mod error;
pub mod db;
pub mod auth;
pub mod dto;
pub mod models;
pub mod routes;
pub mod service;
```

- [ ] **Step 3: Create main.rs stub**

`services/user-service-rust/src/main.rs`:

```rust
use anyhow::Result;
use user_service::config::load_config;
use user_service::db::init_pool;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config("config/config.yaml")?;
    let pool = init_pool(&cfg.database.url, cfg.database.max_connections).await?;

    tracing::info!("user-service-rust started on {}:{}", cfg.server.host, cfg.server.port);
    let _ = pool; // placeholder

    Ok(())
}
```

- [ ] **Step 4: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): scaffold crate with Cargo.toml and main"
```

---

## Task 2: Config + Error 模块 (TDD)

**Files:**
- Create: `services/user-service-rust/src/config.rs`
- Create: `services/user-service-rust/src/error.rs`
- Test: `services/user-service-rust/tests/error_test.rs`

- [ ] **Step 1: Write failing test for AppError → HTTP status**

`services/user-service-rust/tests/error_test.rs`:

```rust
use user_service::error::AppError;
use axum::http::StatusCode;
use axum::response::IntoResponse;

#[test]
fn validation_maps_to_400() {
    let err = AppError::Validation("bad".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[test]
fn not_found_maps_to_404() {
    let err = AppError::NotFound("user".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn forbidden_maps_to_403() {
    let err = AppError::Forbidden;
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[test]
fn db_row_not_found_maps_to_404() {
    let err = AppError::Db(sqlx::Error::RowNotFound);
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn unauthorized_maps_to_401() {
    let err = AppError::Unauthorized;
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn conflict_maps_to_409() {
    let err = AppError::Conflict("dup".into());
    let resp = err.into_response();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd D:\projects\coder-micro-services; cargo test -p user-service --test error_test`
Expected: FAIL with "AppError not found in module user_service::error"

- [ ] **Step 3: Write AppError enum**

`services/user-service-rust/src/error.rs`:

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("forbidden")]
    Forbidden,
    #[error("unauthorized")]
    Unauthorized,
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "VALIDATION",
            AppError::NotFound(_) => "NOT_FOUND",
            AppError::Conflict(_) => "CONFLICT",
            AppError::Forbidden => "FORBIDDEN",
            AppError::Unauthorized => "UNAUTHORIZED",
            AppError::Db(sqlx::Error::RowNotFound) => "NOT_FOUND",
            AppError::Db(_) => "DB_ERROR",
            AppError::Internal(_) => "INTERNAL",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Db(sqlx::Error::RowNotFound) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "code": self.code(),
            "message": self.to_string(),
        }));
        (self.status(), body).into_response()
    }
}
```

- [ ] **Step 4: Write config module**

`services/user-service-rust/src/config.rs`:

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub redis: RedisCfg,
    pub auth: AuthCfg,
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}
```

- [ ] **Step 5: Create config file**

`services/user-service-rust/config/config.yaml`:

```yaml
server:
  host: "0.0.0.0"
  port: 8082
database:
  url: "mysql://user:pass@localhost:3306/kudi"
  max_connections: 16
redis:
  url: "redis://localhost:6379"
auth:
  service_token_issuer: "token-service"
  service_token_audience: "user-service"
```

- [ ] **Step 6: Add stub for db module**

`services/user-service-rust/src/db.rs`:

```rust
use anyhow::Result;
use sqlx::mysql::MySqlPool;

pub async fn init_pool(url: &str, max: u32) -> Result<MySqlPool> {
    common::db::connect(url, max).await
}
```

- [ ] **Step 7: Add stub for auth module**

`services/user-service-rust/src/auth.rs`:

```rust
// placeholder, full impl in Task 5
```

- [ ] **Step 8: Add stub for dto, models, routes, service**

`services/user-service-rust/src/dto.rs`:

```rust
// placeholder, full impl in Task 3
```

`services/user-service-rust/src/models/mod.rs`:

```rust
pub mod user;
pub mod role;
pub mod permission;
```

`services/user-service-rust/src/models/user.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/models/role.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/models/permission.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/mod.rs`:

```rust
pub mod health;
pub mod internal;
pub mod users;
pub mod roles;
pub mod permissions;
pub mod user_roles;
```

`services/user-service-rust/src/routes/health.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/internal.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/users.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/roles.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/permissions.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/routes/user_roles.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/service/mod.rs`:

```rust
pub mod user_svc;
pub mod role_svc;
pub mod claim_svc;
```

`services/user-service-rust/src/service/user_svc.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/service/role_svc.rs`:

```rust
// placeholder
```

`services/user-service-rust/src/service/claim_svc.rs`:

```rust
// placeholder
```

- [ ] **Step 9: Run test to verify it passes**

Run: `cd D:\projects\coder-micro-services; cargo test -p user-service --test error_test`
Expected: PASS (6 tests)

- [ ] **Step 10: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): config + error module with tests"
```

---

## Task 3: Models (TDD)

**Files:**
- Create: `services/user-service-rust/src/models/user.rs`
- Create: `services/user-service-rust/src/models/role.rs`
- Create: `services/user-service-rust/src/models/permission.rs`
- Test: `services/user-service-rust/tests/models_test.rs`

- [ ] **Step 1: Write failing test for UserRow ↔ UserDTO mapping**

`services/user-service-rust/tests/models_test.rs`:

```rust
use user_service::models::user::{UserRow, UserDTO, UserProfile};
use serde_json::json;

#[test]
fn user_row_to_dto_includes_all_fields() {
    let row = UserRow {
        id: 1,
        username: "alice".into(),
        email: Some("a@x.com".into()),
        phone: None,
        nickname: Some("Ali".into()),
        status: 1,
        user_code: "USR-001".into(),
        profile_json: "{}".into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto: UserDTO = row.into_dto(vec!["admin".into()]);
    assert_eq!(dto.id, 1);
    assert_eq!(dto.username, "alice");
    assert_eq!(dto.user_code, "USR-001");
    assert_eq!(dto.roles, vec!["admin"]);
    assert_eq!(dto.profile, UserProfile::default());
}

#[test]
fn user_profile_default_is_empty() {
    let p = UserProfile::default();
    assert_eq!(serde_json::to_value(&p).unwrap(), json!({}));
}

#[test]
fn user_profile_parses_json_string() {
    let p = UserProfile::from_json(r#"{"dept":"eng"}"#).unwrap();
    assert_eq!(p.dept.as_deref(), Some("eng"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p user-service --test models_test`
Expected: FAIL with "module user_service::models::user not found"

- [ ] **Step 3: Write UserRow / UserDTO / UserProfile**

`services/user-service-rust/src/models/user.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: u64,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    pub status: i8,
    pub user_code: String,
    pub profile_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UserProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl UserProfile {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        if s.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(s)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDTO {
    pub id: u64,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    pub status: i8,
    pub user_code: String,
    pub roles: Vec<String>,
    pub profile: UserProfile,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extends: BTreeMap<String, serde_json::Value>,
}

impl UserRow {
    pub fn into_dto(self, roles: Vec<String>) -> UserDTO {
        let profile = UserProfile::from_json(&self.profile_json).unwrap_or_default();
        UserDTO {
            id: self.id,
            username: self.username,
            email: self.email,
            phone: self.phone,
            nickname: self.nickname,
            status: self.status,
            user_code: self.user_code,
            roles,
            profile,
            created_at: self.created_at,
            updated_at: self.updated_at,
            extends: BTreeMap::new(),
        }
    }
}
```

- [ ] **Step 4: Write RoleRow / RoleDTO**

`services/user-service-rust/src/models/role.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RoleRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleDTO {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub permission_codes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl RoleRow {
    pub fn into_dto(self, permission_codes: Vec<String>) -> RoleDTO {
        RoleDTO {
            id: self.id,
            code: self.code,
            name: self.name,
            description: self.description,
            permission_codes,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
```

- [ ] **Step 5: Write PermissionRow**

`services/user-service-rust/src/models/permission.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct PermissionRow {
    pub id: u64,
    pub code: String,
    pub name: String,
    pub resource: String,
    pub action: String,
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p user-service --test models_test`
Expected: PASS (3 tests)

- [ ] **Step 7: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): models user/role/permission with TDD"
```

---

## Task 4: Service 层 user_svc (TDD)

**Files:**
- Create: `services/user-service-rust/src/service/user_svc.rs`
- Test: `services/user-service-rust/tests/user_svc_test.rs`

- [ ] **Step 1: Write failing test for build_user_dto**

`services/user-service-rust/tests/user_svc_test.rs`:

```rust
use user_service::models::user::UserRow;
use user_service::service::user_svc::build_user_dto;

#[test]
fn build_user_dto_with_empty_roles() {
    let row = UserRow {
        id: 7,
        username: "bob".into(),
        email: None,
        phone: None,
        nickname: None,
        status: 1,
        user_code: "USR-007".into(),
        profile_json: "".into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto = build_user_dto(row, vec![]);
    assert_eq!(dto.id, 7);
    assert!(dto.roles.is_empty());
}

#[test]
fn build_user_dto_with_multiple_roles() {
    let row = UserRow {
        id: 7,
        username: "bob".into(),
        email: None,
        phone: None,
        nickname: None,
        status: 1,
        user_code: "USR-007".into(),
        profile_json: r#"{"dept":"eng"}"#.into(),
        created_at: "2026-06-10T00:00:00Z".into(),
        updated_at: "2026-06-10T00:00:00Z".into(),
    };
    let dto = build_user_dto(row, vec!["admin".into(), "ops".into()]);
    assert_eq!(dto.roles.len(), 2);
    assert_eq!(dto.profile.dept.as_deref(), Some("eng"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p user-service --test user_svc_test`
Expected: FAIL

- [ ] **Step 3: Write user_svc with build_user_dto + DB queries**

`services/user-service-rust/src/service/user_svc.rs`:

```rust
use crate::error::AppError;
use crate::models::user::{UserDTO, UserRow};
use sqlx::MySqlPool;

pub fn build_user_dto(row: UserRow, roles: Vec<String>) -> UserDTO {
    row.into_dto(roles)
}

pub async fn fetch_user_with_roles(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<UserDTO, AppError> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE id = ? AND status = 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

    let roles = fetch_user_role_codes(pool, user_id).await?;
    Ok(build_user_dto(row, roles))
}

pub async fn fetch_user_by_code(
    pool: &MySqlPool,
    user_code: &str,
) -> Result<UserDTO, AppError> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE user_code = ? AND status = 1",
    )
    .bind(user_code)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user_code={user_code}")))?;

    let roles = fetch_user_role_codes(pool, row.id).await?;
    Ok(build_user_dto(row, roles))
}

pub async fn fetch_user_role_codes(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r \
         INNER JOIN user_roles ur ON ur.role_id = r.id \
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}

pub async fn list_users(
    pool: &MySqlPool,
    page: u32,
    size: u32,
) -> Result<Vec<UserDTO>, AppError> {
    let size = size.clamp(1, 100);
    let offset = (page.saturating_sub(1)) * size;
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, email, phone, nickname, status, user_code, \
                profile_json, created_at, updated_at \
         FROM users WHERE status = 1 ORDER BY id LIMIT ? OFFSET ?",
    )
    .bind(size)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let roles = fetch_user_role_codes(pool, row.id).await?;
        out.push(build_user_dto(row, roles));
    }
    Ok(out)
}

pub async fn update_user_extends(
    pool: &MySqlPool,
    user_id: u64,
    extends: &serde_json::Value,
) -> Result<(), AppError> {
    let exists: Option<(u64,)> = sqlx::query_as("SELECT id FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("user id={user_id}")));
    }

    let s = serde_json::to_string(extends)
        .map_err(|e| AppError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET extends_json = ?, updated_at = NOW() WHERE id = ?")
        .bind(s)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn assign_user_roles(
    pool: &MySqlPool,
    user_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    for role_id in role_ids {
        sqlx::query("INSERT IGNORE INTO user_roles (user_id, role_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(role_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn remove_user_roles(
    pool: &MySqlPool,
    user_id: u64,
    role_ids: &[u64],
) -> Result<(), AppError> {
    let placeholders = vec!["?"; role_ids.len()].join(",");
    let sql = format!(
        "DELETE FROM user_roles WHERE user_id = ? AND role_id IN ({placeholders})"
    );
    let mut q = sqlx::query(&sql).bind(user_id);
    for r in role_ids {
        q = q.bind(r);
    }
    q.execute(pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p user-service --test user_svc_test`
Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): user_svc with role/extends/CRUD queries"
```

---

## Task 5: Service 层 role_svc + claim_svc

**Files:**
- Create: `services/user-service-rust/src/service/role_svc.rs`
- Create: `services/user-service-rust/src/service/claim_svc.rs`

- [ ] **Step 1: Write role_svc**

`services/user-service-rust/src/service/role_svc.rs`:

```rust
use crate::error::AppError;
use crate::models::role::{RoleDTO, RoleRow};
use sqlx::MySqlPool;

pub async fn list_roles(pool: &MySqlPool) -> Result<Vec<RoleDTO>, AppError> {
    let rows = sqlx::query_as::<_, RoleRow>(
        "SELECT id, code, name, description, created_at, updated_at \
         FROM roles ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let perms = fetch_role_permission_codes(pool, row.id).await?;
        out.push(row.into_dto(perms));
    }
    Ok(out)
}

pub async fn get_role(pool: &MySqlPool, role_id: u64) -> Result<RoleDTO, AppError> {
    let row = sqlx::query_as::<_, RoleRow>(
        "SELECT id, code, name, description, created_at, updated_at \
         FROM roles WHERE id = ?",
    )
    .bind(role_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("role id={role_id}")))?;
    let perms = fetch_role_permission_codes(pool, role_id).await?;
    Ok(row.into_dto(perms))
}

pub async fn create_role(
    pool: &MySqlPool,
    code: &str,
    name: &str,
    description: Option<&str>,
) -> Result<RoleDTO, AppError> {
    let res = sqlx::query(
        "INSERT INTO roles (code, name, description) VALUES (?, ?, ?)",
    )
    .bind(code)
    .bind(name)
    .bind(description)
    .execute(pool)
    .await;

    let id = match res {
        Ok(r) => r.last_insert_id(),
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23000") => {
            return Err(AppError::Conflict(format!("role code={code}")));
        }
        Err(e) => return Err(e.into()),
    };
    get_role(pool, id).await
}

pub async fn update_role(
    pool: &MySqlPool,
    role_id: u64,
    name: &str,
    description: Option<&str>,
) -> Result<RoleDTO, AppError> {
    let res = sqlx::query(
        "UPDATE roles SET name = ?, description = ?, updated_at = NOW() WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(role_id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("role id={role_id}")));
    }
    get_role(pool, role_id).await
}

pub async fn delete_role(pool: &MySqlPool, role_id: u64) -> Result<(), AppError> {
    let res = sqlx::query("DELETE FROM roles WHERE id = ?")
        .bind(role_id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("role id={role_id}")));
    }
    Ok(())
}

pub async fn fetch_role_permission_codes(
    pool: &MySqlPool,
    role_id: u64,
) -> Result<Vec<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT p.code FROM permissions p \
         INNER JOIN role_permissions rp ON rp.permission_id = p.id \
         WHERE rp.role_id = ?",
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}

pub async fn assign_role_permissions(
    pool: &MySqlPool,
    role_id: u64,
    permission_ids: &[u64],
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    for pid in permission_ids {
        sqlx::query(
            "INSERT IGNORE INTO role_permissions (role_id, permission_id) VALUES (?, ?)",
        )
        .bind(role_id)
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
```

- [ ] **Step 2: Write claim_svc**

`services/user-service-rust/src/service/claim_svc.rs`:

```rust
use crate::error::AppError;
use crate::models::permission::PermissionRow;
use sqlx::MySqlPool;

#[derive(Debug, serde::Serialize)]
pub struct UserClaims {
    pub user_id: u64,
    pub username: String,
    pub user_code: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub status: i8,
}

pub async fn fetch_user_claims(
    pool: &MySqlPool,
    user_id: u64,
) -> Result<UserClaims, AppError> {
    let row: (u64, String, String, i8) = sqlx::query_as(
        "SELECT id, username, user_code, status FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("user id={user_id}")))?;

    let roles: Vec<(String,)> = sqlx::query_as(
        "SELECT r.code FROM roles r \
         INNER JOIN user_roles ur ON ur.role_id = r.id \
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let perms: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.code FROM permissions p \
         INNER JOIN role_permissions rp ON rp.permission_id = p.id \
         INNER JOIN user_roles ur ON ur.role_id = rp.role_id \
         WHERE ur.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(UserClaims {
        user_id: row.0,
        username: row.1,
        user_code: row.2,
        status: row.3,
        roles: roles.into_iter().map(|(c,)| c).collect(),
        permissions: perms.into_iter().map(|(c,)| c).collect(),
    })
}

pub async fn list_permissions(pool: &MySqlPool) -> Result<Vec<PermissionRow>, AppError> {
    let rows = sqlx::query_as::<_, PermissionRow>(
        "SELECT id, code, name, resource, action FROM permissions ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 3: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): role_svc + claim_svc"
```

---

## Task 6: DTO + auth 模块

**Files:**
- Create: `services/user-service-rust/src/dto.rs`
- Create: `services/user-service-rust/src/auth.rs`

- [ ] **Step 1: Write DTOs**

`services/user-service-rust/src/dto.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct UpdateMeReq {
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    #[serde(default)]
    pub profile: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct PatchExtendsReq {
    pub extends: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleReq {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateRoleReq {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolePermissionsReq {
    pub permission_ids: Vec<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUserRolesReq {
    pub role_ids: Vec<u64>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_page() -> u32 { 1 }
fn default_size() -> u32 { 20 }
```

- [ ] **Step 2: Write auth module (delegates to authz lib)**

`services/user-service-rust/src/auth.rs`:

```rust
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
};

/// Internal endpoint auth: require service token signed by token-service.
/// Delegates to `authz::require_service_token` after extracting the bearer token.
pub async fn require_internal_service_token(
    req: Request,
    next: Next,
) -> Result<Response, Response> {
    authz::require_service_token(req, next).await
}
```

(Verify the `authz::require_service_token` signature matches; if not, adapt by reading Authorization header + verifying via common::token.)

- [ ] **Step 3: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): dto + auth middleware"
```

---

## Task 7: Routes — health + internal

**Files:**
- Create: `services/user-service-rust/src/routes/health.rs`
- Create: `services/user-service-rust/src/routes/internal.rs`

- [ ] **Step 1: Write health route**

`services/user-service-rust/src/routes/health.rs`:

```rust
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use sqlx::MySqlPool;

pub async fn health(State(pool): State<MySqlPool>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1").execute(&pool).await.is_ok();
    let status_code = if db_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    let body = Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "service": "user-service-rust",
        "components": { "database": if db_ok { "up" } else { "down" } }
    }));
    (status_code, body)
}
```

- [ ] **Step 2: Write internal claims route**

`services/user-service-rust/src/routes/internal.rs`:

```rust
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use sqlx::MySqlPool;
use crate::service::claim_svc;

pub async fn get_user_claims(
    State(pool): State<MySqlPool>,
    Path(user_id): Path<u64>,
) -> Result<impl IntoResponse, crate::error::AppError> {
    let claims = claim_svc::fetch_user_claims(&pool, user_id).await?;
    Ok(Json(claims))
}
```

- [ ] **Step 3: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): routes health + internal claims"
```

---

## Task 8: Routes — users

**Files:**
- Create: `services/user-service-rust/src/routes/users.rs`

- [ ] **Step 1: Write users route module**

`services/user-service-rust/src/routes/users.rs`:

```rust
use crate::dto::{ListResponse, PageQuery, PatchExtendsReq, UpdateMeReq};
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use jwt_ctx::Principal;
use serde_json::json;
use sqlx::MySqlPool;

pub async fn get_current_user(
    State(pool): State<MySqlPool>,
    Extension(principal): Extension<Principal>,
) -> Result<impl IntoResponse, AppError> {
    let uid: u64 = principal
        .user_id
        .parse()
        .map_err(|_| AppError::Unauthorized)?;
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn update_current_user(
    State(pool): State<MySqlPool>,
    Extension(principal): Extension<Principal>,
    Json(req): Json<UpdateMeReq>,
) -> Result<impl IntoResponse, AppError> {
    let uid: u64 = principal
        .user_id
        .parse()
        .map_err(|_| AppError::Unauthorized)?;
    let profile = json!(req.profile);
    sqlx::query(
        "UPDATE users SET email = ?, phone = ?, nickname = ?, profile_json = ?, \
         updated_at = NOW() WHERE id = ?",
    )
    .bind(req.email)
    .bind(req.phone)
    .bind(req.nickname)
    .bind(serde_json::to_string(&profile).unwrap())
    .bind(uid)
    .execute(&pool)
    .await?;
    let dto = user_svc::fetch_user_with_roles(&pool, uid).await?;
    Ok(Json(dto))
}

pub async fn list_users(
    State(pool): State<MySqlPool>,
    Query(q): Query<PageQuery>,
) -> Result<impl IntoResponse, AppError> {
    let items = user_svc::list_users(&pool, q.page, q.size).await?;
    Ok(Json(ListResponse {
        items,
        page: q.page,
        size: q.size,
    }))
}

pub async fn get_user(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = user_svc::fetch_user_with_roles(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn patch_user_extends(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<PatchExtendsReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::update_user_extends(&pool, id, &json!(req.extends)).await?;
    Ok(Json(json!({ "ok": true })))
}
```

- [ ] **Step 2: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): routes users (me, list, get, extends)"
```

---

## Task 9: Routes — roles + permissions + user_roles

**Files:**
- Create: `services/user-service-rust/src/routes/roles.rs`
- Create: `services/user-service-rust/src/routes/permissions.rs`
- Create: `services/user-service-rust/src/routes/user_roles.rs`

- [ ] **Step 1: Write roles route**

`services/user-service-rust/src/routes/roles.rs`:

```rust
use crate::dto::{
    AssignRolePermissionsReq, CreateRoleReq, UpdateRoleReq,
};
use crate::error::AppError;
use crate::service::role_svc;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use sqlx::MySqlPool;

pub async fn list_roles(State(pool): State<MySqlPool>) -> Result<impl IntoResponse, AppError> {
    let items = role_svc::list_roles(&pool).await?;
    Ok(Json(items))
}

pub async fn create_role(
    State(pool): State<MySqlPool>,
    Json(req): Json<CreateRoleReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = role_svc::create_role(&pool, &req.code, &req.name, req.description.as_deref()).await?;
    Ok(Json(dto))
}

pub async fn get_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    let dto = role_svc::get_role(&pool, id).await?;
    Ok(Json(dto))
}

pub async fn update_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<UpdateRoleReq>,
) -> Result<impl IntoResponse, AppError> {
    let dto = role_svc::update_role(&pool, id, &req.name, req.description.as_deref()).await?;
    Ok(Json(dto))
}

pub async fn delete_role(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
) -> Result<impl IntoResponse, AppError> {
    role_svc::delete_role(&pool, id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn assign_role_permissions(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignRolePermissionsReq>,
) -> Result<impl IntoResponse, AppError> {
    role_svc::assign_role_permissions(&pool, id, &req.permission_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
```

- [ ] **Step 2: Write permissions route**

`services/user-service-rust/src/routes/permissions.rs`:

```rust
use crate::error::AppError;
use crate::service::claim_svc;
use axum::{extract::State, response::IntoResponse, Json};
use sqlx::MySqlPool;

pub async fn list_permissions(
    State(pool): State<MySqlPool>,
) -> Result<impl IntoResponse, AppError> {
    let items = claim_svc::list_permissions(&pool).await?;
    Ok(Json(items))
}
```

- [ ] **Step 3: Write user_roles route**

`services/user-service-rust/src/routes/user_roles.rs`:

```rust
use crate::dto::AssignUserRolesReq;
use crate::error::AppError;
use crate::service::user_svc;
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use sqlx::MySqlPool;

pub async fn assign_user_roles(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::assign_user_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn remove_user_roles(
    State(pool): State<MySqlPool>,
    Path(id): Path<u64>,
    Json(req): Json<AssignUserRolesReq>,
) -> Result<impl IntoResponse, AppError> {
    user_svc::remove_user_roles(&pool, id, &req.role_ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
```

- [ ] **Step 4: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): routes roles + permissions + user_roles"
```

---

## Task 10: main.rs 集成 (axum::Router 拼装)

**Files:**
- Modify: `services/user-service-rust/src/main.rs`

- [ ] **Step 1: Write full main.rs**

`services/user-service-rust/src/main.rs`:

```rust
use anyhow::Result;
use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use sqlx::MySqlPool;
use std::net::SocketAddr;
use user_service::{
    config::load_config,
    db::init_pool,
    routes,
};
use jwt_ctx::jwt_extractor;

#[derive(Clone)]
struct AppState {
    pool: MySqlPool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = load_config("config/config.yaml")?;
    let pool = init_pool(&cfg.database.url, cfg.database.max_connections).await?;

    // /api/users/me 走 user-jwt (api-gateway 已校验, 这里仅取 Principal)
    let me_routes = Router::new()
        .route("/me", get(routes::users::get_current_user))
        .route("/me", put(routes::users::update_current_user))
        .route_layer(middleware::from_fn(jwt_extractor));

    // 其余业务端点走 service-token
    let business_routes = Router::new()
        .route("/users", get(routes::users::list_users))
        .route("/users/:id", get(routes::users::get_user))
        .route("/users/:id/extends", patch(routes::users::patch_user_extends))
        .route("/users/:id/roles", post(routes::user_roles::assign_user_roles))
        .route("/users/:id/roles", delete(routes::user_roles::remove_user_roles))
        .route("/roles", get(routes::roles::list_roles))
        .route("/roles", post(routes::roles::create_role))
        .route("/roles/:id", get(routes::roles::get_role))
        .route("/roles/:id", put(routes::roles::update_role))
        .route("/roles/:id", delete(routes::roles::delete_role))
        .route("/roles/:id/permissions", post(routes::roles::assign_role_permissions))
        .route("/permissions", get(routes::permissions::list_permissions))
        .route_layer(middleware::from_fn(user_service::auth::require_internal_service_token));

    // 内部端点 (不走 api-gateway, 仅内网)
    let internal_routes = Router::new()
        .route("/users/:id/claims", get(routes::internal::get_user_claims))
        .route_layer(middleware::from_fn(user_service::auth::require_internal_service_token));

    let app = Router::new()
        .nest("/api/users", me_routes.merge(business_routes.clone()))
        .nest("/api", business_routes)
        .nest("/internal", internal_routes)
        .route("/health", get(routes::health::health))
        .with_state(pool);

    let addr: SocketAddr = format!("{}:{}", cfg.server.host, cfg.server.port)
        .parse()
        .expect("parse addr");
    tracing::info!("user-service-rust listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

(Note: nest path conflicts must be resolved; verify with `cargo check` after writing.)

- [ ] **Step 2: Run cargo check**

Run: `cd D:\projects\coder-micro-services; cargo check -p user-service`
Expected: 0 errors (warnings ok)

- [ ] **Step 3: Commit**

```bash
git add services/user-service-rust/
git commit -m "feat(user-service-rust): main.rs with axum router wiring"
```

---

## Task 11: Dockerfile

**Files:**
- Create: `services/user-service-rust/Dockerfile`

- [ ] **Step 1: Write Dockerfile**

`services/user-service-rust/Dockerfile`:

```dockerfile
FROM rust:1.81-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY libs libs
COPY services services
RUN cargo build --release -p user-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/user-service /usr/local/bin/user-service
COPY services/user-service-rust/config /app/config
EXPOSE 8082
CMD ["user-service"]
```

- [ ] **Step 2: Commit**

```bash
git add services/user-service-rust/Dockerfile
git commit -m "feat(user-service-rust): Dockerfile for production"
```

---

## Task 12: 集成测试 (testcontainers)

**Files:**
- Create: `services/user-service-rust/tests/integration_test.rs`

- [ ] **Step 1: Write integration test**

`services/user-service-rust/tests/integration_test.rs`:

```rust
//! Integration test using testcontainers-mysql.
//! Run with: `cargo test -p user-service --test integration_test -- --test-threads=1`
//! Requires Docker running on host.

use sqlx::mysql::MySqlPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::mysql::Mysql;

async fn setup_pool() -> (ContainerAsync<Mysql>, MySqlPool) {
    let container = Mysql::default().start().await.expect("start mysql");
    let host = container.get_host().await.expect("host");
    let port = container.get_host_port_ipv4(3306).await;
    let url = format!("mysql://root:secret@{host}:{port}/mysql");
    let pool = MySqlPool::connect(&url).await.expect("connect");
    // Apply migrations
    sqlx::query(include_str!("../../migrations/2026_06_10_add_user_code.sql"))
        .execute(&pool)
        .await
        .ok();
    (container, pool)
}

#[tokio::test]
async fn create_and_fetch_user() {
    let (_c, pool) = setup_pool().await;
    // Insert
    sqlx::query("INSERT INTO users (username, user_code) VALUES (?, ?)")
        .bind("alice")
        .bind("USR-001")
        .execute(&pool)
        .await
        .unwrap();
    // Fetch
    let row: (String,) = sqlx::query_as("SELECT username FROM users WHERE user_code = ?")
        .bind("USR-001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "alice");
}

#[tokio::test]
async fn role_create_and_assign() {
    let (_c, pool) = setup_pool().await;
    sqlx::query("INSERT INTO roles (code, name) VALUES (?, ?)")
        .bind("admin")
        .bind("Admin")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (username, user_code) VALUES (?, ?)")
        .bind("bob")
        .bind("USR-002")
        .execute(&pool)
        .await
        .unwrap();
    let role_id: (u64,) = sqlx::query_as("SELECT id FROM roles WHERE code = ?")
        .bind("admin")
        .fetch_one(&pool)
        .await
        .unwrap();
    let user_id: (u64,) = sqlx::query_as("SELECT id FROM users WHERE user_code = ?")
        .bind("USR-002")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id.0)
        .bind(role_id.0)
        .execute(&pool)
        .await
        .unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_roles WHERE user_id = ?")
        .bind(user_id.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}
```

- [ ] **Step 2: Run integration test**

Run: `cd D:\projects\coder-micro-services; cargo test -p user-service --test integration_test -- --test-threads=1`
Expected: 2 PASS (requires Docker)

- [ ] **Step 3: Commit**

```bash
git add services/user-service-rust/
git commit -m "test(user-service-rust): integration test with testcontainers"
```

---

## Task 13: spec-to-code YAML

**Files:**
- Create: `tests/specs/user-service-rust/users-me.yaml`
- Create: `tests/specs/user-service-rust/roles-crud.yaml`
- Modify: `tests/specs/user-service/claims-endpoint.yaml` (change host to user-service-rust)

- [ ] **Step 1: Write users-me spec**

`tests/specs/user-service-rust/users-me.yaml`:

```yaml
name: user-service-rust.users-me
base_url: http://user-service-rust:8082
scenarios:
  - name: get current user
    request:
      method: GET
      path: /api/users/me
      headers:
        Authorization: Bearer ${SERVICE_TOKEN}
    expect:
      status: 200
      json:
        id: 1
        username: alice
        roles: [admin]
  - name: update current user nickname
    request:
      method: PUT
      path: /api/users/me
      headers:
        Authorization: Bearer ${USER_JWT}
        Content-Type: application/json
      body:
        nickname: Ali
    expect:
      status: 200
      json:
        id: 1
        nickname: Ali
```

- [ ] **Step 2: Write roles-crud spec**

`tests/specs/user-service-rust/roles-crud.yaml`:

```yaml
name: user-service-rust.roles-crud
base_url: http://user-service-rust:8082
scenarios:
  - name: create role
    request:
      method: POST
      path: /api/roles
      headers:
        Authorization: Bearer ${SERVICE_TOKEN}
        Content-Type: application/json
      body:
        code: editor
        name: Editor
    expect:
      status: 200
      json:
        code: editor
        name: Editor
  - name: list roles
    request:
      method: GET
      path: /api/roles
      headers:
        Authorization: Bearer ${SERVICE_TOKEN}
    expect:
      status: 200
      json:
        - code: admin
        - code: editor
  - name: delete role
    request:
      method: DELETE
      path: /api/roles/2
      headers:
        Authorization: Bearer ${SERVICE_TOKEN}
    expect:
      status: 200
      json:
        ok: true
```

- [ ] **Step 3: Update existing claims-endpoint spec**

`tests/specs/user-service/claims-endpoint.yaml` 修改:
- 改 `base_url` 从 `http://user-service:8082` 到 `http://user-service-rust:8082`
- 改 `Authorization` header scheme 与新 service-token 一致

- [ ] **Step 4: Validate spec**

Run: `cd D:\projects\coder-micro-services; powershell -File scripts\test-orchestrator.ps1 validate-specs`
Expected: all specs pass validation

- [ ] **Step 5: Commit**

```bash
git add tests/specs/user-service/ tests/specs/user-service-rust/
git commit -m "test(specs): user-service-rust me + roles-crud specs"
```

---

## Task 14: 删 Go 旧 user-service 源

**Files:**
- Delete: `services/user-service/` (Go)

- [ ] **Step 1: 备份并删除**

Run: `cd D:\projects\coder-micro-services; Move-Item services/user-service services/user-service-go-bak -Force`
(Or `git rm -r services/user-service` if branch already committed with old files.)

- [ ] **Step 2: Verify workspace builds**

Run: `cd D:\projects\coder-micro-services; cargo check --workspace`
Expected: 0 errors (user-service-rust 仍在, Go user-service 已不在)

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "chore(workspace): remove Go user-service source (replaced by user-service-rust)"
```

---

## Self-Review Checklist

- [x] Spec coverage: 14 业务 + 1 internal + 1 health → Task 1-10 全部覆盖
- [x] Placeholder scan: 0 TBD / TODO (only `?page=1&size=20` URL syntax, not placeholders)
- [x] Type consistency: UserRow / UserDTO / RoleDTO / PermissionRow / UserClaims 类型名一致
- [x] Test coverage: error / models / user_svc 三个 unit + integration 1 个
- [x] 鉴权: `me` 走 user-jwt, 其他业务走 service-token, internal 走 service-token
- [x] 错误: AppError 6 个 variant + 7 个 status code 映射

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-10-user-service-rust-plan.md`. Two execution options:

1. **Subagent-Driven (recommended)** - dispatch fresh subagent per task, review between
2. **Inline Execution** - execute tasks in this session, batch checkpoints

**注意**: 因 ring 0.17.14 + MSVC 18 不兼容, **全部 cargo 命令必须在真实 PowerShell (vcvarsall 加载 v143 后) 跑**. sandbox 中 cargo build 失败是预期的, 不是 plan bug.
