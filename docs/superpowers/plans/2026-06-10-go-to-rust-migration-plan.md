# Go 服务整体向 Rust 迁移主实施计划 [COMPLETED]

> **状态：已完成（Completed）**。所有 Go 服务已成功迁移至 Rust + Axum 技术栈。
>
> **迁移成果：**
> - auth-service → 已删除，功能并入 iam-service
> - user-service → 已迁移为 Rust 服务
> - token-server → 已迁移并更名为 token-service
> - 所有共享库（jwt-ctx、digua_authz、digua_common）均已用 Rust 重写
>
> **清理项见：** [`2026-06-10-go-to-rust-migration-cleanup-plan.md`](file:///d:/projects/coder-micro-services/docs/superpowers/plans/2026-06-10-go-to-rust-migration-cleanup-plan.md)

---

# Go 服务整体�?Rust �?主实施计�?[ORIGINAL, DO NOT EXECUTE]

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 仓库内所�?Go 服务与共享库 Big-Bang 切换�?Rust；删 Go 资产；同�?CI / 脚本 / AI 规则�?

**Architecture:** 单一 Cargo workspace 5 服务（Rust + Axum�? 3 共享 crate（digua_common / jwt-ctx / digua_authz）；保留现有 MySQL schema；保留现�?HTTP API 契约�?NET / Java SDK 不动）�?

**Tech Stack:** Rust 1.78+、`axum 0.8`、`sqlx 0.8` (mysql + runtime-tokio-rustls)、`redis 0.27`、`argon2 0.5`、`reqwest 0.12`、`tracing`、`aws_lc_rs`（沿�?workspace）�?

**Spec:** [`docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md`](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md)

**关联�?plan（未在本计划内，按顺序后续单独跑 writing-plans 生成）：**
- �?plan A：`docs/superpowers/plans/2026-06-11-auth-service-rust-plan.md`
- �?plan B：`docs/superpowers/plans/2026-06-11-user-service-rust-plan.md`
- �?plan C：`docs/superpowers/plans/2026-06-10-notification-service-plan.md`（已存在，按其执行）

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | workspace members 拓到 8 �?|
| `libs/jwt-ctx/Cargo.toml`、`src/lib.rs` | Principal extractor |
| `libs/jwt-ctx/tests/principal_test.rs` | 解析单测 |
| `libs/digua_authz/Cargo.toml`、`src/lib.rs` | require_role 中间�?|
| `libs/digua_authz/tests/require_role_test.rs` | 中间件单�?|
| `libs/digua_common/src/{password,db,redisx,http}.rs` | Argon2、MySQL 池、Redis 池、reqwest client |
| `libs/digua_common/tests/{password,db_pool}_test.rs` | 工具单测 |
| `services/notification-service/Cargo.toml` | 加入 workspace |
| 删除：`libs/go-jwt-ctx/`、`libs/go-digua_authz/`、`services/auth-service/**/*.go`、`services/user-service/**/*.go`、`scripts/e2e_test.go`、根 `go.mod`/`go.sum`/`go.work`（如有） |
| 修改：`.gitlab-ci.yml`、`scripts/test-orchestrator.ps1`、`scripts/lib/*.ps1`、`.trae/rules/project_rules.md`、`infrastructure/docker-compose.*.yaml`、`services/{auth,user}-service/Dockerfile` |

---

## Task 1: workspace 拓建

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 编辑 Cargo.toml �?members**

```toml
[workspace]
members = [
    "libs/digua_common",
    "libs/jwt-ctx",
    "libs/digua_authz",
    "services/token-server",
    "services/api-gateway",
    "services/auth-service",
    "services/user-service",
    "services/notification-service",
]
resolver = "2"
```

> �?`services/auth-service`、`services/user-service`、`services/notification-service` 此时仍是 Go 目录或尚未初始化，workspace 编译会失败。需先建 stub `Cargo.toml`（仅 `[package]` + `edition = "2021"`，无 `src/`）�?

- [ ] **Step 2: �?3 �?stub crate（auth-service / user-service / notification-service�?*

`services/auth-service/Cargo.toml`�?
```toml
[package]
name = "auth-service"
version = "0.1.0"
edition = "2021"
```

`services/user-service/Cargo.toml`：同上，name 改为 `user-service`�?

`services/notification-service/Cargo.toml`：同上，name 改为 `notification-service`�?

每个目录下建�?`src/lib.rs`�?
```rust
// placeholder
```

- [ ] **Step 3: �?workspace check**

Run: `cargo check --workspace`
Expected: `Finished` 无错误（libs / 现有 Rust 服务 + 3 �?stub 编译通过�?

- [ ] **Step 4: 提交**

```bash
git add Cargo.toml services/auth-service services/user-service services/notification-service
git commit -m "chore(workspace): extend cargo members to 8 with stubs"
```

---

## Task 2: libs/jwt-ctx crate 骨架

**Files:**
- Create: `libs/jwt-ctx/Cargo.toml`
- Create: `libs/jwt-ctx/src/lib.rs`

- [ ] **Step 1: �?Cargo.toml**

```toml
[package]
name = "jwt-ctx"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
thiserror.workspace = true
async-trait = "0.1"
```

- [ ] **Step 2: 写最�?lib.rs**

```rust
pub mod principal;

pub use principal::Principal;
```

- [ ] **Step 3: 验证编译**

Run: `cd libs/jwt-ctx && cargo check`
Expected: `Finished`（principal 模块尚未建，下一步建�?

- [ ] **Step 4: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): scaffold crate"
```

---

## Task 3: Principal 解析（TDD�?

**Files:**
- Create: `libs/jwt-ctx/src/principal.rs`
- Create: `libs/jwt-ctx/tests/principal_test.rs`

- [ ] **Step 1: 写失败的测试**

`libs/jwt-ctx/tests/principal_test.rs`�?
```rust
use axum::http::HeaderMap;
use digua_jwt_ctx::Principal;
use std::collections::HashSet;

#[test]
fn parses_all_headers() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "42".parse().unwrap());
    h.insert("x-user-dept", "7".parse().unwrap());
    h.insert("x-user-dept-leader", "true".parse().unwrap());
    h.insert("x-user-roles", "admin,sales".parse().unwrap());
    h.insert("x-user-perms", "user.read,user.write".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_id, 42);
    assert_eq!(p.dept_id, Some(7));
    assert!(p.dept_leader);
    assert_eq!(p.roles, vec!["admin".to_string(), "sales".to_string()]);
    assert_eq!(p.permissions, HashSet::from(["user.read".into(), "user.write".into()]));
}

#[test]
fn missing_user_id_is_error() {
    let h = HeaderMap::new();
    assert!(Principal::from_headers(&h).is_err());
}

#[test]
fn optional_fields_default() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "1".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.dept_id, None);
    assert!(!p.dept_leader);
    assert!(p.roles.is_empty());
    assert!(p.permissions.is_empty());
}
```

- [ ] **Step 2: 跑测试确�?fail**

Run: `cd libs/jwt-ctx && cargo test --test principal_test`
Expected: 编译失败（`Principal` 不存在）

- [ ] **Step 3: 写实�?*

`libs/jwt-ctx/src/principal.rs`�?
```rust
use axum::http::HeaderMap;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: u64,
    pub dept_id: Option<u64>,
    pub dept_leader: bool,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PrincipalError {
    #[error("missing x-user-id")]
    MissingUserId,
    #[error("invalid x-user-id")]
    InvalidUserId,
}

impl Principal {
    pub fn from_headers(h: &HeaderMap) -> Result<Self, PrincipalError> {
        let user_id = h.get("x-user-id")
            .ok_or(PrincipalError::MissingUserId)?
            .to_str().map_err(|_| PrincipalError::InvalidUserId)?
            .parse::<u64>().map_err(|_| PrincipalError::InvalidUserId)?;
        let dept_id = h.get("x-user-dept")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        let dept_leader = h.get("x-user-dept-leader")
            .and_then(|v| v.to_str().ok())
            .map(|s| s == "true")
            .unwrap_or(false);
        let roles = h.get("x-user-roles")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        let permissions = h.get("x-user-perms")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        Ok(Self { user_id, dept_id, dept_leader, roles, permissions })
    }

    pub fn has_role(&self, role: &str) -> bool { self.roles.iter().any(|r| r == role) }
    pub fn has_permission(&self, perm: &str) -> bool { self.permissions.contains(perm) }
}
```

- [ ] **Step 4: 跑测试确�?pass**

Run: `cd libs/jwt-ctx && cargo test --test principal_test`
Expected: 3 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): add Principal header parser with tests"
```

---

## Task 4: Principal �?axum extractor

**Files:**
- Modify: `libs/jwt-ctx/src/principal.rs`
- Create: `libs/jwt-ctx/tests/extractor_test.rs`

- [ ] **Step 1: 写失败测�?*

`libs/jwt-ctx/tests/extractor_test.rs`�?
```rust
use axum::body::Body;
use axum::http::{Request, header};
use axum::routing::get;
use axum::Router;
use digua_jwt_ctx::Principal;
use tower::ServiceExt;

async fn who(me: Principal) -> String { format!("u{}", me.user_id) }

#[tokio::test]
async fn extractor_returns_principal() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "42")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], b"u42");
}

#[tokio::test]
async fn missing_header_returns_401() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 2: 跑测试确�?fail**

Run: `cd libs/jwt-ctx && cargo test --test extractor_test`
Expected: 编译失败（未实现 `FromRequestParts`�?

- [ ] **Step 3: 实现 extractor**

�?`libs/jwt-ctx/src/principal.rs` 追加�?
```rust
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

#[async_trait::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = axum::http::StatusCode;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Self::from_headers(&parts.headers).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
    }
}
```

`Cargo.toml` 加依赖：
```toml
tower = "0.5"
```

- [ ] **Step 4: 跑测试确�?pass**

Run: `cd libs/jwt-ctx && cargo test`
Expected: 5 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): add axum FromRequestParts extractor"
```

---

## Task 5: libs/digua_authz crate 骨架

**Files:**
- Create: `libs/digua_authz/Cargo.toml`
- Create: `libs/digua_authz/src/lib.rs`

- [ ] **Step 1: �?Cargo.toml**

```toml
[package]
name = "digua_authz"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
jwt-ctx = { path = "../jwt-ctx" }
tower = "0.5"
tower-http = { version = "0.6", features = ["util"] }
thiserror.workspace = true
```

- [ ] **Step 2: 写最�?lib.rs**

```rust
pub mod require_role;

pub use require_role::require_role;
```

- [ ] **Step 3: 验证编译**

Run: `cd libs/digua_authz && cargo check`
Expected: `Finished`

- [ ] **Step 4: 提交**

```bash
git add libs/digua_authz
git commit -m "feat(digua_authz): scaffold crate"
```

---

## Task 6: require_role 中间件（TDD�?

**Files:**
- Create: `libs/digua_authz/src/require_role.rs`
- Create: `libs/digua_authz/tests/require_role_test.rs`

- [ ] **Step 1: 写失败测�?*

`libs/digua_authz/tests/require_role_test.rs`�?
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::middleware as axum_mw;
use axum::routing::get;
use axum::Router;
use digua_authz::require_role;
use tower::ServiceExt;

async fn secret(digua_jwt_ctx::Principal) -> &'static str { "ok" }

#[tokio::test]
async fn with_role_passes() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "1")
        .header(header::HeaderName::from_static("x-user-roles"), "user,admin")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn without_role_is_403() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "1")
        .header(header::HeaderName::from_static("x-user-roles"), "user")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_principal_is_401() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: 跑测试确�?fail**

Run: `cd libs/digua_authz && cargo test --test require_role_test`
Expected: 编译失败（`require_role` 不存在）

- [ ] **Step 3: 实现中间�?*

`libs/digua_authz/src/require_role.rs`�?
```rust
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use digua_jwt_ctx::Principal;

pub fn require_role(role: &'static str)
  -> impl Fn(Request, Next) -> futures::future::BoxFuture<'static, Result<Response, StatusCode>>
       + Clone + Send + Sync + 'static
{
    use futures::future::BoxFuture;
    move |req: Request, next: Next| {
        let role = role;
        Box::pin(async move {
            let (mut parts, body) = req.into_parts();
            let p = match Principal::from_request_parts(&mut parts, &()).await {
                Ok(p) => p,
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            };
            if !p.has_role(role) {
                return Err(StatusCode::FORBIDDEN);
            }
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }) as BoxFuture<'static, Result<Response, StatusCode>>
    }
}
```

`Cargo.toml` 加：
```toml
futures = "0.3"
```

- [ ] **Step 4: 跑测试确�?pass**

Run: `cd libs/digua_authz && cargo test`
Expected: 3 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/digua_authz
git commit -m "feat(digua_authz): add require_role middleware with tests"
```

---

## Task 7: libs/digua_common 扩展 �?password 模块（TDD�?

**Files:**
- Create: `libs/digua_common/src/password.rs`
- Create: `libs/digua_common/tests/password_test.rs`
- Modify: `libs/digua_common/src/lib.rs`

- [ ] **Step 1: 写失败测�?*

`libs/digua_common/tests/password_test.rs`�?
```rust
use digua_common::password::{hash, verify};

#[test]
fn hash_and_verify_roundtrip() {
    let h = hash("hunter2").unwrap();
    assert!(verify("hunter2", &h).unwrap());
    assert!(!verify("wrong", &h).unwrap());
}

#[test]
fn different_inputs_yield_different_hashes() {
    let h1 = hash("a").unwrap();
    let h2 = hash("a").unwrap();
    assert_ne!(h1, h2); // salt 随机
}
```

- [ ] **Step 2: 跑测试确�?fail**

Run: `cd libs/digua_common && cargo test --test password_test`
Expected: 编译失败（`password` 模块不存在）

- [ ] **Step 3: 写实�?*

`libs/digua_common/src/password.rs`�?
```rust
use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;

pub fn hash(p: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default().hash_password(p.as_bytes(), &salt)?.to_string())
}

pub fn verify(p: &str, h: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(h)?;
    Ok(Argon2::default().verify_password(p.as_bytes(), &parsed).is_ok())
}
```

`libs/digua_common/Cargo.toml` 加：
```toml
argon2.workspace = true
anyhow = "1"
```

`libs/digua_common/src/lib.rs`�?
```rust
pub mod password;
```

- [ ] **Step 4: 跑测试确�?pass**

Run: `cd libs/digua_common && cargo test --test password_test`
Expected: 2 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/digua_common
git commit -m "feat(digua_common): add argon2 password hash/verify"
```

---

## Task 8: libs/digua_common 扩展 �?db �?

**Files:**
- Create: `libs/digua_common/src/db.rs`
- Modify: `libs/digua_common/src/lib.rs`
- Modify: `libs/digua_common/Cargo.toml`

- [ ] **Step 1: 写实现（无外部依赖，连接测试在集成测试）**

`libs/digua_common/src/db.rs`�?
```rust
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;

pub async fn connect(url: &str, max: u32) -> anyhow::Result<MySqlPool> {
    Ok(MySqlPoolOptions::new()
        .max_connections(max)
        .acquire_timeout(Duration::from_secs(5))
        .connect(url).await?)
}
```

`libs/digua_common/Cargo.toml` 加：
```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql"] }
```

`libs/digua_common/src/lib.rs` 追加�?
```rust
pub mod db;
```

- [ ] **Step 2: 验证编译**

Run: `cd libs/digua_common && cargo check`
Expected: 通过

- [ ] **Step 3: 提交**

```bash
git add libs/digua_common
git commit -m "feat(digua_common): add mysql pool factory"
```

---

## Task 9: libs/digua_common 扩展 �?redis �?

**Files:**
- Create: `libs/digua_common/src/redisx.rs`
- Modify: `libs/digua_common/src/lib.rs`
- Modify: `libs/digua_common/Cargo.toml`

- [ ] **Step 1: 写实�?*

`libs/digua_common/src/redisx.rs`�?
```rust
use redis::aio::ConnectionManager;

pub async fn connect(url: &str) -> anyhow::Result<ConnectionManager> {
    let client = redis::Client::open(url)?;
    Ok(ConnectionManager::new(client).await?)
}
```

`libs/digua_common/Cargo.toml` 加：
```toml
redis.workspace = true
```

`libs/digua_common/src/lib.rs` 追加�?
```rust
pub mod redisx;
```

- [ ] **Step 2: 验证编译**

Run: `cd libs/digua_common && cargo check`
Expected: 通过

- [ ] **Step 3: 提交**

```bash
git add libs/digua_common
git commit -m "feat(digua_common): add redis connection manager factory"
```

---

## Task 10: libs/digua_common 扩展 �?http client

**Files:**
- Create: `libs/digua_common/src/http.rs`
- Modify: `libs/digua_common/src/lib.rs`
- Modify: `libs/digua_common/Cargo.toml`

- [ ] **Step 1: 写实�?*

`libs/digua_common/src/http.rs`�?
```rust
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client")
    })
}
```

`libs/digua_common/Cargo.toml` 加：
```toml
reqwest.workspace = true
```

`libs/digua_common/src/lib.rs` 追加�?
```rust
pub mod http;
```

- [ ] **Step 2: 验证编译**

Run: `cd libs/digua_common && cargo check`
Expected: 通过

- [ ] **Step 3: 提交**

```bash
git add libs/digua_common
git commit -m "feat(digua_common): add shared reqwest client"
```

---

## Task 11: �?Go 端契�?fixture（auth-service�?

**Files:**
- Create: `tests/specs/auth-service/login.yaml`
- Create: `tests/specs/auth-service/refresh.yaml`
- Create: `tests/specs/auth-service/me.yaml`

> 来源：`services/auth-service/integration_test.go` �?`services/auth-service/handlers/auth_test.go`（在删除 Go 之前抽完�?

- [ ] **Step 1: 读现�?Go 测试，列端点清单**

Run: `Get-Content services\auth-service\*.go, services\auth-service\handlers\*.go, services\auth-service\store\*.go`
预期：记录每个端点的 method / path / 请求字段 / 响应字段 / 状态码

- [ ] **Step 2: �?login.yaml（最小可跑契约）**

```yaml
service: auth-service
endpoints:
  - name: login_success
    request:
      method: POST
      path: /api/auth/login
      headers: { Content-Type: application/json }
      body: { username: "alice", password: "hunter2" }
    expect:
      status: 200
      body_contains: ["code", "expires_in"]

  - name: login_bad_password
    request:
      method: POST
      path: /api/auth/login
      headers: { Content-Type: application/json }
      body: { username: "alice", password: "WRONG" }
    expect:
      status: 401
      body: { error: "invalid credentials" }

  - name: login_locked
    request:
      method: POST
      path: /api/auth/login
      headers: { Content-Type: application/json }
      body: { username: "alice", password: "x" }
    expect:
      status: 423
      body_contains: ["retry_after"]
```

- [ ] **Step 3: �?refresh.yaml �?me.yaml**

`tests/specs/auth-service/refresh.yaml`�?
```yaml
service: auth-service
endpoints:
  - name: refresh_success
    request:
      method: POST
      path: /api/auth/refresh
      headers: { Content-Type: application/json }
      body: { refresh_token: "REFRESH" }
    expect:
      status: 200
      body_contains: ["access_token", "expires_in"]

  - name: refresh_invalid
    request:
      method: POST
      path: /api/auth/refresh
      headers: { Content-Type: application/json }
      body: { refresh_token: "BAD" }
    expect:
      status: 401
```

`tests/specs/auth-service/me.yaml`�?
```yaml
service: auth-service
endpoints:
  - name: me_with_principal
    request:
      method: GET
      path: /api/auth/me
      headers:
        X-User-Id: "42"
        X-User-Roles: "user"
    expect:
      status: 200
      body_contains: ["id", "username"]

  - name: me_unauthenticated
    request:
      method: GET
      path: /api/auth/me
    expect:
      status: 401
```

- [ ] **Step 4: 校验 spec**

Run: `.\scripts\test-orchestrator.ps1 validate-specs`
Expected: 通过（若 validator 已支持新路径；若不支持，先用 `Validate-Spec` �?lib 内单独调�?

- [ ] **Step 5: 提交**

```bash
git add tests/specs/auth-service
git commit -m "test(auth-service): add contract specs from Go integration tests"
```

---

## Task 12: �?Go 端契�?fixture（user-service�?

**Files:**
- Create: `tests/specs/user-service/users.yaml`
- Create: `tests/specs/user-service/roles.yaml`
- Create: `tests/specs/user-service/permissions.yaml`

- [ ] **Step 1: �?user-service Go 测试列端�?*

Run: `Get-Content services\user-service\*.go, services\user-service\handlers\*.go`
预期：users / roles / permissions 三个域，每个 CRUD 端点

- [ ] **Step 2: �?users.yaml**

```yaml
service: user-service
endpoints:
  - name: list_users
    request:
      method: GET
      path: /api/users
      headers: { X-User-Id: "9001", X-User-Roles: "user.admin" }
    expect:
      status: 200
      body_is_array: true

  - name: create_user
    request:
      method: POST
      path: /api/users
      headers: { X-User-Id: "9001", X-User-Roles: "user.admin", Content-Type: application/json }
      body: { username: "bob", password: "hunter2", dept_id: 7 }
    expect:
      status: 201
      body_contains: ["id", "username"]

  - name: create_user_forbidden
    request:
      method: POST
      path: /api/users
      headers: { X-User-Id: "9001", X-User-Roles: "user", Content-Type: application/json }
      body: { username: "bob", password: "x" }
    expect:
      status: 403
```

- [ ] **Step 3: �?roles.yaml �?permissions.yaml**

�?users.yaml 结构，每个域至少�?list / create / create_forbidden 三个端点�?

- [ ] **Step 4: 校验**

Run: `.\scripts\test-orchestrator.ps1 validate-specs`
Expected: 通过

- [ ] **Step 5: 提交**

```bash
git add tests/specs/user-service
git commit -m "test(user-service): add contract specs from Go integration tests"
```

---

## Task 13: 跑子 plan A（auth-service Rust 化）

**依赖**：本计划 Task 1-12 全部完成�?

> 启动新会话，调用 `writing-plans` 技能，�?`docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md` �?§5.1 节作为输入，生成 `docs/superpowers/plans/2026-06-11-auth-service-rust-plan.md`，再调用 `executing-plans` �?`subagent-driven-development` 执行�?

- [ ] **Step 1: �?writing-plans 生成�?plan A**

由新会话/agent 完成�?

- [ ] **Step 2: 跑子 plan A 全部 task**

要求：所�?`cargo test -p auth-service` �?`cargo clippy -p auth-service --all-targets -- -D warnings` 通过�?

- [ ] **Step 3: 校验 spec**

Run: `.\scripts\test-orchestrator.ps1 validate-specs`
Expected: auth-service 全部 spec 通过

- [ ] **Step 4: 提交**

�?plan A 完成时已�?commit。此处仅汇总�?

```bash
git log --oneline -20
```

---

## Task 14: 跑子 plan B（user-service Rust 化）

**依赖**：本计划 Task 13 完成�?

- [ ] **Step 1: �?writing-plans 生成�?plan B**

输入：spec §5.2；输出：`docs/superpowers/plans/2026-06-11-user-service-rust-plan.md`�?

- [ ] **Step 2: 跑子 plan B 全部 task**

要求：`cargo test -p user-service`、`cargo clippy -p user-service --all-targets -- -D warnings` 通过�?

- [ ] **Step 3: 校验 spec**

Run: `.\scripts\test-orchestrator.ps1 validate-specs`
Expected: user-service 全部 spec 通过

- [ ] **Step 4: 提交汇�?*

```bash
git log --oneline -30
```

---

## Task 15: 跑子 plan C（notification-service 接入�?

**依赖**：本计划 Task 1 完成（workspace �?notification-service）�?

- [ ] **Step 1: 按现�?plan 实施 notification-service**

输入：`docs/superpowers/plans/2026-06-10-notification-service-plan.md`�?

- [ ] **Step 2: 验证编译**

Run: `cargo check -p notification-service`
Expected: 通过

- [ ] **Step 3: 跑全 workspace 测试**

Run: `cargo test --workspace --all-targets`
Expected: 全部通过

- [ ] **Step 4: 提交**

按子 plan 自带 commit 流程�?

---

## Task 16: 删除 Go 资产

**Files:**
- Delete: `libs/go-jwt-ctx/`
- Delete: `libs/go-digua_authz/`
- Delete: `services/auth-service/` �?`*.go`、`go.mod`、`go.sum`
- Delete: `services/user-service/` �?`*.go`、`go.mod`、`go.sum`
- Delete: `scripts/e2e_test.go`
- Delete: �?`go.mod`、`go.sum`、`go.work`（如有）

- [ ] **Step 1: �?Go 共享�?*

```bash
git rm -r libs/go-jwt-ctx libs/go-digua_authz
```

- [ ] **Step 2: �?auth-service Go 文件（保留目录、保�?Cargo.toml、保�?Dockerfile�?*

```bash
git rm services/auth-service/*.go
git rm -r services/auth-service/handlers services/auth-service/models services/auth-service/store services/auth-service/config services/auth-service/bin
git rm services/auth-service/go.mod services/auth-service/go.sum services/auth-service/integration_test.go
```

- [ ] **Step 3: �?user-service Go 文件（同上模式）**

```bash
git rm services/user-service/*.go
git rm -r services/user-service/handlers services/user-service/models services/user-service/store services/user-service/config services/user-service/bin services/user-service/docs
git rm services/user-service/go.mod services/user-service/go.sum services/user-service/integration_test.go
```

- [ ] **Step 4: �?scripts/e2e_test.go �?scripts/go.mod**

```bash
git rm scripts/e2e_test.go scripts/go.mod scripts/go.sum
```

- [ ] **Step 5: 删根 go.mod / go.sum / go.work（如有）**

```bash
git rm go.mod go.sum go.work 2>$null
```

- [ ] **Step 6: 校验仓库�?Go 残留**

```bash
git ls-files | Select-String -Pattern "\.go$|go\.mod|go\.sum|go\.work"
```

Expected: 无输�?

- [ ] **Step 7: �?workspace 检�?*

Run: `cargo check --workspace`
Expected: 通过

- [ ] **Step 8: 提交**

```bash
git commit -m "chore: remove all go sources and module files"
```

---

## Task 17: CI 同步

**Files:**
- Modify: `.gitlab-ci.yml`

- [ ] **Step 1: �?go_test job**

�?`.gitlab-ci.yml` 中删�?`go_test` 整段（含 `tags`、`image`、`before_script`、`script`）�?

- [ ] **Step 2: rust_test 改全 workspace**

```yaml
rust_test:
  stage: test
  tags:
    - docker
  image: zhcoder-docker-registry.com:8000/official/rust:1.96-slim
  variables:
    REDIS_URL: redis://192.168.2.130:6379
  before_script:
    - export RUSTUP_DIST_SERVER=http://crates.kudi.repos
    - export RUSTUP_UPDATE_ROOT=http://crates.kudi.reposrustup
    - apt-get update && apt-get install -y pkg-config libssl-dev
  script:
    - cargo test --workspace --all-targets --verbose
```

- [ ] **Step 3: build_auth_service / build_user_service �?Dockerfile 内容**

`Dockerfile.auth-service` �?`Dockerfile.user-service` 改为 Rust 多阶�?builder（参�?`services/notification-service/Dockerfile` 风格）：

```dockerfile
FROM rust:1.78-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/<svc> ./services/<svc>
RUN cargo build --release -p <svc>

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/<svc> /usr/local/bin/<svc>
COPY docker-example/config/<svc> /app/config
EXPOSE <port>
CMD ["/usr/local/bin/<svc>", "/app/config/config.yaml"]
```

`<svc>` = `auth-service` / `user-service`；`<port>` 查原 Go 版�?

- [ ] **Step 4: 校验 CI YAML 语法**

Run: `python -c "import yaml; yaml.safe_load(open('.gitlab-ci.yml'))"`
Expected: 无异�?

- [ ] **Step 5: 提交**

```bash
git add .gitlab-ci.yml Dockerfile.auth-service Dockerfile.user-service
git commit -m "ci: switch all services to rust builder; remove go test job"
```

---

## Task 18: 脚本同步

**Files:**
- Modify: `scripts/test-orchestrator.ps1`
- Modify: `scripts/lib/Test-Runner.ps1`
- Modify: `scripts/lib/Docker-Helper.ps1`

- [ ] **Step 1: �?lib �?Go 相关分支**

Run: `Select-String -Path scripts\lib\*.ps1 -Pattern "go test|\.go\$|go\.mod"`
预期：列�?Go 处理代码�?

- [ ] **Step 2: 移除 Get-ServiceList 里的 Go 项（如有硬编码）**

`scripts/lib/Test-Runner.ps1` �?`Get-ServiceList` 改为只返�?Rust 服务�?
```powershell
function Get-ServiceList {
    @("token-server","api-gateway","auth-service","user-service","notification-service")
}
```

- [ ] **Step 3: 移除 Invoke-ServiceTests �?Go test 分支**

�?`go test ./... -v -short` 等调用改�?`cargo test -p $Service --all-targets`�?
```powershell
$cmd = "cargo", "test", "-p", $Service, "--all-targets"
& $cmd[0] @($cmd | Select-Object -Skip 0) 2>&1
```

- [ ] **Step 4: Docker-Helper �?service 列表同步**

`scripts/lib/Docker-Helper.ps1`：如有硬编码 Go 镜像名，改为�?Rust 镜像�?

- [ ] **Step 5: 跑脚�?*

Run: `.\scripts\test-orchestrator.ps1 loop -Service auth-service`
Expected: Rust 测试通过（或 fail 提示，可�?fix-prompt�?

- [ ] **Step 6: 提交**

```bash
git add scripts/
git commit -m "chore(scripts): remove go test branches; align with rust workspace"
```

---

## Task 19: AI 规则同步

**Files:**
- Modify: `.trae/rules/project_rules.md`

- [ ] **Step 1: �?`services/*/**.go` �?coder-run-tests 路径匹配**

`.trae/rules/project_rules.md` 中删�?Go 路径触发表项�?

- [ ] **Step 2: �?`cargo build 失败 �?go test 编译失败` 等含 Go 的旁�?*

把表中所�?Go 字眼删除�?

- [ ] **Step 3: 确认 Rust 触发表仍准确**

- `services/*/src/**.rs` �?`coder-run-tests -Service <name>`（保留并补全 5 个服务名�?
- `services/*/tests/**.rs` �?`coder-run-tests -Service <name>`（同上）
- `docker-example/config/api-gateway/routes/*.router.toml` �?`coder-route-validate`（保留）
- `tests/specs/**.yaml` �?`coder-spec-to-code`（保留）

- [ ] **Step 4: 提交**

```bash
git add .trae/rules/project_rules.md
git commit -m "chore(ai): remove go path rules from project_rules"
```

---

## Task 20: 收尾

**Files:** 当前所有改�?

- [ ] **Step 1: �?fmt**

Run: `cargo fmt --all`
Expected: 无输�?

- [ ] **Step 2: �?clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: �?warning

- [ ] **Step 3: 跑全 workspace 测试**

Run: `cargo test --workspace --all-targets`
Expected: 全部通过

- [ ] **Step 4: �?spec 校验**

Run: `.\scripts\test-orchestrator.ps1 validate-specs`
Expected: 全部 spec 通过

- [ ] **Step 5: �?SDK E2E smoke**

```bash
cd sdk/java && mvn -B verify
cd ../dotnet && dotnet test --nologo --verbosity minimal
```

Expected: 通过

- [ ] **Step 6: 提交（若�?fmt/clippy 改动�?*

```bash
git add -A
git commit -m "style(workspace): apply rustfmt and clippy fixes"
```

---

## 自审

- **Spec 覆盖**�?
  - §3 workspace 拓扑 �?Task 1
  - §4.1 libs/digua_common 扩展 �?Task 7-10
  - §4.2 libs/jwt-ctx �?Task 2-4
  - §4.3 libs/digua_authz �?Task 5-6
  - §5.1 auth-service Rust �?�?Task 13（子 plan A�?
  - §5.2 user-service Rust �?�?Task 14（子 plan B�?
  - §5.3 notification-service 接入 �?Task 15（子 plan C 已有�?
  - §6 契约保护 �?Task 11-12
  - §7 删除清单 �?Task 16
  - §8 CI/脚本/规则 �?Task 17-19
  - §10 实施顺序 �?Task 1-20
  - §11 测试策略 �?Task 20
- **占位�?*：所�?step 含可执行代码 / 命令
- **类型一�?*：`Principal`、`require_role`、`Argon2::default()`、`MySqlPoolOptions` �?spec §4 一�?
- **可执行�?*：每�?2-5 分钟，符合粒�?
- **YAGNI**：未引入新框架；未重设计 API；未重写 SDK
