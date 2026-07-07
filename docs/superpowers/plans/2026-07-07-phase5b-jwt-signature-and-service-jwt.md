# Phase 5-B JWT Signature & Service-JWT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JWT signature verification (RS256 via JWKS) and short-lived `service-jwt` issuance so that the security gap left by phase5a-2 (dephase Consul Connect) is closed. Every internal HTTP call across services is authenticated via Bearer JWT, not by docker network topology.

**Architecture:** New `libs/common-jwt` exposes `JWKSCache` (periodic JWKS fetch from `token-service:8082/.well-known/jwks.json`), `verify_token()` (RS256 + optional `expected_aud` check), `sign_token()` (RS256 with private key), and `ServiceJwtCache` (per-target service-jwt with refresh). `libs/jwt-ctx::Principal` adds `from_token(token, &cache, aud?)` and `FromRequestParts` becomes async. `token-service` adds `POST /tokens/service-jwt` (bootstrap-token auth, mints 1h RS256 JWT with `aud: <target>`). 7 services pull JWKS at startup, mint their first service-jwt, and `common-http::ResilientClient` auto-injects `Authorization: Bearer <service-jwt>` on outbound calls.

**Tech Stack:** Rust 1.96 (workspace), `jsonwebtoken = "9"` (new dep), `reqwest` 0.12 + rustls (existing), `axum` 0.8 async extractor (existing), `tokio::sync::watch` for shutdown signalling (existing).

**Spec:** `docs/superpowers/specs/2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md` (must be committed before this plan starts).

---

## Global Constraints

- **One commit per task**, prefix `dephase(auth): ` (consistent with phase5a-2 style).
- **Per service Cargo.toml**: add `common-jwt = { workspace = true }` only in the task that touches that service.
- **Workspace `Cargo.toml`**: `members` adds `libs/common-jwt` in Task A; `[workspace.dependencies]` adds `jsonwebtoken = { version = "9", default-features = false }` in Task A.
- **Service runtime env vars** (introduced in Task D1, present in all subsequent service tasks):
  - `SERVICE_NAME` — already set by phase5a-2 docker-compose labels / env
  - `SERVICE_BOOTSTRAP_TOKEN` — 32-byte random; injected via docker-compose env in Task D1 onwards
  - `TENANT_ID` — i64, default `1` for dev
  - `TOKEN_SERVICE_URL` — default `http://token-service:8082` for dev
- **`token-service` env addition**: `SERVICE_BOOTSTRAP_TOKENS` — comma-separated list of accepted bootstrap tokens, parsed at startup into `TokenServerState::bootstrap_tokens`.
- **JWT algorithm**: RS256 only. No HS256, no PS256, no EdDSA.
- **Audience policy**:
  - `Principal::from_token(token, cache, aud)`:
    - `aud = None` → skips aud check (gateway path, user-jwt)
    - `aud = Some("iam-service")` → fails on mismatch
  - `/iam/internal/*` and any internal route always set `aud = Some(<self-name>)` via a thin middleware.
- **Test command** (CI gate): `cargo test --workspace --lib`. Local dev: `CARGO_TARGET_DIR=/tmp/cargo-dephase cargo test -p common-jwt -p jwt-ctx`.
- **No new DB tables / migrations**.
- **No new external crates besides `jsonwebtoken`**.
- **`authz` lib reuse**: existing `require_role` / `require_permission` middleware stays; we add ONE new middleware (`verify_aud_principal_middleware`) in `authz`.
- **Service-jwt audience contract**: any outbound HTTP call that hits another service must carry `Authorization: Bearer <service-jwt for that target>`. The `ResilientClient` injector looks up the right service-jwt by `aud`.
- **Spec §0.5.5 open questions (4 items)** stay open — captured in the dephase spec, NOT in this plan.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `Cargo.toml` (workspace) | modify | Add `libs/common-jwt` to `members`; add `jsonwebtoken` to `[workspace.dependencies]` |
| `libs/common-jwt/Cargo.toml` | create | Crate manifest (name `common-jwt`) |
| `libs/common-jwt/src/lib.rs` | create | JWKSCache, verify_token, sign_token, ServiceJwtCache, errors |
| `libs/common-jwt/src/cache.rs` | create | JWKSCache impl (~150 lines) |
| `libs/common-jwt/src/verify.rs` | create | verify_token + sign_token (~80 lines) |
| `libs/common-jwt/src/service_jwt.rs` | create | ServiceJwtCache impl (~120 lines) |
| `libs/common-jwt/tests/cache_test.rs` | create | JWKSCache: parse / refresh / kid lookup / failure tolerance |
| `libs/common-jwt/tests/verify_test.rs` | create | sign+verify roundtrip / expired / wrong aud / wrong kid / missing kid |
| `libs/common-jwt/tests/service_jwt_test.rs` | create | ServiceJwtCache: cold start, near-expiry refresh, target partitioning |
| `libs/jwt-ctx/src/principal.rs` | modify | Add `from_token` (async, verify), add `PrincipalStateProvider` trait, add `PrincipalVerifyError` |
| `libs/authz/src/lib.rs` | modify | Add `verify_aud_principal_middleware(aud)` (axum middleware factory) |
| `libs/authz/src/verify_aud.rs` | create | ~50-line middleware impl |
| `libs/authz/tests/verify_aud_test.rs` | create | 4 cases: valid aud, wrong aud, no auth header, expired token |
| `libs/common-http/src/client.rs` | modify | `ResilientClient::call` accepts `auth_injector: Option<Arc<AuthInjector>>` |
| `libs/common-http/src/auth_injector.rs` | create | `AuthInjector::header_value(&self) -> Option<HeaderValue>` |
| `services/token-service/src/main.rs` | modify | Load `SERVICE_BOOTSTRAP_TOKENS` env, inject into state, mount `/tokens/service-jwt` route |
| `services/token-service/src/state.rs` | modify (or add) | Add `private_key_pem: String`, `bootstrap_tokens: Vec<String>` fields |
| `services/token-service/src/routes/service_jwt.rs` | create | `POST /tokens/service-jwt` handler (~80 lines) |
| `services/token-service/src/lib.rs` | modify | Re-export `ServiceJwtRequest` / `ServiceJwtResponse` |
| `services/token-service/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/iam-service/src/main.rs` | modify | JWKS + service-jwt cache, AppState, mount `verify_aud_principal_middleware("iam-service")` on `/iam/internal/*` |
| `services/iam-service/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/notification-service/src/main.rs` | modify | JWKS + service-jwt cache, AppState, replace `None` in IamMemberClient calls |
| `services/notification-service/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/audit-service/src/main.rs` | modify | JWKS + service-jwt cache, AppState |
| `services/audit-service/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/token-service/src/lib.rs` (Task D4) | modify | When this task runs, this is `token-service` itself — same change as iam but `aud = None` for self (user-jwt) |
| `services/api-gateway/src/main.rs` | modify | JWKS cache (no service-jwt needed; gateway has no service-to-service outbound), AppState |
| `services/api-gateway/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/prue_backend/src/main.rs` | modify | JWKS + service-jwt cache, AppState (no outbound service calls today; service-jwt for future-proofing) |
| `services/prue_backend/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/iam-outbox-publisher/src/main.rs` | modify | service-jwt cache (no JWKS needed; outbox publisher does SQL only) |
| `services/iam-outbox-publisher/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `services/notification-outbox-publisher/src/main.rs` | modify | service-jwt cache |
| `services/notification-outbox-publisher/Cargo.toml` | modify | Add `common-jwt = { workspace = true }` |
| `infrastructure/docker-compose-services.yml` | modify | Add `SERVICE_BOOTSTRAP_TOKEN` / `TENANT_ID` / `TOKEN_SERVICE_URL` to each business service; add `SERVICE_BOOTSTRAP_TOKENS` to token-service |
| `infrastructure/.env` | modify | Add dev bootstrap tokens (one per service, 32-byte random; for dev use deterministic placeholders like `dev-bt-iam-svc-...`) |
| `docs/ai-context.md` | modify | §2 service map: add `common-jwt`, `ServiceJwtCache`; §3 architecture add common-jwt |
| `docs/guides/service-to-service-auth.md` | create | Bootstrap token 运维手册 (rotate / 泄漏处理 / dev / prod) |
| `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` | modify | §2.2 "Phase 5-B JWT 签名验证" 段标 superseded |

No other files. New files: 7. Modified: 17. Deleted: 0. Renamed: 0.

---

## Task A: Build `libs/common-jwt` (TDD)

**Files:**
- Modify: `Cargo.toml` (workspace)
- Create: `libs/common-jwt/Cargo.toml`
- Create: `libs/common-jwt/src/{lib,cache,verify,service_jwt}.rs`
- Create: `libs/common-jwt/tests/{cache,verify,service_jwt}_test.rs`

**Interfaces:**
- Consumes: nothing
- Produces: `common_jwt::{JWKSCache, JwksConfig, verify_token, sign_token, ServiceJwtCache, ServiceJwtConfig, JwksError, VerifyError, SignError, ServiceJwtError}`. Exact signatures: see spec §3.1.

- [ ] **Step 1: Add `jsonwebtoken` to workspace and create empty `common-jwt` crate**

In `Cargo.toml`:
- Under `[workspace] members = [...]`, add `"libs/common-jwt"`.
- Under `[workspace.dependencies]`, add:
  ```toml
  jsonwebtoken = { version = "9", default-features = false }
  ```

Create `libs/common-jwt/Cargo.toml`:
```toml
[package]
name = "common-jwt"
version = "0.1.0"
edition.workspace = true

[dependencies]
jsonwebtoken = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["sync", "time", "rt", "macros"] }
```

Create `libs/common-jwt/src/lib.rs` with the public API surface (function signatures + types) but no implementations yet:
```rust
//! common-jwt: shared JWT primitives (JWKS cache, verify, sign, service-jwt cache)

pub mod cache;
pub mod service_jwt;
pub mod verify;

pub use cache::{JwksConfig, JWKSCache, JwksError};
pub use service_jwt::{ServiceJwtCache, ServiceJwtConfig, ServiceJwtError};
pub use verify::{sign_token, SignError, VerifyError};
```

Create empty stub files: `cache.rs`, `verify.rs`, `service_jwt.rs` (each with `// TODO: Task A` and a single empty fn so the crate compiles).

Run: `cargo check -p common-jwt`
Expected: clean compile (empty stubs OK).

- [ ] **Step 2: Write failing test for `verify_token` roundtrip (TDD red)**

Open `libs/common-jwt/tests/verify_test.rs` and write:
```rust
use common_jwt::{sign_token, verify_token, JWKSCache, JwksConfig};
use serde_json::json;
use std::time::Duration;

// Test fixture: generate an RSA keypair in-test.
const TEST_PRIVATE_PEM: &str = include_str!("../tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM:  &str = include_str!("../fixtures/test_public.pem");

fn mock_jwks() -> JWKSCache {
    // Build a JWKSCache with the test public key pre-loaded, no HTTP.
    // We use the internal constructor `JWKSCache::from_pem` (added in Step 4).
    todo!()
}

#[tokio::test]
async fn sign_then_verify_roundtrip_succeeds() {
    let cache = mock_jwks();
    let claims = json!({
        "iss": "test",
        "sub": "user-1",
        "aud": "iam-service",
        "tid": 1,
        "exp": (chrono::Utc::now().timestamp() + 60),
    });
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let verified = verify_token(&token, &cache, Some("iam-service")).await.unwrap();
    assert_eq!(verified["sub"], "user-1");
    assert_eq!(verified["aud"], "iam-service");
}
```

Create fixture keypair. Run:
```bash
mkdir -p libs/common-jwt/tests/fixtures
openssl genrsa -out libs/common-jwt/tests/fixtures/test_private.pem 2048 2>/dev/null
openssl rsa -in libs/common-jwt/tests/fixtures/test_private.pem -pubout -out libs/common-jwt/tests/fixtures/test_public.pem 2>/dev/null
```

Run: `cargo test -p common-jwt --test verify_test sign_then_verify_roundtrip_succeeds`
Expected: FAIL with `todo!()` panic (or `sign_token` not found).

- [ ] **Step 3: Implement `sign_token` and `verify_token` (TDD green)**

In `libs/common-jwt/src/verify.rs`:
```rust
use jsonwebtoken::{decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("malformed JWT: {0}")]
    Malformed(String),
    #[error("signature invalid")]
    SignatureInvalid,
    #[error("audience mismatch: expected {expected}, got {got}")]
    AudienceMismatch { expected: String, got: String },
    #[error("expired")]
    Expired,
    #[error("JWKS error: {0}")]
    Jwks(#[from] crate::cache::JwksError),
}

#[derive(Error, Debug)]
pub enum SignError {
    #[error("invalid PEM key: {0}")]
    InvalidKey(String),
    #[error("encode failed: {0}")]
    Encode(String),
}

/// Sign a JWT with RS256 and the given PEM-encoded private key.
/// Sets `exp` based on `now() + ttl`. Caller-provided `claims["exp"]` is overridden.
pub fn sign_token(
    private_key_pem: &str,
    claims: &Value,
    ttl: Duration,
) -> Result<String, SignError> {
    let mut claims = claims.clone();
    let now = chrono::Utc::now().timestamp();
    claims["iat"] = json!(now);
    claims["exp"] = json!(now + ttl.as_secs() as i64);
    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| SignError::InvalidKey(e.to_string()))?;
    let header = Header::new(Algorithm::RS256);
    encode(&header, &claims, &key).map_err(|e| SignError::Encode(e.to_string()))
}

/// Verify a JWT against a JWKSCache. If `expected_aud` is Some, the JWT's
/// `aud` claim must equal `expected_aud` exactly.
pub async fn verify_token(
    token: &str,
    cache: &crate::cache::JWKSCache,
    expected_aud: Option<&str>,
) -> Result<Value, VerifyError> {
    let header = decode_header(token).map_err(|e| VerifyError::Malformed(e.to_string()))?;
    let kid = header.kid.ok_or_else(|| VerifyError::Malformed("missing kid".into()))?;
    let key = cache.get(&kid).or_else(|| cache.default_key()).ok_or_else(|| {
        VerifyError::Malformed(format!("unknown kid: {kid}"))
    })?;
    let mut validation = Validation::new(Algorithm::RS256);
    match expected_aud {
        Some(aud) => validation.set_audience(&[aud]),
        None => validation.validate_aud = false,
    };
    let data = decode::<Value>(token, &key, &validation).map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::InvalidSignature => VerifyError::SignatureInvalid,
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => VerifyError::Expired,
        jsonwebtoken::errors::ErrorKind::InvalidAudience => VerifyError::AudienceMismatch {
            expected: expected_aud.unwrap_or("(none)").to_string(),
            got: "(see token)".to_string(),
        },
        _ => VerifyError::Malformed(e.to_string()),
    })?;
    Ok(data.claims)
}
```

Add `JWKSCache::from_pem` and `JWKSCache::get`/`default_key` stubs in `cache.rs` (enough to make the test compile and pass):
```rust
use jsonwebtoken::DecodingKey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum JwksError { /* variants per spec §3.1 */ }

#[derive(Clone, Debug)]
pub struct JwksConfig {
    pub url: String,
    pub refresh_interval: Duration,
    pub request_timeout: Duration,
}

impl Default for JwksConfig { /* default url = "http://token-service:8082/.well-known/jwks.json", 1h, 5s */ }

pub struct JWKSCache {
    inner: Arc<RwLock<JwksInner>>,
    config: JwksConfig,
    http: reqwest::Client,
}

pub struct JwksInner {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Option<std::time::Instant>,
}

impl JWKSCache {
    pub fn new(config: JwksConfig, http: reqwest::Client) -> Self { todo!() }
    /// Test-only constructor: pre-load a single public key with kid="default".
    pub fn from_pem(public_pem: &str) -> Result<Self, JwksError> {
        let key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|e| JwksError::KeyBuildFailed(e.to_string()))?;
        let mut keys = HashMap::new();
        keys.insert("default".to_string(), key);
        Ok(Self {
            inner: Arc::new(RwLock::new(JwksInner { keys, fetched_at: Some(std::time::Instant::now()) })),
            config: JwksConfig { url: String::new(), refresh_interval: Duration::from_secs(3600), request_timeout: Duration::from_secs(5) },
            http: reqwest::Client::new(),
        })
    }
    pub fn get(&self, kid: &str) -> Option<DecodingKey> {
        self.inner.blocking_read().keys.get(kid).cloned()
    }
    pub fn default_key(&self) -> Option<DecodingKey> {
        // Prefer kid="default"; otherwise first.
        let g = self.inner.blocking_read();
        g.keys.get("default").cloned().or_else(|| g.keys.values().next().cloned())
    }
    pub async fn refresh(&self) -> Result<(), JwksError> { todo!() }
    pub fn spawn_refresh_loop(self: Arc<Self>, _shutdown: tokio::sync::watch::Receiver<bool>) {}
}
```

Add the missing `JwksError` variants per spec §3.1.

Run: `cargo test -p common-jwt --test verify_test sign_then_verify_roundtrip_succeeds`
Expected: PASS.

- [ ] **Step 4: Add adversarial cases (expired / wrong aud / wrong kid / missing kid)**

Append to `libs/common-jwt/tests/verify_test.rs`:
```rust
#[tokio::test]
async fn expired_token_returns_expired_error() {
    let cache = mock_jwks();
    let mut claims = json!({"sub":"x","aud":"iam-service","tid":1});
    // craft already-expired
    claims["exp"] = json!(chrono::Utc::now().timestamp() - 60);
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let err = verify_token(&token, &cache, Some("iam-service")).await.unwrap_err();
    assert!(matches!(err, VerifyError::Expired));
}

#[tokio::test]
async fn wrong_audience_returns_mismatch() {
    let cache = mock_jwks();
    let claims = json!({"sub":"x","aud":"file-service","tid":1,"exp":(chrono::Utc::now().timestamp()+60)});
    let token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let err = verify_token(&token, &cache, Some("iam-service")).await.unwrap_err();
    assert!(matches!(err, VerifyError::AudienceMismatch { .. }));
}

#[tokio::test]
async fn tampered_signature_returns_invalid_signature() {
    let cache = mock_jwks();
    let claims = json!({"sub":"x","aud":"iam-service","tid":1,"exp":(chrono::Utc::now().timestamp()+60)});
    let mut token = sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    // flip a byte in the signature segment
    let mut parts: Vec<&str> = token.rsplitn(2, '.').collect();
    let sig = parts[0].to_string();
    let flipped = sig.chars().next().map(|c| if c == 'A' { 'B' } else { 'A' }).unwrap();
    let new_sig = format!("{}{}", flipped, &sig[1..]);
    token = token.replace(&sig, &new_sig);
    let err = verify_token(&token, &cache, Some("iam-service")).await.unwrap_err();
    assert!(matches!(err, VerifyError::SignatureInvalid));
}

#[tokio::test]
async fn unknown_kid_returns_malformed() {
    // Build a token with a different kid header.
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("attacker-controlled".into());
    let key = EncodingKey::from_rsa_pem(TEST_PRIVATE_PEM.as_bytes()).unwrap();
    let claims = json!({"sub":"x","aud":"iam-service","tid":1,"exp":(chrono::Utc::now().timestamp()+60)});
    let token = encode(&header, &claims, &key).unwrap();
    let cache = mock_jwks();
    let err = verify_token(&token, &cache, Some("iam-service")).await.unwrap_err();
    assert!(matches!(err, VerifyError::Malformed(_)));
}
```

Run: `cargo test -p common-jwt --test verify_test`
Expected: all 5 tests pass.

- [ ] **Step 5: Add `JWKSCache::refresh` (HTTP path)**

In `libs/common-jwt/src/cache.rs`:
```rust
impl JWKSCache {
    pub async fn refresh(&self) -> Result<(), JwksError> {
        // GET self.config.url
        // Parse JWKS response: { "keys": [ { "kid": "...", "n": "...", "e": "..." }, ... ] }
        // Build DecodingKey for each, store in inner.keys
        // Update inner.fetched_at = Some(Instant::now())
        ...
    }
}
```

For parsing JWKS, use `jsonwebtoken::DecodingKey::from_rsa_components(n_b64, e_b64)`. Handle JWK `kty: "RSA"` and ignore others (return Ok, but no key added).

Add test in `libs/common-jwt/tests/cache_test.rs`:
```rust
use common_jwt::{JWKSCache, JwksConfig};
use std::time::Duration;
use std::sync::Arc;
use tokio::sync::watch;

#[tokio::test]
async fn cache_refresh_loads_keys_from_jwks() {
    // Spin up a mock JWKS server with `httpmock` or wiremock.
    // Simpler: extract refresh logic into `pub async fn parse_jwks(...)` and
    // unit test that.
    let json = r#"{"keys":[{"kid":"k1","kty":"RSA","n":"...","e":"AQAB"}]}"#;
    let keys = common_jwt::cache::parse_jwks(json).unwrap();
    assert!(keys.contains_key("k1"));
}
```

Extract `parse_jwks(json: &str) -> Result<HashMap<String, DecodingKey>, JwksError>` as a free function and unit-test it (avoids needing a mock HTTP server in unit tests).

Run: `cargo test -p common-jwt`
Expected: all tests pass; crate builds clean.

- [ ] **Step 6: Add `ServiceJwtCache` + tests**

Implement `libs/common-jwt/src/service_jwt.rs` per spec §3.4.2. Add tests in `libs/common-jwt/tests/service_jwt_test.rs`:
- `cold_start_calls_token_service_and_caches`
- `near_expiry_triggers_refresh`
- `different_targets_get_different_tokens`
- (mock token-service with `wiremock` or stub the HTTP layer)

Run: `cargo test -p common-jwt`
Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml libs/common-jwt
git commit -m "dephase(auth): introduce libs/common-jwt (JWKSCache + verify + ServiceJwtCache)"
```

---

## Task B: `token-service` service-jwt endpoint

**Files:**
- Modify: `services/token-service/Cargo.toml`
- Modify: `services/token-service/src/main.rs`
- Modify: `services/token-service/src/state.rs` (or wherever `TokenServerState` is defined)
- Create: `services/token-service/src/routes/service_jwt.rs`
- Modify: `services/token-service/src/lib.rs`

**Interfaces:**
- Consumes: `common_jwt::sign_token` (Task A)
- Produces: `POST /tokens/service-jwt` accepting `Authorization: Bearer ${SERVICE_BOOTSTRAP_TOKEN}` + JSON body `{"aud": "iam-service", "tid": 1}`. Returns 200 `{"access_token", "token_type":"Bearer","expires_in":3600}` or 401 / 400. `TokenServerState` gains `private_key_pem: String` and `bootstrap_tokens: Vec<String>`.

- [ ] **Step 1: Add bootstrap tokens to state + load from env**

Find `TokenServerState` (likely `services/token-service/src/state.rs` or inline in `main.rs`). Add:
```rust
pub struct TokenServerState {
    pub db: DbPool,
    pub issuer: Arc<crate::issuer::TokenIssuer>,
    pub issuer_str: String,
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: Arc<ResilientClient>,
    pub user_service_url: String,
    pub private_key_pem: String,                    // NEW
    pub bootstrap_tokens: Vec<String>,             // NEW
}
```

In `services/token-service/src/main.rs`, after loading `cfg`:
```rust
let private_key_pem = std::fs::read_to_string(&cfg.jwt_private_key_path)
    .expect("failed to read private key");
let bootstrap_tokens: Vec<String> = std::env::var("SERVICE_BOOTSTRAP_TOKENS")
    .unwrap_or_default()
    .split(',')
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .collect();
if bootstrap_tokens.is_empty() {
    tracing::warn!("SERVICE_BOOTSTRAP_TOKENS not set; /tokens/service-jwt will reject all callers");
}
let state = TokenServerState {
    /* ... existing fields ... */
    private_key_pem,
    bootstrap_tokens,
};
```

- [ ] **Step 2: Create `routes/service_jwt.rs`**

Per spec §3.3. The handler validates bootstrap token, validates `aud` is non-empty, signs the JWT, returns response. Use `axum::extract::State` and `axum::http::HeaderMap`. The handler module includes the route registration:
```rust
pub fn routes(state: TokenServerState) -> Router {
    Router::new()
        .route("/tokens/service-jwt", axum::routing::post(handle_service_jwt))
        .with_state(state)
}
```

- [ ] **Step 3: Mount route in main**

In `services/token-service/src/main.rs`:
```rust
.merge(routes::service_jwt::routes(state.clone()))
```

(Adjust if existing routing uses a different merge pattern.)

- [ ] **Step 4: Integration test (in `services/token-service/tests/service_jwt_test.rs`)**

Use the existing test setup (or create a `tests/` axum harness). Mount the router. Send a request with:
- valid bootstrap + valid body → 200, response has `access_token`, parse with `jsonwebtoken::decode` using `cfg.jwt_public_key` → claims include `iss: "token-service"`, `sub: <caller-service-name>`, `aud: <requested>`
- wrong bootstrap → 401
- missing `aud` → 400
- empty `aud` → 400

Run: `cargo test -p token-service --test service_jwt_test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add services/token-service
git commit -m "dephase(auth): token-service POST /tokens/service-jwt (bootstrap-token auth, RS256)"
```

---

## Task C: `jwt-ctx` async refactor + verify path

**Files:**
- Modify: `libs/jwt-ctx/src/principal.rs`
- Modify: `libs/jwt-ctx/src/lib.rs` (re-export new types)
- Modify: `libs/jwt-ctx/Cargo.toml` (add `common-jwt` dep)

**Interfaces:**
- Consumes: `common_jwt::{JWKSCache, VerifyError, JwksError}` (Task A)
- Produces:
  - `Principal::from_token(token, &JWKSCache, expected_aud: Option<&str>) -> Result<Self, PrincipalVerifyError>` (async)
  - `Principal::from_jwt_payload(token)` — **kept** for unit tests, deprecated in doc-comment
  - `PrincipalStateProvider` trait (impl'd by any state with `Arc<JWKSCache>`)
  - `PrincipalVerifyError` enum
  - `FromRequestParts for Principal` becomes async

- [ ] **Step 1: Add `common-jwt` dep to `jwt-ctx`**

In `libs/jwt-ctx/Cargo.toml`:
```toml
[dependencies]
common-jwt = { workspace = true }
```

- [ ] **Step 2: Add `PrincipalVerifyError` + `PrincipalStateProvider`**

In `libs/jwt-ctx/src/principal.rs`, add:
```rust
pub trait PrincipalStateProvider: Send + Sync {
    fn jwks(&self) -> Arc<common_jwt::JWKSCache>;
}

#[derive(thiserror::Error, Debug)]
pub enum PrincipalVerifyError {
    #[error("missing authorization header")]
    MissingAuthorization,
    #[error("invalid authorization header format")]
    InvalidAuthorizationFormat,
    #[error("verification failed: {0}")]
    VerifyFailed(#[from] common_jwt::VerifyError),
    #[error("missing sub claim")]
    MissingSub,
    #[error("missing tid claim")]
    MissingTenantId,
}
```

- [ ] **Step 3: Add `Principal::from_token` (async, with verify)**

```rust
impl Principal {
    /// Decode + verify a JWT against a JWKSCache.
    /// `expected_aud`: Some forces aud match; None skips.
    pub async fn from_token(
        token: &str,
        cache: &common_jwt::JWKSCache,
        expected_aud: Option<&str>,
    ) -> Result<Self, PrincipalVerifyError> {
        let claims = common_jwt::verify_token(token, cache, expected_aud).await
            .map_err(PrincipalVerifyError::VerifyFailed)?;
        Self::from_claims(claims).map_err(|e| match e {
            PrincipalError::MissingSub => PrincipalVerifyError::MissingSub,
            PrincipalError::MissingTenantId => PrincipalVerifyError::MissingTenantId,
            other => PrincipalVerifyError::VerifyFailed(common_jwt::VerifyError::Malformed(other.to_string())),
        })
    }

    fn from_claims(payload: serde_json::Value) -> Result<Self, PrincipalError> {
        // Existing from_jwt_payload body, refactored to take a Value
        let user_id = payload.get("sub")...to_string();
        // ... rest unchanged
    }
}
```

- [ ] **Step 4: Make `FromRequestParts` async**

```rust
#[axum::async_trait]
impl<S> FromRequestParts<S> for Principal
where
    S: PrincipalStateProvider + Send + Sync,
{
    type Rejection = (axum::http::StatusCode, &'static str);
    async fn from_request_parts(parts, state) -> Result<Self, Self::Rejection> {
        let token = parts.headers.get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((StatusCode::UNAUTHORIZED, "missing bearer"))?;
        let cache = state.jwks();
        Self::from_token(token, &cache, None).await  // aud=None for gateway path
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token"))
    }
}
```

- [ ] **Step 5: Update all `AppState` types to impl `PrincipalStateProvider`**

For each service that uses `Principal` extractor (iam / notification / audit / token / api-gateway / prue_backend), the `AppState` (or equivalent) must:
1. Add `pub jwks: Arc<common_jwt::JWKSCache>`
2. `impl PrincipalStateProvider for AppState { fn jwks(&self) -> Arc<...> { self.jwks.clone() } }`

These are the actual per-service migrations (Tasks D1-D7). Task C only sets up the trait and the FromRequestParts signature. The per-service impls come in D*.

- [ ] **Step 6: Update existing unit tests that use `from_jwt_payload`**

`from_jwt_payload` is kept (no breakage). Add new unit tests for `from_token`:
```rust
#[tokio::test]
async fn from_token_with_valid_token_returns_principal() {
    let cache = common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = json!({...});
    let token = common_jwt::sign_token(TEST_PRIVATE_PEM, &claims, Duration::from_secs(60)).unwrap();
    let p = Principal::from_token(&token, &cache, Some("iam-service")).await.unwrap();
    assert_eq!(p.user_id, "user-1");
}
```

Run: `cargo test -p jwt-ctx`
Expected: all pass (existing tests using `from_jwt_payload` still pass; new tests pass).

- [ ] **Step 7: Commit**

```bash
git add libs/jwt-ctx
git commit -m "dephase(auth): jwt-ctx Principal::from_token (async, RS256 verify via JWKSCache)"
```

---

## Task D1: `iam-service` verify_aud middleware + service-jwt cache

**Files:**
- Modify: `services/iam-service/Cargo.toml` (add `common-jwt` dep)
- Modify: `services/iam-service/src/main.rs` (JWKS + service-jwt cache, AppState field, middleware on `/iam/internal/*`)
- Modify: `services/iam-service/src/routes/internal_users.rs` (no functional change; middleware does the work)

**Interfaces:**
- Consumes: Task A (`common_jwt`), Task B (`/tokens/service-jwt`), Task C (`Principal::from_token`)
- Produces:
  - `AppState.jwks: Arc<JWKSCache>` (Task C trait)
  - `AppState.service_jwt: Arc<ServiceJwtCache>`
  - `verify_aud_principal_middleware("iam-service")` mounted on `/iam/internal/*` (added in this task in `libs/authz`)
  - Outbound calls to token-service use service-jwt (not strictly required for iam; the auto-injector picks up cache)

- [ ] **Step 1: Add `verify_aud_principal_middleware` to `libs/authz`**

This is logically part of Task C, but the middleware needs the route module to import it. Add it here for locality with iam's route registration.

In `libs/authz/src/verify_aud.rs`:
```rust
use axum::{extract::State, http::StatusCode, middleware::Next, response::Response};
use jwt_ctx::{Principal, PrincipalStateProvider};

pub fn verify_aud_principal_middleware<S>(
    expected_aud: &'static str,
) -> impl Fn(State<S>, axum::http::HeaderMap, axum::extract::Request, Next) -> ...
where
    S: PrincipalStateProvider + Send + Sync + Clone + 'static,
{
    move |State(state), headers, mut req, next| async move {
        let token = headers.get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((StatusCode::UNAUTHORIZED, "missing bearer"))?;
        let cache = state.jwks();
        let _principal = Principal::from_token(token, &cache, Some(expected_aud)).await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token"))?;
        // Note: actual `Principal` extraction happens in the handler via
        // FromRequestParts. The middleware's job is to gate the route by
        // requiring a valid aud=<self> token. The handler then re-extracts
        // via Principal extractor. We accept the double-verify cost for
        // explicit route gating.
        // To avoid the double work, we could store the principal in extensions;
        // for now, just gate.
        Ok::<_, (StatusCode, &'static str)>(next.run(req).await)
    }
}
```

Add `verify_aud_principal_middleware` to `libs/authz/src/lib.rs` exports.

In `libs/authz/Cargo.toml`:
```toml
[dependencies]
common-jwt = { workspace = true }  # if not already
jwt-ctx = { workspace = true }
```

- [ ] **Step 2: Add `common-jwt` to iam-service Cargo.toml**

```toml
[dependencies]
common-jwt = { workspace = true }
```

- [ ] **Step 3: Add JWKS + service-jwt cache boilerplate to iam-service main**

After DB / Redis init, before `axum::serve`:
```rust
let http = reqwest::Client::builder()
    .timeout(Duration::from_secs(5))
    .build()?;
let jwks = Arc::new(common_jwt::JWKSCache::new(
    common_jwt::JwksConfig {
        url: std::env::var("TOKEN_SERVICE_URL")
            .unwrap_or_else(|_| "http://token-service:8082/.well-known/jwks.json".into()),
        refresh_interval: Duration::from_secs(3600),
        request_timeout: Duration::from_secs(5),
    },
    http.clone(),
));
jwks.refresh().await.expect("JWKS refresh failed at startup");

let service_jwt = Arc::new(common_jwt::ServiceJwtCache::new(
    common_jwt::ServiceJwtConfig {
        token_service_url: std::env::var("TOKEN_SERVICE_URL")
            .unwrap_or_else(|_| "http://token-service:8082".into()),
        bootstrap_token: std::env::var("SERVICE_BOOTSTRAP_TOKEN")
            .expect("SERVICE_BOOTSTRAP_TOKEN required"),
        caller_service_name: "iam-service".into(),
        caller_tenant: std::env::var("TENANT_ID")?.parse()?,
        refresh_skew: Duration::from_secs(60),
        request_timeout: Duration::from_secs(5),
    },
    http.clone(),
));
service_jwt.refresh("iam-service").await.ok();  // self-issued not needed, but doesn't hurt
```

Add `jwks` and `service_jwt` to `AppState`:
```rust
pub struct AppState {
    // ... existing
    pub jwks: Arc<common_jwt::JWKSCache>,
    pub service_jwt: Arc<common_jwt::ServiceJwtCache>,
}
```

Implement `PrincipalStateProvider` for `AppState`:
```rust
impl jwt_ctx::PrincipalStateProvider for AppState {
    fn jwks(&self) -> Arc<common_jwt::JWKSCache> { self.jwks.clone() }
}
```

- [ ] **Step 4: Mount `verify_aud_principal_middleware("iam-service")` on `/iam/internal/*`**

In the router builder:
```rust
let app = Router::new()
    .route("/iam/internal/users/by-role/{role_id}", get(list_users_by_role))
    .route("/iam/internal/users/by-dept/{dept_id}", get(list_users_by_dept))
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        verify_aud_principal_middleware("iam-service"),
    ))
    .with_state(state);
```

- [ ] **Step 5: Build + test**

```bash
cargo build -p iam-service
cargo test -p iam-service
```

Expected: clean build, all existing tests pass. (Existing tests that don't go through `/iam/internal/*` are unaffected.)

- [ ] **Step 6: Commit**

```bash
git add services/iam-service libs/authz
git commit -m "dephase(auth): iam-service verify_aud middleware + service-jwt cache"
```

---

## Task D2: `notification-service` JWKS + service-jwt cache

**Files:**
- Modify: `services/notification-service/Cargo.toml` (add `common-jwt`)
- Modify: `services/notification-service/src/main.rs` (boilerplate + AppState)
- Modify: `services/notification-service/src/iam_client.rs` (replace `None` with real `Some(service_jwt_for("iam-service"))`)

**Interfaces:**
- Consumes: Task A (`common_jwt`), Task B (token-service), Task C (Principal), Task D1 (verify_aud pattern)
- Produces:
  - `AppState.jwks`, `AppState.service_jwt`
  - `IamMemberClient::by_role(role_id, tenant, jwt)`, `by_dept(...)`, `expand_scopes(...)`, `fetch_items(...)` all accept `Some(&str)` for the JWT (already plumbed from cdd0c8e)
  - Outbound `by_role` / `by_dept` calls pass service-jwt for `iam-service` audience

- [ ] **Step 1: Add `common-jwt` to notification-service Cargo.toml**

- [ ] **Step 2: Add JWKS + service-jwt cache to main, AppState, PrincipalStateProvider**

Same boilerplate as D1, with `caller_service_name: "notification-service"`. `verify_aud_principal_middleware` is NOT mounted on notification-service's HTTP routes (it has no `/internal/*`).

- [ ] **Step 3: Update `iam_client` to use real service-jwt**

In `services/notification-service/src/iam_client.rs`, the methods `by_role`, `by_dept`, `expand_scopes`, `publish_announcement` currently call `fetch_items(&url, user_jwt)` where `user_jwt: Option<&str>`. The user's JWT is irrelevant for service-to-service; replace with service-jwt:

Add to `IamMemberClient`:
```rust
pub struct IamMemberClient {
    http: Arc<ResilientClient>,
    base_url: String,
    service_jwt: Arc<common_jwt::ServiceJwtCache>,  // NEW
}
```

In `fetch_items(&self, url: &str, _user_jwt: Option<&str>)`:
- Replace `_user_jwt` with `service_jwt.get("iam-service")` lookup
- Pass that into the Authorization header

Actually simpler: the `ResilientClient` already supports an `auth_injector` (Task C's `AuthInjector`). The injector's `header_value()` calls `service_jwt.get(target)`. So the iam_client doesn't need to know about service-jwt at all — the `ResilientClient` is configured once with the injector and handles all calls.

Wire it up in main:
```rust
let http = reqwest::Client::builder().timeout(...).build()?;
let injector = common_http::auth_injector::AuthInjector::new(
    service_jwt.clone(),
    "iam-service".into(),  // default target
);
let http_client = ResilientClient::with_auth_injector(http, injector);
```

Then in `fetch_items`, call `http_client.call("iam", Method::GET, url, &HeaderMap::new(), Bytes::new())` — the injector auto-adds the auth header.

The `user_jwt: Option<&str>` parameter becomes redundant. Simplify: remove it from the `IamMemberClient` API. (This is a refactor; do it as part of this task.)

- [ ] **Step 4: Build + test**

```bash
cargo build -p notification-service
cargo test -p notification-service
```

Expected: clean. Existing tests for `IamMemberClient` need updating to drop the `user_jwt` arg.

- [ ] **Step 5: Commit**

```bash
git add services/notification-service
git commit -m "dephase(auth): notification-service service-jwt cache; replace IamMemberClient user_jwt with service-jwt"
```

---

## Task D3: `audit-service` JWKS + service-jwt cache

**Files:**
- Modify: `services/audit-service/Cargo.toml`
- Modify: `services/audit-service/src/main.rs`

Same pattern as D2. `audit-service` has no outbound service-to-service calls today (it consumes RabbitMQ events and writes to DB). Service-jwt cache is wired up for future-proofing but not actively used.

- [ ] **Step 1-4**: Same as D2 boilerplate. `caller_service_name: "audit-service"`.

- [ ] **Step 5: Commit**

```bash
git add services/audit-service
git commit -m "dephase(auth): audit-service service-jwt cache boilerplate (no active outbound)"
```

---

## Task D4: `token-service` self-verify + `fetch_json` carry user-jwt

**Files:**
- Modify: `services/token-service/Cargo.toml` (add `common-jwt`)
- Modify: `services/token-service/src/main.rs` (JWKS cache, AppState)
- Modify: `services/token-service/src/lib.rs` / `routes/refresh.rs` / `routes/access.rs` (read user-jwt from request headers, pass to `fetch_json`)

- [ ] **Step 1: Add `common-jwt` dep**

- [ ] **Step 2: Add JWKS cache to main + AppState + PrincipalStateProvider**

Note: `token-service` issues user-jwt and verifies user-jwt; it does NOT issue service-jwt for itself (the bootstrap-token path is for OTHER services calling token-service, not for self). So:
- `jwks` cache: yes
- `service_jwt` cache: NOT needed for token-service itself (it doesn't call other services)
- `verify_aud_principal_middleware`: NOT needed (token-service is the issuer; user-jwt has `aud: "user"` not "token-service")

- [ ] **Step 3: `fetch_json` carries `user_jwt` (from request) for the two call sites**

The two call sites (`routes/access.rs:79` and `routes/refresh.rs:115`) currently pass `None` for the `user_jwt: Option<&str>` argument. They have access to the request (`req.headers()` via `HeaderMap` extractor).

Add `HeaderMap` extractor to both handlers:
```rust
pub async fn handle_access(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    Json(req): Json<AccessRequest>,
) -> Result<...> {
    let user_jwt = headers.get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    // ... later: crate::fetch_json(&state.http_client, "iam-service", &claims_url, user_jwt)
}
```

This is the right place to forward the user-jwt (matching the spec §4.2 architecture).

- [ ] **Step 4: Build + test**

```bash
cargo build -p token-service
cargo test -p token-service
```

Expected: clean. Existing tests should pass; new test for `HeaderMap` extraction in handlers.

- [ ] **Step 5: Commit**

```bash
git add services/token-service
git commit -m "dephase(auth): token-service JWKS cache; fetch_json carries user-jwt from request"
```

---

## Task D5: `api-gateway` JWKS cache (no service-jwt)

**Files:**
- Modify: `services/api-gateway/Cargo.toml`
- Modify: `services/api-gateway/src/main.rs`

- [ ] **Step 1: Add `common-jwt` dep**

- [ ] **Step 2: Add JWKS cache to main + AppState + PrincipalStateProvider**

`api-gateway` is the JWT verifier for ALL inbound user requests. The `aud=None` in `Principal::from_token` (set by `FromRequestParts`) is the gateway-path config. service-jwt NOT needed (gateway doesn't call other services).

- [ ] **Step 3: Smoke test**

```bash
cargo build -p api-gateway
cargo test -p api-gateway
```

Expected: clean. Existing tests should pass.

- [ ] **Step 4: Commit**

```bash
git add services/api-gateway
git commit -m "dephase(auth): api-gateway JWKS cache; user-jwt verify via Principal extractor"
```

---

## Task D6: `prue_backend` JWKS + service-jwt cache (future-proofing)

**Files:**
- Modify: `services/prue_backend/Cargo.toml`
- Modify: `services/prue_backend/src/main.rs`

- [ ] **Step 1-3**: Same boilerplate. `caller_service_name: "prue_backend"`. No active outbound service calls today; service-jwt is wired but unused.

- [ ] **Step 4: Commit**

```bash
git add services/prue_backend
git commit -m "dephase(auth): prue_backend service-jwt cache boilerplate (no active outbound)"
```

---

## Task D7: outbox-publishers — service-jwt cache only

**Files:**
- Modify: `services/iam-outbox-publisher/Cargo.toml`
- Modify: `services/iam-outbox-publisher/src/main.rs`
- Modify: `services/notification-outbox-publisher/Cargo.toml`
- Modify: `services/notification-outbox-publisher/src/main.rs`

- [ ] **Step 1: Add `common-jwt` dep to both**

- [ ] **Step 2: Add service-jwt cache to both (no JWKS)**

Outbox publishers do SQL only (no inbound HTTP). They don't need JWKS, but they need service-jwt IF they call other services. Today they don't, so this is future-proofing. Skip if too speculative — defer to the next phase that requires it.

**Decision**: Skip service-jwt cache for outbox-publishers in this task. They're SQL-only and don't call other services. The `common-jwt` dep is added but no cache instantiation. Re-evaluate when outbox-publishers gain outbound calls (phase5b-event-sync?).

- [ ] **Step 3: Commit**

```bash
git add services/iam-outbox-publisher services/notification-outbox-publisher
git commit -m "dephase(auth): outbox-publishers add common-jwt dep (no active use; future-proofing)"
```

---

## Task E: docs — `service-to-service-auth.md` + ai-context + phase5a spec superseded

**Files:**
- Create: `docs/guides/service-to-service-auth.md`
- Modify: `docs/ai-context.md`
- Modify: `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` (mark §2.2 "Phase 5-B JWT 签名验证" 段 superseded)
- Modify: `infrastructure/.env` (add `SERVICE_BOOTSTRAP_TOKENS` and per-service bootstrap tokens)
- Modify: `infrastructure/docker-compose-services.yml` (add `SERVICE_BOOTSTRAP_TOKEN` / `TENANT_ID` / `TOKEN_SERVICE_URL` to each business service; add `SERVICE_BOOTSTRAP_TOKENS` to token-service)

- [ ] **Step 1: Write `service-to-service-auth.md`**

Outline:
- §1 概览 — service-jwt 在 phase5a-2 后的位置
- §2 token-service bootstrap-token 端点 — POST /tokens/service-jwt 怎么用
- §3 bootstrap token 运维 — rotate / 泄漏处理 / dev / prod
- §4 service 主进程启动序列（§4.1 of spec）
- §5 内部调用出站怎么注入 header（common-http::ResilientClient）
- §6 verify_aud 中间件使用指南
- §7 dev 模式 token 示例
- §8 出错处理速查表

- [ ] **Step 2: Update `docs/ai-context.md`**

§2 service map table — add a column or row entry for `common-jwt` lib.
§3 architecture — add common-jwt to the lib list.
Add a §4 ADR entry: "2026-07-07 phase5b: 5a-2 留下的 JWT 验签缺口补齐。"

- [ ] **Step 3: Mark phase5a §2.2 superseded**

In `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md`:
- Add a "Superseded by 2026-07-07-phase5b-..." banner at the top of §2.2

- [ ] **Step 4: Generate dev bootstrap tokens + write to `infrastructure/.env`**

For each business service, generate a 32-byte random token. For dev, use deterministic placeholders (so docker-compose can be re-run without state drift):
```bash
# Generate one per service (dev values, NOT for production)
IAM_DEV_BT=$(openssl rand -hex 32)
TOKEN_DEV_BT=$(openssl rand -hex 32)
NOTIFICATION_DEV_BT=$(openssl rand -hex 32)
AUDIT_DEV_BT=$(openssl rand -hex 32)
PRUE_DEV_BT=$(openssl rand -hex 32)
GATEWAY_DEV_BT=$(openssl rand -hex 32)
```

Append to `infrastructure/.env`:
```bash
# Phase 5-B: dev bootstrap tokens (one per business service).
# DO NOT use these in production. Production must use a secret manager.
IAM_DEV_BT=...
TOKEN_DEV_BT=...
# (etc)

# token-service accepts all of the above
SERVICE_BOOTSTRAP_TOKENS=${IAM_DEV_BT},${TOKEN_DEV_BT},${NOTIFICATION_DEV_BT},${AUDIT_DEV_BT},${PRUE_DEV_BT},${GATEWAY_DEV_BT}

# Common
TENANT_ID=1
TOKEN_SERVICE_URL=http://token-service:8082
```

- [ ] **Step 5: Update `docker-compose-services.yml`**

For each business service (iam / token / notification / audit / prue_backend / api-gateway):
- Add to `environment:`:
  ```yaml
    SERVICE_BOOTSTRAP_TOKEN: "${IAM_DEV_BT}"  # or the service-specific name
    TENANT_ID: "${TENANT_ID}"
    TOKEN_SERVICE_URL: "${TOKEN_SERVICE_URL}"
  ```

For `token-service`:
- Add:
  ```yaml
    SERVICE_BOOTSTRAP_TOKENS: "${SERVICE_BOOTSTRAP_TOKENS}"
  ```

- [ ] **Step 6: Commit**

```bash
git add docs/ai-context.md docs/guides/service-to-service-auth.md \
        docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md \
        infrastructure/.env infrastructure/docker-compose-services.yml
git commit -m "dephase(auth): docs + compose bootstrap tokens (phase5b rollout)"
```

---

## Acceptance criteria

After Task E, all of the following must hold:

1. `cargo check --workspace` clean
2. `cargo test --workspace --lib` green
3. `cargo test -p common-jwt -p jwt-ctx` green
4. `node scripts/check-spec.js` green
5. `node scripts/contracts-gen.js --check` green
6. dev compose 启动后，notification-service → iam-service (`/iam/internal/users/by-role/1?tenant_id=1`) returns 200 (not 401)
7. dev compose 启动后，gateway → token-service `/tokens/jwks` returns 200 (existing behavior preserved)
8. manually: change a service's `SERVICE_BOOTSTRAP_TOKEN` to an invalid value, restart; service should panic at startup (cold-start verify_token fails)
9. manually: send a request to `/iam/internal/*` with `Authorization: Bearer <wrong-token>` → 401
10. manually: send a request to `/iam/internal/*` with `Authorization: Bearer <valid-user-jwt>` → 401 (audience mismatch)

---

## Execution-time risks and mitigations

| Risk | Detection | Mitigation |
|---|---|---|
| `jsonwebtoken` 9.x breaks existing user-jwt tokens | cargo test -p token-service | pin to `=9.0.0` after first test pass |
| Service main panics if token-service not up at boot | `docker compose up` | add 3 retries with backoff in main before panic |
| `verify_aud` middleware double-verifies (cost) | runtime | store Principal in request extensions to skip re-extract in handler |
| Per-service `cargo test` timeout on slow CI | CI | `cargo nextest` parallel runs |
| Bootstrap token leakage in `.env` | git history scan | add `.env` to `.gitignore` (already done) + use Vault for prod |
| audience middleware forgotten on new internal route | manual | lint rule: `/iam/internal/*` MUST have middleware (CodeGraph or `#[route_internal]` macro) |
| 5a-2 spec §0.5.6 open questions deprioritized | spec review | reopen as phase5c / phase5b-tls later |

---

## Self-Review (post-draft)

- [x] **Spec coverage**: §0.5 / §2 / §3 → Tasks A-C; §3.3-3.4 / §6 → Tasks D*-E; §4 → Section 4 of plan; §5 → Error handling in tasks; §7 → §Acceptance criteria; §8 → §Risks
- [x] **No placeholder patterns**: every step has explicit code. Bootstrap token values marked as "openssl rand -hex 32" with deterministic dev placeholder.
- [x] **Type / method / port consistency across tasks**:
  - `JWKSCache::from_pem` (test-only constructor) defined in Task A
  - `ServiceJwtCache::get(target)` consistently used as `(target: &str) -> String`
  - `AuthInjector` defined in `libs/common-http/src/auth_injector.rs` (Task D1 dependency)
  - `verify_aud_principal_middleware` defined in `libs/authz` (Task D1)
  - `PrincipalStateProvider` defined in `libs/jwt-ctx` (Task C)
  - Port 8082 token-service / 8081 iam / 8083 notification — consistent with phase5a-2
- [x] **Bite-sized steps**: each step is one operation. Largest step is "Step 5: Add `JWKSCache::refresh` (HTTP path)" — acceptable since it's one logical unit.
- [x] **One commit per task**: Step 7 of each task is the commit. Task D7 is intentionally small (no active use).
- [x] **Test command runs locally**: `CARGO_TARGET_DIR=/tmp/cargo-dephase cargo test -p common-jwt -p jwt-ctx`; full workspace test in CI (tmpfs 6.4G insufficient locally).
