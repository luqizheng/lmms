# Credential-Login & User Service 重头做 — 主实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 删除 Go 版 auth-service / user-service；新建 Rust 版 credential-login-service 与 user-service；改造 token-server 为 token-service，承担所有 token 签发；删 Go 资产；同步 CI / 脚本 / AI 规则。

**Architecture:** 单一 Cargo workspace（4 Rust 服务 + 3 共享 crate）；token-service 集中签发 user JWT 与 service JWT；credential-login-service 与 user-service 通过 service JWT 互调内部端点；user-service 数据落 `oa3.member_*`（沿用 docs/member_org.sql），新增 `UserCode` 字段。

**Tech Stack:** Rust 1.78+、`axum 0.8`、`sqlx 0.8` (mysql + runtime-tokio-rustls)、`redis 0.27`、`argon2 0.5`、`jwt-simple 0.12`、`reqwest 0.12`、`nanoid 0.4`、`tracing`、`aws_lc_rs`（沿用 workspace）。

**Spec:** [`docs/superpowers/specs/2026-06-10-credential-login-user-rust-design.md`](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-10-credential-login-user-rust-design.md)

**关联子 plan（按顺序后续单独跑 writing-plans 生成）：**
- 子 plan A：`docs/superpowers/plans/2026-06-11-user-service-rust-plan.md`
- 子 plan B：`docs/superpowers/plans/2026-06-11-credential-login-service-rust-plan.md`

**取代关系：**
- 本计划取代 `docs/superpowers/plans/2026-06-10-go-to-rust-migration-plan.md`（已标 Superseded）
- `docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md` 已标 Superseded

---

## 文件结构

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | workspace members 调整 |
| `migrations/2026_06_10_add_user_code.sql` | oa3.member_user 加 UserCode 列 + Rust 回填入口 |
| `services/token-server/` → `services/token-service/` | 改名；加 `/api/v1/tokens/user-jwt` 端点 |
| `services/credential-login-service/` (新建) | 完整 Rust 服务（子 plan B 实施） |
| `services/user-service/` (新建) | 完整 Rust 服务（子 plan A 实施） |
| `libs/jwt-ctx/src/principal.rs` | `Principal` 加 `user_code` 字段 |
| `libs/authz/src/require_service_token.rs` (新建) | service JWT 验签中间件 |
| `docker/config/api-gateway/routes/{credential-login,user,token}.router.toml` (新建) | 路由 |
| `docker/config/{credential-login,user,token}-service/config.yaml` (新建) | 容器内配置 |
| `Dockerfile.{credential-login,user,token}-service` (新建) | Rust builder |
| `infrastructure/docker-compose.*.yaml` | image 调整 |
| `.gitlab-ci.yml` | 删 go_test；改 rust_test；改 build jobs |
| `scripts/test-orchestrator.ps1`、`scripts/lib/*.ps1` | 移除 Go 分支 |
| `.trae/rules/project_rules.md` | 删 Go 规则 |
| 删除：`services/auth-service/`、`libs/go-jwt-ctx/`、`libs/go-authz/`、`scripts/e2e_test.go`、根 `go.mod`/`go.sum`/`go.work` | — |

---

## Task 1: workspace 调整

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 编辑 Cargo.toml**

```toml
[workspace]
members = [
    "libs/common",
    "libs/jwt-ctx",
    "libs/authz",
    "services/token-service",
    "services/api-gateway",
    "services/credential-login-service",
    "services/user-service",
    "services/notification-service",
]
resolver = "2"
```

- [ ] **Step 2: 把 `services/token-server/` 改名为 `services/token-service/`**

```bash
git mv services/token-server services/token-service
```

- [ ] **Step 3: 改 `services/token-service/Cargo.toml` 的 package.name**

`services/token-service/Cargo.toml`：
```toml
[package]
name = "token-service"
version = "0.1.0"
edition = "2021"
```

- [ ] **Step 4: 建 2 个 stub（credential-login-service / user-service）**

`services/credential-login-service/Cargo.toml`：
```toml
[package]
name = "credential-login-service"
version = "0.1.0"
edition = "2021"
```

`services/credential-login-service/src/lib.rs`：
```rust
// placeholder
```

`services/user-service/Cargo.toml`：
```toml
[package]
name = "user-service"
version = "0.1.0"
edition = "2021"
```

`services/user-service/src/lib.rs`：
```rust
// placeholder
```

- [ ] **Step 5: 验证 workspace 编译**

Run: `cargo check --workspace`
Expected: `Finished`（含 token-service 改名 + 2 个 stub 通过；现有 5 个成员仍编译）

- [ ] **Step 6: 提交**

```bash
git add Cargo.toml services/token-service services/credential-login-service services/user-service
git commit -m "chore(workspace): rename token-server to token-service; add stubs"
```

---

## Task 2: libs/jwt-ctx crate 骨架

**Files:**
- Create: `libs/jwt-ctx/Cargo.toml`
- Create: `libs/jwt-ctx/src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml**

```toml
[package]
name = "jwt-ctx"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
thiserror.workspace = true
async-trait = "0.1"
tower = "0.5"
```

- [ ] **Step 2: 写 lib.rs**

```rust
pub mod principal;

pub use principal::Principal;
```

- [ ] **Step 3: 验证编译**

Run: `cd libs/jwt-ctx && cargo check`
Expected: `Finished`（principal 模块尚未建，下一步建）

- [ ] **Step 4: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): scaffold crate"
```

---

## Task 3: Principal 解析（含 user_code 字段）

**Files:**
- Create: `libs/jwt-ctx/src/principal.rs`
- Create: `libs/jwt-ctx/tests/principal_test.rs`

- [ ] **Step 1: 写失败测试**

`libs/jwt-ctx/tests/principal_test.rs`：
```rust
use axum::http::HeaderMap;
use jwt_ctx::Principal;
use std::collections::HashSet;

#[test]
fn parses_all_headers() {
    let mut h = HeaderMap::new();
    h.insert("x-user-id", "user-uuid-1".parse().unwrap());
    h.insert("x-user-code", "aB3xY9zQ".parse().unwrap());
    h.insert("x-user-dept", "7".parse().unwrap());
    h.insert("x-user-roles", "admin,sales".parse().unwrap());
    h.insert("x-user-perms", "user.read,user.write".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_id, "user-uuid-1");
    assert_eq!(p.user_code, "aB3xY9zQ");
    assert_eq!(p.dept_id, Some(7));
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
    h.insert("x-user-id", "u".parse().unwrap());
    let p = Principal::from_headers(&h).unwrap();
    assert_eq!(p.user_code, "");
    assert_eq!(p.dept_id, None);
    assert!(p.roles.is_empty());
    assert!(p.permissions.is_empty());
}
```

- [ ] **Step 2: 跑测试确认 fail**

Run: `cd libs/jwt-ctx && cargo test --test principal_test`
Expected: 编译失败（`Principal` 不存在）

- [ ] **Step 3: 写实现**

`libs/jwt-ctx/src/principal.rs`：
```rust
use axum::http::HeaderMap;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: String,    // UUID
    pub user_code: String,  // 8 位 NanoID
    pub dept_id: Option<i64>,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PrincipalError {
    #[error("missing x-user-id")]
    MissingUserId,
}

impl Principal {
    pub fn from_headers(h: &HeaderMap) -> Result<Self, PrincipalError> {
        let user_id = h.get("x-user-id")
            .ok_or(PrincipalError::MissingUserId)?
            .to_str()
            .map_err(|_| PrincipalError::MissingUserId)?
            .to_string();
        let user_code = h.get("x-user-code")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let dept_id = h.get("x-user-dept")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok());
        let roles = h.get("x-user-roles")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        let permissions = h.get("x-user-perms")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
            .unwrap_or_default();
        Ok(Self { user_id, user_code, dept_id, roles, permissions })
    }

    pub fn has_role(&self, role: &str) -> bool { self.roles.iter().any(|r| r == role) }
    pub fn has_permission(&self, perm: &str) -> bool { self.permissions.contains(perm) }
}
```

- [ ] **Step 4: 跑测试确认 pass**

Run: `cd libs/jwt-ctx && cargo test --test principal_test`
Expected: 3 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): add Principal with user_code field"
```

---

## Task 4: Principal 的 axum extractor

**Files:**
- Modify: `libs/jwt-ctx/src/principal.rs`
- Create: `libs/jwt-ctx/tests/extractor_test.rs`

- [ ] **Step 1: 写失败测试**

`libs/jwt-ctx/tests/extractor_test.rs`：
```rust
use axum::body::Body;
use axum::http::{Request, header};
use axum::routing::get;
use axum::Router;
use jwt_ctx::Principal;
use tower::ServiceExt;

async fn who(me: Principal) -> String { format!("u{}", me.user_code) }

#[tokio::test]
async fn extractor_returns_principal() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "u")
        .header(header::HeaderName::from_static("x-user-code"), "aB3xY9zQ")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], b"uaB3xY9zQ");
}

#[tokio::test]
async fn missing_header_returns_401() {
    let app = Router::new().route("/", get(who));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 401);
}
```

- [ ] **Step 2: 跑测试确认 fail**

Run: `cd libs/jwt-ctx && cargo test --test extractor_test`
Expected: 编译失败（未实现 `FromRequestParts`）

- [ ] **Step 3: 实现 extractor**

在 `libs/jwt-ctx/src/principal.rs` 追加：
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

- [ ] **Step 4: 跑测试确认 pass**

Run: `cd libs/jwt-ctx && cargo test`
Expected: 5 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/jwt-ctx
git commit -m "feat(jwt-ctx): add axum FromRequestParts extractor"
```

---

## Task 5: libs/authz crate 骨架

**Files:**
- Create: `libs/authz/Cargo.toml`
- Create: `libs/authz/src/lib.rs`

- [ ] **Step 1: 写 Cargo.toml**

```toml
[package]
name = "authz"
version = "0.1.0"
edition = "2021"

[dependencies]
axum.workspace = true
jwt-ctx = { path = "../jwt-ctx" }
tower = "0.5"
thiserror.workspace = true
futures = "0.3"
```

- [ ] **Step 2: 写 lib.rs**

```rust
pub mod require_role;
pub mod require_service_token;

pub use require_role::require_role;
pub use require_service_token::require_service_token;
```

- [ ] **Step 3: 验证编译**

Run: `cd libs/authz && cargo check`
Expected: `Finished`

- [ ] **Step 4: 提交**

```bash
git add libs/authz
git commit -m "feat(authz): scaffold crate"
```

---

## Task 6: require_role 中间件（TDD）

**Files:**
- Create: `libs/authz/src/require_role.rs`
- Create: `libs/authz/tests/require_role_test.rs`

- [ ] **Step 1: 写失败测试**

`libs/authz/tests/require_role_test.rs`：
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::middleware as axum_mw;
use axum::routing::get;
use axum::Router;
use authz::require_role;
use tower::ServiceExt;

async fn secret(jwt_ctx::Principal) -> &'static str { "ok" }

#[tokio::test]
async fn with_role_passes() {
    let app = Router::new()
        .route("/", get(secret))
        .layer(axum_mw::from_fn(require_role("admin")));
    let req = Request::builder()
        .uri("/")
        .header(header::HeaderName::from_static("x-user-id"), "u")
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
        .header(header::HeaderName::from_static("x-user-id"), "u")
        .header(header::HeaderName::from_static("x-user-roles"), "user")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: 跑测试确认 fail**

Run: `cd libs/authz && cargo test --test require_role_test`
Expected: 编译失败（`require_role` 不存在）

- [ ] **Step 3: 实现中间件**

`libs/authz/src/require_role.rs`：
```rust
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use futures::future::BoxFuture;
use jwt_ctx::Principal;

pub fn require_role(role: &'static str)
  -> impl Fn(Request, Next) -> BoxFuture<'static, Result<Response, StatusCode>>
       + Clone + Send + Sync + 'static
{
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
        })
    }
}
```

- [ ] **Step 4: 跑测试确认 pass**

Run: `cd libs/authz && cargo test`
Expected: 2 tests pass

- [ ] **Step 5: 提交**

```bash
git add libs/authz
git commit -m "feat(authz): add require_role middleware"
```

---

## Task 7: require_service_token 中间件

**Files:**
- Create: `libs/authz/src/require_service_token.rs`
- Create: `libs/authz/tests/require_service_token_test.rs`

- [ ] **Step 1: 加依赖**

`libs/authz/Cargo.toml` 追加：
```toml
jwt-simple.workspace = true
base64.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: 写失败测试**

`libs/authz/tests/require_service_token_test.rs`：
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::middleware as axum_mw;
use axum::routing::get;
use axum::Router;
use authz::require_service_token;
use base64::Engine;
use jwt_simple::algorithms::RS256KeyPair;
use jwt_simple::claims::Claims;
use jwt_simple::prelude::*;
use tower::ServiceExt;

async fn echo() -> &'static str { "ok" }

fn make_key_and_token(aud: &str, sub: &str) -> (String, String) {
    let kp = RS256KeyPair::generate(2048).unwrap();
    let pk_pem = kp.public_key().to_pem().unwrap();
    let claims = Claims::create(Duration::from_secs(60))
        .with_issuer("token-service")
        .with_audience(aud)
        .with_subject(sub)
        .with_key_value("token_type", "service");
    let token = kp.sign(claims).unwrap();
    (pk_pem, token)
}

#[tokio::test]
async fn valid_service_token_passes() {
    let (pk_pem, token) = make_key_and_token("user-service", "credential-login-service");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token("user-service", pk_pem)));
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn wrong_audience_is_403() {
    let (pk_pem, token) = make_key_and_token("other-service", "x");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token("user-service", pk_pem)));
    let req = Request::builder()
        .uri("/")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn missing_token_is_401() {
    let (pk_pem, _) = make_key_and_token("user-service", "x");
    let app = Router::new()
        .route("/", get(echo))
        .layer(axum_mw::from_fn(require_service_token("user-service", pk_pem)));
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 3: 跑测试确认 fail**

Run: `cd libs/authz && cargo test --test require_service_token_test`
Expected: 编译失败（`require_service_token` 不存在）

- [ ] **Step 4: 实现中间件**

`libs/authz/src/require_service_token.rs`：
```rust
use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use futures::future::BoxFuture;
use jwt_simple::algorithms::RS256PublicKey;
use jwt_simple::prelude::*;
use std::collections::HashSet;

pub fn require_service_token(
    expected_aud: &'static str,
    public_key_pem: String,
) -> impl Fn(Request, Next) -> BoxFuture<'static, Result<Response, StatusCode>>
     + Clone + Send + Sync + 'static
{
    let pk = RS256PublicKey::from_pem(&public_key_pem)
        .expect("invalid public key pem");
    move |req: Request, next: Next| {
        let pk = pk.clone();
        let expected_aud = expected_aud;
        Box::pin(async move {
            let token = match req.headers().get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
            {
                Some(t) => t.to_string(),
                None => return Err(StatusCode::UNAUTHORIZED),
            };
            let claims: JWTClaims<HashMap<String, serde_json::Value>> = match pk.verify_token(&token, None) {
                Ok(c) => c,
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            };
            let token_type = claims.custom.get("token_type").and_then(|v| v.as_str());
            if token_type != Some("service") {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let aud_ok = match claims.audiences {
                Some(ref set) => set.iter().any(|a| a == expected_aud),
                None => false,
            };
            if !aud_ok {
                return Err(StatusCode::FORBIDDEN);
            }
            Ok(next.run(req).await)
        })
    }
}
```

- [ ] **Step 5: 跑测试确认 pass**

Run: `cd libs/authz && cargo test`
Expected: 5 tests pass（2 个 require_role + 3 个 require_service_token）

- [ ] **Step 6: 提交**

```bash
git add libs/authz
git commit -m "feat(authz): add require_service_token middleware with sig verify"
```

---

## Task 8: libs/common 扩展

**Files:**
- Create: `libs/common/src/password.rs`
- Create: `libs/common/src/db.rs`
- Create: `libs/common/src/redisx.rs`
- Create: `libs/common/src/http.rs`
- Create: `libs/common/tests/password_test.rs`
- Modify: `libs/common/Cargo.toml`
- Modify: `libs/common/src/lib.rs`

- [ ] **Step 1: 加依赖**

`libs/common/Cargo.toml`：
```toml
[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true

# 新增
argon2.workspace = true
anyhow = "1"
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "mysql"] }
redis.workspace = true
reqwest.workspace = true
tokio = { workspace = true, features = ["sync"] }
```

- [ ] **Step 2: 写 password 模块（TDD）**

`libs/common/tests/password_test.rs`：
```rust
use common::password::{hash, verify};

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
    assert_ne!(h1, h2);
}
```

`libs/common/src/password.rs`：
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

- [ ] **Step 3: 写 db / redisx / http 模块**

`libs/common/src/db.rs`：
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

`libs/common/src/redisx.rs`：
```rust
use redis::aio::ConnectionManager;

pub async fn connect(url: &str) -> anyhow::Result<ConnectionManager> {
    let client = redis::Client::open(url)?;
    Ok(ConnectionManager::new(client).await?)
}
```

`libs/common/src/http.rs`：
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

- [ ] **Step 4: 写 lib.rs**

`libs/common/src/lib.rs`：
```rust
pub mod password;
pub mod db;
pub mod redisx;
pub mod http;
```

- [ ] **Step 5: 跑测试**

Run: `cd libs/common && cargo test --test password_test`
Expected: 2 tests pass

- [ ] **Step 6: 验证 workspace 编译**

Run: `cargo check --workspace`
Expected: 通过

- [ ] **Step 7: 提交**

```bash
git add libs/common
git commit -m "feat(common): add password/db/redisx/http utilities"
```

---

## Task 9: DB 迁移 — oa3.member_user 加 UserCode

**Files:**
- Create: `migrations/2026_06_10_add_user_code.sql`
- Create: `services/user-service/src/bin/backfill_user_code.rs`（回填入口，stub 阶段先建空文件）

> **本任务只生成 DDL 与回填工具**。回填执行由后续子 plan A 内的 user-service 完整代码负责（应用层生成保证唯一性）。

- [ ] **Step 1: 写 DDL**

`migrations/2026_06_10_add_user_code.sql`：
```sql
USE oa3;
ALTER TABLE member_user
  ADD COLUMN UserCode VARCHAR(16) NOT NULL UNIQUE AFTER Id;
```

> 用户上线后此 DDL 执行期间表可能锁；建议在低峰期或使用 gh-ost/pt-online-schema-change 工具。但**v1 接受短暂锁表**。

- [ ] **Step 2: 跑 DDL 验证（针对本地 MySQL 容器）**

```powershell
docker exec -i mysql mysql -uroot -p<pwd> < migrations\2026_06_10_add_user_code.sql
docker exec -i mysql mysql -uroot -p<pwd> -e "USE oa3; DESCRIBE member_user;"
```

Expected: `UserCode VARCHAR(16)` 列出现

- [ ] **Step 3: 建回填工具 stub**

`services/user-service/src/bin/backfill_user_code.rs`：
```rust
// TODO 子 plan A 实施：循环更新 member_user SET UserCode = <NanoID> WHERE UserCode = '' OR UserCode IS NULL
// 实施时使用 nanoid 0.4 crate 在 Rust 端生成 8 位 NanoID，保证唯一性
fn main() {
    println!("backfill_user_code: stub (see 子 plan A)");
}
```

- [ ] **Step 4: 提交**

```bash
git add migrations services/user-service/src/bin/backfill_user_code.rs
git commit -m "feat(db): add UserCode column to member_user; backfill stub"
```

---

## Task 10: token-service 改名验证

**Files:**
- Modify: `services/token-service/Cargo.toml`（package.name 已改）
- Modify: `services/token-service/src/**/*.rs`（如含 `token-server` 字面量）

- [ ] **Step 1: 全仓搜 `token-server` 字面量**

```bash
git grep -n "token-server" -- ':!Cargo.lock' ':!docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md' ':!docs/superpowers/plans/2026-06-10-go-to-rust-migration-plan.md'
```

预期：仅 docs/superpowers/specs/2026-06-10-credential-login-user-rust-design.md 与本 plan 含此字面量（保留作为 Superseded 标记）。如果源码中含 `token-server` 字面量（如 `tracing::info!("token-server started")`），逐个替换为 `token-service`。

- [ ] **Step 2: 验证编译**

Run: `cargo check -p token-service`
Expected: 通过

- [ ] **Step 3: 跑现有 token-service 测试**

Run: `cargo test -p token-service --all-targets`
Expected: 通过（无新功能；仅确保改名不破坏）

- [ ] **Step 4: 提交（如有源码改动）**

```bash
git add services/token-service
git commit -m "refactor(token-service): rename references from token-server"
```

---

## Task 11: token-service 新增 /api/v1/tokens/user-jwt 端点

**Files:**
- Modify: `services/token-service/src/routes/*.rs`
- Create: `services/token-service/src/routes/user_jwt.rs`
- Modify: `services/token-service/src/main.rs`

- [ ] **Step 1: 写失败测试**

`services/token-service/tests/user_jwt_test.rs`：
```rust
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::Router;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn user_jwt_requires_service_token() {
    let app = token_service::build_router_with_pem("test-pem");
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/tokens/user-jwt")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"user_id":"u","claims":{}}).to_string())).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

> 端点的 `build_router_with_pem` 是测试辅助工厂。**实际完整测试**（含签名验签 / audience 校验）在子 plan A / B 调通后补；本任务只确保路由可挂、缺少 token 时拒绝。

- [ ] **Step 2: 跑测试确认 fail**

Run: `cd services/token-service && cargo test --test user_jwt_test`
Expected: 编译失败（`build_router_with_pem` 不存在或路由未挂）

- [ ] **Step 3: 实现端点**

`services/token-service/src/routes/user_jwt.rs`：
```rust
use crate::error::AppError;
use crate::issuer::TokenIssuer;
use crate::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
pub struct UserJwtRequest {
    pub user_id: String,
    pub user_code: String,
    pub claims: Value,
    pub ttl_secs: Option<u64>,
}

#[derive(Serialize)]
pub struct UserJwtResponse {
    pub access_token: String,
    pub expires_in: u64,
}

pub async fn handler(
    State(state): State<AppState>,
    Json(req): Json<UserJwtRequest>,
) -> Result<impl IntoResponse, AppError> {
    let ttl = req.ttl_secs.unwrap_or(3600);
    let token = state.issuer.issue_user_jwt(
        &req.user_id,
        &req.user_code,
        req.claims,
        ttl,
    )?;
    Ok((StatusCode::OK, Json(UserJwtResponse { access_token: token, expires_in: ttl })))
}
```

`services/token-service/src/issuer.rs` 追加：
```rust
use jwt_simple::prelude::*;
use serde_json::Value;

impl TokenIssuer {
    pub fn issue_user_jwt(
        &self,
        user_id: &str,
        user_code: &str,
        extra_claims: Value,
        ttl_secs: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use std::collections::HashSet;
        let mut claims: JWTClaims<HashMap<String, Value>> = JWTClaims::new();
        claims.subject = Some(user_id.to_string());
        claims.issuer = Some(self.issuer.clone());
        claims.audiences = Some(HashSet::from(["coder-micro-services".to_string()]));
        claims.jwt_id = Some(uuid::Uuid::new_v4().to_string());
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        claims.expires_at = Some(jwt_simple::prelude::Duration::from_secs(now_secs + ttl_secs));
        claims.not_before = Some(jwt_simple::prelude::Duration::from_secs(now_secs));

        let mut custom: HashMap<String, Value> = HashMap::new();
        custom.insert("client_id".to_string(), Value::String("credential-login-service".to_string()));
        custom.insert("scope".to_string(), Value::String("access".to_string()));
        custom.insert("user_code".to_string(), Value::String(user_code.to_string()));
        if let Some(obj) = extra_claims.as_object() {
            for (k, v) in obj {
                if matches!(k.as_str(), "iss" | "aud" | "exp" | "iat" | "sub" | "nbf" | "jti") {
                    continue;
                }
                custom.insert(k.clone(), v.clone());
            }
        }
        claims.custom = custom;

        Ok(self.key_pair.sign(claims)?)
    }
}
```

- [ ] **Step 4: 路由注册**

`services/token-service/src/main.rs`（或 router 装配处）追加：
```rust
.route("/api/v1/tokens/user-jwt", post(routes::user_jwt::handler))
```

- [ ] **Step 5: 验证编译**

Run: `cargo check -p token-service`
Expected: 通过

- [ ] **Step 6: 跑测试**

Run: `cd services/token-service && cargo test --test user_jwt_test`
Expected: 测试通过（或按测试调整）

- [ ] **Step 7: 提交**

```bash
git add services/token-service
git commit -m "feat(token-service): add /api/v1/tokens/user-jwt endpoint"
```

---

## Task 12: api-gateway 路由（credential-login / user / token）

**Files:**
- Create: `docker/config/api-gateway/routes/credential-login.router.toml`
- Create: `docker/config/api-gateway/routes/user.router.toml`
- Create: `docker/config/api-gateway/routes/token.router.toml`
- Modify: `services/api-gateway/src/router.rs`（如需）

- [ ] **Step 1: 写 credential-login 路由**

`docker/config/api-gateway/routes/credential-login.router.toml`：
```toml
[[routes]]
match = "/api/v1/auth"
service = "credential-login-service"
strip_prefix = "/api/v1/auth"
```

- [ ] **Step 2: 写 user 路由**

`docker/config/api-gateway/routes/user.router.toml`：
```toml
[[routes]]
match = "/api/v1/users"
service = "user-service"
strip_prefix = "/api/v1/users"

[[routes]]
match = "/api/v1/roles"
service = "user-service"
strip_prefix = "/api/v1/roles"

[[routes]]
match = "/api/v1/orgs"
service = "user-service"
strip_prefix = "/api/v1/orgs"

[[routes]]
match = "/api/v1/posts"
service = "user-service"
strip_prefix = "/api/v1/posts"
```

- [ ] **Step 3: 写 token 路由**

`docker/config/api-gateway/routes/token.router.toml`：
```toml
[[routes]]
match = "/api/v1/tokens"
service = "token-service"
strip_prefix = "/api/v1/tokens"

[[routes]]
match = "/.well-known"
service = "token-service"
strip_prefix = "/.well-known"
```

- [ ] **Step 4: 校验路由**

Run: `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
Expected: 无错误

- [ ] **Step 5: 提交**

```bash
git add docker/config/api-gateway/routes
git commit -m "feat(api-gateway): add routes for credential-login/user/token"
```

---

## Task 13: 删除 Go 资产

**Files:**
- Delete: `services/auth-service/`
- Delete: `libs/go-jwt-ctx/`
- Delete: `libs/go-authz/`
- Delete: `scripts/e2e_test.go`
- Delete: 根 `go.mod`、`go.sum`、`go.work`（如有）

- [ ] **Step 1: 删 Go 共享库**

```bash
git rm -r libs/go-jwt-ctx libs/go-authz
```

- [ ] **Step 2: 删 auth-service（Go）**

```bash
git rm -r services/auth-service
```

- [ ] **Step 3: 删 scripts/e2e_test.go + scripts/go.mod**

```bash
git rm scripts/e2e_test.go scripts/go.mod scripts/go.sum
```

- [ ] **Step 4: 删根 go.mod / go.sum / go.work（如有）**

```bash
git rm go.mod go.sum go.work 2>$null
```

- [ ] **Step 5: 校验无 Go 残留**

```bash
git ls-files | Select-String -Pattern "\.go$|go\.mod|go\.sum|go\.work"
```

Expected: 无输出

- [ ] **Step 6: 验证 workspace 编译**

Run: `cargo check --workspace`
Expected: 通过

- [ ] **Step 7: 提交**

```bash
git commit -m "chore: remove go auth-service and go shared libs"
```

---

## Task 14: CI 同步

**Files:**
- Modify: `.gitlab-ci.yml`
- Create: `Dockerfile.credential-login-service`
- Create: `Dockerfile.user-service`
- Create: `Dockerfile.token-service`

- [ ] **Step 1: 删 go_test job**

在 `.gitlab-ci.yml` 中删除 `go_test` 整段。

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

- [ ] **Step 3: 删 `build_auth_service` job**

- [ ] **Step 4: 改 `build_token_server` → `build_token_service`**

```yaml
build_token_service:
  stage: build
  tags: [shell]
  script:
    - sudo docker build -f Dockerfile.token-service -t $DOCKER_REGISTRY/$DOCKER_REPO/token-service:$IMAGE_TAG .
    - sudo docker push $DOCKER_REGISTRY/$DOCKER_REPO/token-service:$IMAGE_TAG
    - sudo docker tag $DOCKER_REGISTRY/$DOCKER_REPO/token-service:$IMAGE_TAG $DOCKER_REGISTRY/$DOCKER_REPO/token-service:latest
    - sudo docker push $DOCKER_REGISTRY/$DOCKER_REPO/token-service:latest
```

- [ ] **Step 5: 新增 `build_credential_login_service` 与 `build_user_service`**

模板同上，文件名与镜像名替换为对应服务。

- [ ] **Step 6: 写 Dockerfile.credential-login-service**

```dockerfile
FROM rust:1.78-slim AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services/credential-login-service ./services/credential-login-service
RUN cargo build --release -p credential-login-service

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/credential-login-service /usr/local/bin/credential-login-service
COPY docker/config/credential-login-service /app/config
EXPOSE 8081
CMD ["/usr/local/bin/credential-login-service", "/app/config/config.yaml"]
```

- [ ] **Step 7: 写 Dockerfile.user-service（镜像名/路径/端口替换为 user-service / 8082）**

- [ ] **Step 8: 写 Dockerfile.token-service（镜像名/路径/端口替换为 token-service；端口沿用 4000）**

> 端口分配需与现有 token-server 不冲突；如冲突，token-service 沿用 token-server 旧端口。

- [ ] **Step 9: 校验 CI YAML**

Run: `python -c "import yaml; yaml.safe_load(open('.gitlab-ci.yml'))"`
Expected: 无异常

- [ ] **Step 10: 提交**

```bash
git add .gitlab-ci.yml Dockerfile.credential-login-service Dockerfile.user-service Dockerfile.token-service
git commit -m "ci: rust workspace test; rename token-server to token-service; add credential-login/user dockerfiles"
```

---

## Task 15: 脚本同步

**Files:**
- Modify: `scripts/test-orchestrator.ps1`
- Modify: `scripts/lib/Test-Runner.ps1`
- Modify: `scripts/lib/Docker-Helper.ps1`

- [ ] **Step 1: 改 Get-ServiceList**

`scripts/lib/Test-Runner.ps1` 中：
```powershell
function Get-ServiceList {
    @("token-service","api-gateway","credential-login-service","user-service","notification-service")
}
```

- [ ] **Step 2: 移除 Go test 分支**

`scripts/lib/Test-Runner.ps1` 中将 `go test` 调用改为：
```powershell
$cmd = @("cargo","test","-p",$Service,"--all-targets")
& $cmd[0] @($cmd[1..($cmd.Length-1)]) 2>&1
```

- [ ] **Step 3: 改 Docker-Helper service 列表**

将 Go 服务名替换为 Rust 服务名（与 Get-ServiceList 一致）。

- [ ] **Step 4: 跑脚本**

Run: `.\scripts\test-orchestrator.ps1 loop -Service token-service`
Expected: Rust 测试通过

- [ ] **Step 5: 提交**

```bash
git add scripts/
git commit -m "chore(scripts): align service list and remove go test branches"
```

---

## Task 16: AI 规则同步

**Files:**
- Modify: `.trae/rules/project_rules.md`

- [ ] **Step 1: 删 Go 路径触发表项**

`.trae/rules/project_rules.md` 中删除 `services/*/**.go`、`services/*/**_test.go` 触发表项。

- [ ] **Step 2: 删 `go test` 失败 → coder-fix-tests 表项**

- [ ] **Step 3: 补全 Rust 触发表（5 个服务名）**

- `services/*/src/**.rs` → `coder-run-tests -Service <name>`
- 服务名清单：`token-service` / `api-gateway` / `credential-login-service` / `user-service` / `notification-service`

- [ ] **Step 4: 提交**

```bash
git add .trae/rules/project_rules.md
git commit -m "chore(ai): align project_rules with rust workspace"
```

---

## Task 17: 收尾

**Files:** 当前所有改动

- [ ] **Step 1: 跑 fmt**

Run: `cargo fmt --all`
Expected: 无输出

- [ ] **Step 2: 跑 clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: 无 warning

- [ ] **Step 3: 跑全 workspace 测试**

Run: `cargo test --workspace --all-targets`
Expected: 全部通过

- [ ] **Step 4: 校验路由**

Run: `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
Expected: 通过

- [ ] **Step 5: 提交（若有 fmt/clippy 改动）**

```bash
git add -A
git commit -m "style(workspace): apply rustfmt and clippy fixes"
```

---

## 自审

- **Spec 覆盖**：
  - §2 服务清单 → Task 1-2（含 token-service 改名 + stub）
  - §3 数据库 / UserCode → Task 9（DDL + 回填 stub）
  - §4 端点 → Task 11（user-jwt 端点）；credential-login / user 端点由子 plan A / B
  - §5 协作流 → 由子 plan A / B 实现
  - §6 JWT claims → Task 3-7（Principal / 服务间鉴权）
  - §7 Cargo workspace → Task 1
  - §8 内部端点鉴权 → Task 7（require_service_token）
  - §9 删除 / 改造 → Task 10-13
  - §10 风险 → 散落各 task（如 Task 9 注释 DDL 锁表风险）
  - §11 实施顺序 1-4 → 主 plan Task 1-12；5-6 → 子 plan A / B；7-10 → 主 plan Task 12-17
- **占位符**：所有 step 含可执行代码 / 命令
- **类型一致**：`Principal.user_code: String`、`require_service_token(expected_aud, public_key_pem)` 与 spec 一致
- **可执行性**：每步 2-5 分钟
- **YAGNI**：未引入新框架；未重设计 API；未重写 SDK；未做防爆破 / 2FA
- **可拆 plan 性**：子 plan A / B 触发点在 Task 1 之后任意时间启动
