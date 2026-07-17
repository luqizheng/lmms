# Phase 5-B：JWT 签名验证 + service-jwt 机制 — 设计

**日期**：2026-07-07 · 起草
**状态**：草案待评审
**关联**：
- 上游：[`2026-07-07-deprecate-consul-connect-use-registrator-design.md`](2026-07-07-deprecate-consul-connect-use-registrator-design.md) (phase5a-2)。本 spec 是其 §0.5.6 open question #1-#3 的落地。
- 取消/取代：[`2026-07-03-phase5a-consul-connect-mtls-design.md`](2026-07-03-phase5a-consul-connect-mtls-design.md) §2.2 的「Phase 5-B：JWT 签名验证 (K3s 路径)」 段（被本文档取代）。
- 关联 commit：`b5ae6dd`（`/iam/internal/*` 加 Principal extractor 但无验签）、`cdd0c8e`（notification / token passthrough 留 `None` 占位）。

---

## 0. 一句话

> `libs/jwt-ctx` 当前只 decode 不 verify，phase5a-2 之后所有 service→service 内部调用「信任调用方」已经是默认假设。本 spec 加 ① 走 JWKS 验签任何 JWT（用户 + 内部都覆盖），② token-service 新增 `/tokens/service-jwt` 端点发短期 service-jwt，③ 业务服务冷启动拿 service-jwt 并在 outbound HTTP 自动注入。所有改动以 `dephase(auth): ...` 前缀 commit。

---

## 0.5. 决策路径回溯（brainstorming trace）

### 0.5.1 时间线

| # | 触发 | 选项 / 决定 | 理由 |
|---|---|---|---|
| 1 | phase5a-2 落地后 `cdd0c8e` 留 `None` 占位 + `b5ae6dd` 加 `Principal` extractor 但 jwt-ctx 不验签 | security 缺口识别 | 服务间调用现在全靠 docker 网络拓扑「信任」；任何能进 `superpowers` 网络的容器都能调 `/iam/internal/*` |
| 2 | AI 自荐 4 个 phase5b 方向（auth / tls / event-sync / hygiene） | user 选 5b-auth | 安全是生产前置，与项目「无生产数据可大胆重构」的开发阶段最合适先做 |
| 3 | JWT 验证 key source：静态 PEM / 动态 JWKS / HS256 共享 | **JWKS** | 生产友好，支持 key rotation；冷启动能 reach token-service 即可（既有依赖） |
| 4 | service-jwt issuer：token-service / gateway / pre-shared API key | **token-service** | 复用现有签发架构（同一对 RSA key）；不引入新服务 |
| 5 | scope：MVP / 全栈 / 只 service-jwt | **全栈** | 半成品不是好半成品，user-jwt + service-jwt 一起做 |
| 6 | lib 拓扑：扩 jwt-ctx / 新 common-jwt | **新 common-jwt** | jwt-ctx 只管 claim 模型，crypto 集中在 common-jwt；service-jwt 与 user-jwt 共享 lib |
| 7 | `expected_aud` 校验 | **caller 决定** | 灵活；service→service 强制，gateway 路径可选 |
| 8 | 后台 refresh 启动 | **caller 控制** | refresh task 生命周期跟 service main 的 `shutdown_rx` 走 |
| 9 | bootstrap token 形式 | **每服务独立** | 一服务泄漏不影响其他；rotate 是改 env + 重启 |

### 0.5.2 否决选项

**MVP（只做 jwt-ctx 验签 + user-jwt 路径）**：
- ❌ `/iam/internal/*` 仍然缺鉴权；service-jwt 留到下一 phase 又会形成新的「半成品」→ 拒绝

**HS256 共享密钥**：
- ❌ 让项目从 RS256 倒退；share-secret 不是现代最佳实践 → 拒绝

**静态 PEM 镜像烘焙**：
- ❌ key rotation 要重建所有 service 镜像；dev / prod 切换要换镜像 → 拒绝

**long-lived API key 代替 service-jwt**：
- ❌ 不是 JWT-风格身份；revocation 走 out-of-band (env 重启)；跟 phase5a-2「JWT 透传」精神不连贯 → 拒绝

**mTLS-like 网络拓扑信任**：
- ❌ 实现复杂，依赖网络层；违反 phase5a-2「服务直连 + 应用层 auth」原则 → 拒绝

### 0.5.3 采纳理由（按权重降序）

1. **安全是生产前置**：phase5a-2 已落地，mesh 不再提供 SPIFFE identity；现在任何能进 docker 网络的容器都等同于「任意 service」—— 是真实漏洞，不是 nice-to-have
2. **5a-2 commit `b5ae6dd` 留的 Principal extractor 是空壳**：`jwt-ctx::Principal::from_jwt_payload` 只 base64-decode payload 不验签；下一步就是给它加 verify
3. **复用量大**：token-service 已有 `jwt_simple` 签发 + `/tokens/jwks` 端点 + RSA keypair；common-jwt 主要做 verify + JWKS caching，复用这套设施
4. **scope 紧**：12 commits 左右，1-2 周可完成
5. **AGENTS.md 允许重构**：开发阶段、无生产数据

### 0.5.4 spec review checklist

- [x] 决策点 1-9 有答案 + 理由
- [x] 每个否决选项有反对论据
- [x] 「不做什么」显式列出（§1.3）
- [x] 改动影响面按文件列（§6）
- [x] 验证步骤可执行（§7）
- [x] 错误处理覆盖（§5）
- [x] 风险与缓解成对（§8）

### 0.5.5 仍未解决的开放问题

| 问题 | 暂定答案 | 何时拍板 |
|---|---|---|
| 是否需要 refresh_token 撤销列表（5a-2 引入了 Redis perm_epoch）跟 JWT 验证串联 | 否，perm_epoch 已经是 token 失效检查器，不在 JWT 签名层 | phase5c 评估 |
| 服务间 mTLS 跟 service-jwt 是不是二选一 | 二选一；选 service-jwt（更轻 / 跨语言友好） | 已在 §1.3 拒绝 mTLS-like |
| Service-jwt claims 里 `scopes` 字段是否本 spec 落地 | 不落地（`scopes: ["internal:..."]` 占位留着） | phase5b-event-sync 决定 |

---

## 1. 背景与现状

### 1.1 5a-2 留下的安全状态

phase5a-2（commit `b5ae6dd` + `cdd0c8e`）做了：

- `/iam/internal/*` 加 `Principal` extractor
- `token-service::fetch_json` / `notification-service::iam_client` 加 `user_jwt: Option<&str>` 参数
- call site 暂时传 `None`（设计「未来调用方传 user-jwt」）

但 `jwt-ctx::Principal::from_jwt_payload` 实际行为是：

```rust
pub fn from_jwt_payload(token: &str) -> Result<Self, PrincipalError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 { ... }
    let payload_b64 = parts[1];
    let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64)...;
    let payload: Value = serde_json::from_slice(&payload_bytes)?;
    // 直接读 claims，没有 verify_signature!
    ...
}
```

`jsonwebtoken::decode` 这步**完全没做**。任何能伪造 `{"sub":"x","tid":1,"exp":<far-future>}` 的人，base64 编码后塞进 `Authorization: Bearer <junk>`，iam-service 都会接受。

### 1.2 已具备的设施（避免重新发明）

- `services/token-service` 已有 RSA keypair（`/app/keys/{private,public}.pem`），用 `jwt_simple` 签发
- `services/token-service` 已有 `GET /tokens/jwks` 端点，发布 RS256 public key
- `common-http::ResilientClient` 已有 service 调 service 的 retry/timeout/breaker
- AGENTS.md 已有 `aud: "service"` / `aud: "user"` 约定（iam AGENTS.md §「内部服务调用」）

### 1.3 不做的事（明确边界）

- ❌ 不做 TLS termination（dev 仍 HTTP；TLS 是 phase5b-tls 的事，下一 spec）
- ❌ 不做 mTLS / SPIFFE（phase5a-2 已撤 mesh）
- ❌ 不做 refresh_token 撤销与 JWT 验签的耦合（AGENTS.md 已有 perm_epoch 机制，独立演化）
- ❌ 不动 frontend（Nginx，纯消费 gateway，不在 5b-auth 范围）
- ❌ 不引入新外部 crate（只 `jsonwebtoken` 加入 workspace.dependencies）

---

## 2. 目标架构

### 2.1 组件图

```
                    ┌─────────────────────────────────────────┐
                    │  libs/common-jwt (NEW)                   │
                    │  ├─ JWKSCache                            │
                    │  ├─ verify_token(token, cache, aud?)    │
                    │  └─ sign_token(key_pem, claims, ttl)    │
                    └─────────────────────────────────────────┘
                              ▲                ▲
                              │                │
       ┌──────────────────────┘                └────────────────────┐
       │                                                            │
┌──────┴─────────────┐                                  ┌──────────┴──────────┐
│  libs/jwt-ctx      │                                  │  services/token-svc  │
│  Principal::       │                                  │  POST /tokens/       │
│  from_token (NEW)  │                                  │    service-jwt       │
│  verify via common-│                                  │  (NEW endpoint)      │
│  jwt               │                                  │  uses sign_token()   │
└────────────────────┘                                  └─────────────────────┘
       ▲                                                            │
       │                                                            │
       │  7 个 service 启动：                                        │
       │  1. JWKSCache 拉 JWKS                                      │
       │  2. ServiceJwtCache 拿 service-jwt                          │
       │  3. 后台 task 续期                                          │
       │                                                            │
┌──────┴────────────┐  ← JWKS  GET http://token-service:8082/        │
│ 业务 service      │       .well-known/jwks.json                    │
│ (iam / token /     │  ← service-jwt POST /tokens/service-jwt        │
│  notification /   │       Authorization: Bearer <bootstrap>        │
│  audit / api-gw /  │                                              │
│  prue_backend /   │  ────────────  ResilientClient  ──────  → up   │
│  outbox-pubs /     │              注入 Authorization: Bearer       │
│  schedulers)       │              <service-jwt from cache>          │
└───────────────────┘                                              ↓
                                                              service-to-service
                                                              调用全部受 service-jwt 保护
```

### 2.2 端点：token-service 新增 `POST /tokens/service-jwt`

```
POST /tokens/service-jwt
Headers:
  Authorization: Bearer <SERVICE_BOOTSTRAP_TOKEN>   # 每服务一个
  Content-Type: application/json
Body:
  { "aud": "iam-service", "tid": 1 }   # aud 必填，tid 必填
Response 200:
  {
    "access_token": "<RS256 JWT>",
    "token_type": "Bearer",
    "expires_in": 3600
  }
Response 401: bootstrap token 错 / 缺
Response 400: aud / tid 缺或不合法
```

签发 JWT claims（用 token-service 现有 private key，跟 user-jwt 一致）：

```json
{
  "iss": "token-service",
  "sub": "<caller-service-name>",   // 来自 SERVICE_NAME env
  "aud": "<target-service-name>",
  "tid": <caller-tenant>,
  "iat": <now>,
  "exp": <now + 3600>,
  "roles": ["service"],
  "scopes": ["internal:<verb>"]      // 预留，phase5b-event-sync 用
}
```

---

## 3. 组件详细设计

### 3.1 `libs/common-jwt` (NEW)

```rust
//! shared JWT primitives: JWKS cache, verify, sign
//! Phase 5-B. Replaces ad-hoc jsonwebtoken usage scattered across services.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;
use thiserror::Error;

// ──────────── JWKS cache ────────────

#[derive(Clone, Debug)]
pub struct JwksConfig {
    pub url: String,
    pub refresh_interval: Duration,    // default 3600s
    pub request_timeout: Duration,     // default 5s
}

impl Default for JwksConfig {
    fn default() -> Self { ... }
}

pub struct JWKSCache {
    inner: Arc<RwLock<JwksInner>>,
    config: JwksConfig,
    http: reqwest::Client,
}

struct JwksInner {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Option<Instant>,
}

impl JWKSCache {
    pub fn new(config: JwksConfig, http: reqwest::Client) -> Self;

    /// Force refresh from token-service. Replaces inner map. Idempotent.
    pub async fn refresh(&self) -> Result<(), JwksError>;

    /// Look up DecodingKey by kid. Returns None if not cached.
    pub fn get(&self, kid: &str) -> Option<DecodingKey>;

    /// Spawn background refresh loop. Caller controls lifecycle via shutdown.
    pub fn spawn_refresh_loop(self: Arc<Self>, mut shutdown: tokio::sync::watch::Receiver<bool>);

    /// Convenience: read first key (most setups only have one). Returns None
    /// if cache is empty. The returned key has kid="" for legacy callers.
    pub fn default_key(&self) -> Option<DecodingKey>;
}

// ──────────── Token operations ────────────

pub async fn verify_token(
    token: &str,
    cache: &JWKSCache,
    expected_aud: Option<&str>,
) -> Result<serde_json::Value, VerifyError> {
    let header = decode_header(token).map_err(|e| VerifyError::Malformed(e.to_string()))?;
    let key = cache.get(&header.kid.ok_or_else(|| VerifyError::Malformed("missing kid".into()))?)
        .or_else(|| cache.default_key())
        .ok_or_else(|| VerifyError::Malformed(format!("unknown kid: {:?}", header.kid)))?;
    let mut validation = Validation::new(Algorithm::RS256);
    if let Some(aud) = expected_aud {
        validation.set_audience(&[aud]);
    } else {
        validation.validate_aud = false;
    }
    let data = decode::<serde_json::Value>(token, &key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::InvalidSignature => VerifyError::SignatureInvalid,
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => VerifyError::Expired,
            jsonwebtoken::errors::ErrorKind::InvalidAudience => VerifyError::AudienceMismatch { ... },
            _ => VerifyError::Malformed(e.to_string()),
        })?;
    Ok(data.claims)
}

pub fn sign_token(
    private_key_pem: &str,
    claims: &serde_json::Value,
    ttl: Duration,
) -> Result<String, SignError>;

// ──────────── Errors ────────────

#[derive(Error, Debug)]
pub enum JwksError {
    #[error("JWKS fetch failed: {0}")]
    FetchFailed(#[from] reqwest::Error),
    #[error("JWKS parse failed: {0}")]
    ParseFailed(String),
    #[error("missing kid in JWK")]
    MissingKid,
    #[error("JWK construction failed: {0}")]
    KeyBuildFailed(String),
}

#[derive(Error, Debug)]
pub enum VerifyError {
    #[error("malformed JWT: {0}")]
    Malformed(String),
    #[error("signature invalid")]
    SignatureInvalid,
    #[error("audience mismatch: expected {expected:?}, got {got:?}")]
    AudienceMismatch { expected: String, got: String },
    #[error("expired")]
    Expired,
    #[error("JWKS error: {0}")]
    Jwks(#[from] JwksError),
}

#[derive(Error, Debug)]
pub enum SignError {
    #[error("invalid PEM key: {0}")]
    InvalidKey(String),
    #[error("encode failed: {0}")]
    Encode(String),
}
```

依赖（新增到 workspace.dependencies）：

```toml
jsonwebtoken = { version = "9", default-features = false }
```

（`reqwest`、`serde`、`tokio`、`thiserror` 已经在 workspace 里了。）

### 3.2 `libs/jwt-ctx` 改造

```rust
// 旧：纯 decode。新增 verify 路径。
impl Principal {
    /// 旧路径：纯 decode，无 verify。保留供 unit test / mock。
    /// Phase 5-B: 内部使用，service 端应该走 from_token。
    pub fn from_jwt_payload(token: &str) -> Result<Self, PrincipalError>;

    /// 新路径：decode + verify via JWKSCache。
    /// `expected_aud`: Some 强制 aud 匹配，None 跳过（gateway 路径可选）。
    pub async fn from_token(
        token: &str,
        cache: &common_jwt::JWKSCache,
        expected_aud: Option<&str>,
    ) -> Result<Self, PrincipalError> {
        let claims = common_jwt::verify_token(token, cache, expected_aud).await?;
        // 现有 claim 解析逻辑（user_id, user_code, tid, ...）
        Self::from_claims(claims).map_err(...)
    }

    fn from_claims(payload: serde_json::Value) -> Result<Self, PrincipalError>;
}

// FromRequestParts 变 async
impl<S> FromRequestParts<S> for Principal
where
    S: Send + Sync,
    S: PrincipalStateProvider,    // 任何有 jwks: Arc<JWKSCache> 的 state
{
    type Rejection = (StatusCode, &'static str);
    async fn from_request_parts(parts, state) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or((StatusCode::UNAUTHORIZED, "missing bearer"))?;
        let cache = state.jwks();
        Self::from_token(token, &cache, None)   // aud=None 跳过（gateway path）
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid token"))
    }
}

pub trait PrincipalStateProvider {
    fn jwks(&self) -> Arc<common_jwt::JWKSCache>;
}
```

**错误变体新增**（保持 `PrincipalError` 不变，新增 `PrincipalVerifyError`）：

```rust
#[derive(Error, Debug)]
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

`PrincipalError` 保持不变（向后兼容 from_jwt_payload）。

### 3.3 `token-service` 新增端点

```rust
// services/token-service/src/routes/service_jwt.rs (NEW)

use axum::{
    extract::State,
    http::HeaderMap,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, EncodingKey, Header};
use common_jwt::sign_token;

#[derive(Debug, Deserialize)]
pub struct ServiceJwtRequest {
    pub aud: String,                    // 必填：目标服务名
    pub tid: i64,                       // 必填：caller tenant
}

#[derive(Debug, Serialize)]
pub struct ServiceJwtResponse {
    pub access_token: String,
    pub token_type: &'static str,        // "Bearer"
    pub expires_in: i64,                 // 3600
}

pub async fn handle_service_jwt(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    Json(req): Json<ServiceJwtRequest>,
) -> Result<Json<ServiceJwtResponse>, (StatusCode, String)> {
    // 1. validate bootstrap token
    let auth = headers.get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer".into()))?;
    if !state.bootstrap_tokens.iter().any(|t| t.as_str() == auth) {
        return Err((StatusCode::UNAUTHORIZED, "invalid bootstrap".into()));
    }
    // 2. validate request
    if req.aud.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "aud required".into()));
    }
    // 3. sign
    let now = chrono::Utc::now().timestamp();
    let caller = std::env::var("SERVICE_NAME").unwrap_or_else(|_| "unknown".into());
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": caller,
        "aud": req.aud,
        "tid": req.tid,
        "iat": now,
        "exp": now + 3600,
        "roles": ["service"],
    });
    let token = sign_token(&state.private_key_pem, &claims, Duration::from_secs(3600))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("sign: {}", e)))?;
    Ok(Json(ServiceJwtResponse {
        access_token: token,
        token_type: "Bearer",
        expires_in: 3600,
    }))
}

pub fn routes(state: TokenServerState) -> Router {
    Router::new()
        .route("/tokens/service-jwt", axum::routing::post(handle_service_jwt))
        .with_state(state)
}
```

`TokenServerState` 新增字段：

```rust
pub struct TokenServerState {
    pub db: DbPool,
    pub issuer: Arc<TokenIssuer>,
    pub issuer_str: String,
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: Arc<ResilientClient>,
    pub user_service_url: String,
    pub private_key_pem: String,                  // NEW
    pub bootstrap_tokens: Arc<Vec<String>>,       // NEW
}
```

bootstrap tokens 来源：`std::env::var("SERVICE_BOOTSTRAP_TOKENS")` 解析 comma-separated，注入到 `TokenServerState`（启动时）。

### 3.4 Service-side 集成模式

#### 3.4.1 `libs/common-http` 新增 `service-jwt` 注入

```rust
// common-http 新增 builder option
pub struct ResilientClient {
    // ... existing fields
    service_jwt: Option<Arc<ServiceJwtInjector>>,
}

pub struct ServiceJwtInjector {
    cache: Arc<ServiceJwtCache>,
}

impl ServiceJwtInjector {
    pub fn header_value(&self) -> Option<HeaderValue> {
        let token = self.cache.current_token()?;
        HeaderValue::from_str(&format!("Bearer {token}")).ok()
    }
}

// call site
client.call_with_extra_headers(extra_headers_with_auth);

// 在 ResilientClient::call 内部：
//   if let Some(inj) = &self.service_jwt {
//       headers.insert(AUTHORIZATION, inj.header_value()?);
//   }
```

具体：给 `ResilientClient::call` 加一个 `with_authorization: Option<HeaderValue>` 参数（默认 None，service main 在 state 里持有 injector，每次 call 注入）。

#### 3.4.2 `libs/common-jwt` 新增 `ServiceJwtCache`（与 `JWKSCache` 配套）

```rust
pub struct ServiceJwtCache {
    inner: Arc<RwLock<ServiceJwtInner>>,
    config: ServiceJwtConfig,
    http: reqwest::Client,
}

pub struct ServiceJwtConfig {
    pub token_service_url: String,     // http://token-service:8082
    pub bootstrap_token: String,
    pub caller_service_name: String,    // 来自 SERVICE_NAME
    pub caller_tenant: i64,             // 来自 TENANT_ID env
    pub refresh_skew: Duration,         // default 60s
    pub request_timeout: Duration,      // default 5s
}

struct ServiceJwtInner {
    token: Option<String>,
    expires_at: Option<Instant>,
}

impl ServiceJwtCache {
    pub fn new(config: ServiceJwtConfig, http: reqwest::Client) -> Self;

    /// Mint service-jwt for a specific target. Result is cached and reused
    /// until within refresh_skew of expiry.
    pub async fn get(&self, target: &str) -> Result<String, ServiceJwtError>;

    /// Force refresh (used by background loop and on test path).
    pub async fn refresh(&self, target: &str) -> Result<String, ServiceJwtError>;

    /// Background refresh loop. Caller controls shutdown.
    pub fn spawn_refresh_loop(
        self: Arc<Self>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    );
}
```

`ResilientClient` 调用约定更新：

```rust
// 旧
client.call(service, method, url, headers, body).await

// 新（推荐）：通过 state 拿到的 injector 自动加 Authorization
let headers = state.service_jwt.headers_for("iam-service")?;
client.call_with_auth(service, method, url, headers, body).await
```

#### 3.4.3 每个 service main 的 boilerplate

```rust
// services/<svc>/src/main.rs (新增 section)
let service_jwt = Arc::new(common_jwt::ServiceJwtCache::new(
    common_jwt::ServiceJwtConfig {
        token_service_url: std::env::var("TOKEN_SERVICE_URL")?,
        bootstrap_token: std::env::var("SERVICE_BOOTSTRAP_TOKEN")?,
        caller_service_name: std::env::var("SERVICE_NAME")?,
        caller_tenant: std::env::var("TENANT_ID")?.parse()?,
        refresh_skew: Duration::from_secs(60),
        request_timeout: Duration::from_secs(5),
    },
    http.clone(),
));
service_jwt.refresh("iam-service").await?;  // 启动时强制拉一次
service_jwt.clone().spawn_refresh_loop(shutdown_rx.clone());

let jwks = Arc::new(common_jwt::JWKSCache::new(
    common_jwt::JwksConfig {
        url: format!("{}/.well-known/jwks.json",
                     std::env::var("TOKEN_SERVICE_URL")?),
        refresh_interval: Duration::from_secs(3600),
        request_timeout: Duration::from_secs(5),
    },
    http.clone(),
));
jwks.refresh().await?;
jwks.spawn_refresh_loop(shutdown_rx);
```

`AppState` / `TokenServerState` 增字段：

```rust
pub struct AppState {
    // ... 现有 fields
    pub jwks: Arc<common_jwt::JWKSCache>,
    pub service_jwt: Arc<common_jwt::ServiceJwtCache>,
}
```

### 3.5 `principal_extractor` 的 `audience` 校验

service-to-service 路径要求 aud 匹配本服务名（`iam-service` 端点验 `aud: "iam-service"`）。Principal extractor 在 service main 注册时配置：

```rust
// iam-service main.rs
let app = Router::new()
    .route("/iam/internal/users/by-role/{role_id}", get(list_users_by_role))
    .layer(axum::middleware::from_fn_with_state(
        state.jwks.clone(),
        verify_aud_principal_middleware("iam-service"),  // 强制 aud=iam-service
    ))
    .with_state(state);

// 其他 service 端点不需要 verify_aud（接受 user-jwt，aud=user）
```

middleware 实现在 `libs/authz` 复用现有的 `require_role` 模式。

---

## 4. 数据流（关键 path）

### 4.1 Service 启动序列

```
T+0s:    process start
T+0.1s:  load .env, parse SERVICE_NAME, SERVICE_BOOTSTRAP_TOKEN, TENANT_ID, TOKEN_SERVICE_URL
T+0.2s:  init http client, db pool, redis
T+0.3s:  JWKSCache::refresh()           # panic if fail
T+0.4s:  ServiceJwtCache::refresh("iam-service")  # panic if fail
T+0.5s:  spawn refresh loops
T+0.6s:  axum serve
T+∞:     ready
```

冷启动 0.5s 在 dev docker 里是合理的预算。

### 4.2 内部调用序列（notification → iam）

```
T:   notification handler receives request
     ↓
     Principal extractor (jwt-ctx::Principal::from_token)  ← verify user-jwt
     ↓
     handler calls IamMemberClient.by_role(role_id, tenant, Some(user_jwt))
     ↓
     IamMemberClient::fetch_items
       ResilientClient.call_with_auth(...)
         Authorization: Bearer <service-jwt>  ← ServiceJwtCache::get("iam-service")
     ↓
     iam-service receives
     ↓
     Principal extractor + verify_aud_principal_middleware("iam-service")
       jsonwebtoken::decode(token, key_from_jwks, validation{aud: "iam-service"})
     ↓
     handler runs with verified Principal + TenantScope
```

调用方（notification）传 user-jwt **不直接给 iam**；它传 `Some(user_jwt)` 是为了保留 caller 上下文给 audit log 或将来的 X-Service-Caller 头。**实际给 iam 的是 service-jwt**（由 ResilientClient 自动注入）。

### 4.3 Refresh 后台 task

```rust
// JWKS: 1h interval
loop {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(3600)) => {
            match cache.refresh().await {
                Ok(()) => info!("jwks refreshed"),
                Err(e) => warn!("jwks refresh failed: {}, keeping stale", e),
            }
        }
        _ = shutdown.changed() => break,
    }
}

// service-jwt: 30s interval check; refresh if within 60s of expiry
loop {
    tokio::select! {
        _ = tokio::time::sleep(Duration::from_secs(30)) => {
            for target in TARGETS.iter() {
                if cache.needs_refresh(target) {
                    match cache.refresh(target).await { ... }
                }
            }
        }
        _ = shutdown.changed() => break,
    }
}
```

---

## 5. 错误处理

| 场景 | 行为 | 错误码 |
|---|---|---|
| 启动时 JWKS 拉取失败 | panic / 进程退出 | n/a（启动失败） |
| 启动时 service-jwt 拉取失败 | panic | n/a |
| 后台 JWKS refresh 失败 | log warn，保持 stale | n/a（不阻塞请求） |
| 后台 service-jwt refresh 失败 | 指数退避 1s/2s/4s/8s/... 上限 60s | n/a |
| bootstrap token 错 | 401 | token-service 端返回 |
| 用户请求 JWT 验签失败 | 401，无 body 细节 | 401 |
| JWT 过期 | 401 + `WWW-Authenticate: Bearer error="invalid_token", error_description="expired"` | 401 |
| kid 找不到 | 触发强制 JWKS refresh 一次，仍未找到 → 401 | 401 |
| audience 不匹配 | 401 + `WWW-Authenticate: ... error="insufficient_scope"` | 401 |
| service-jwt 拉取 5xx | 退避 + 重试 | n/a（调用方重试由 ResilientClient 处理） |

---

## 6. 改动清单

### 6.1 文件新建

| 文件 | 内容 |
|---|---|
| `libs/common-jwt/Cargo.toml` | crate 描述，依赖 `jsonwebtoken` / `reqwest` / `tokio` / `serde` / `thiserror` |
| `libs/common-jwt/src/lib.rs` | JWKSCache / verify_token / sign_token / ServiceJwtCache / Errors |
| `libs/common-jwt/tests/cache_test.rs` | JWKS parse / refresh / kid lookup |
| `libs/common-jwt/tests/verify_test.rs` | sign+verify roundtrip / expired / wrong aud / wrong kid |
| `docs-site/guides/service-to-service-auth.md` | 内部调用 auth 流程 + bootstrap token 运维手册 |
| `services/token-service/src/routes/service_jwt.rs` | `POST /tokens/service-jwt` handler |

### 6.2 文件修改

| 文件 | 改动 |
|---|---|
| `Cargo.toml` (workspace) | 加入 `"libs/common-jwt"` 成员；`[workspace.dependencies]` 加 `jsonwebtoken` |
| `libs/jwt-ctx/src/principal.rs` | 新增 `Principal::from_token` (async)，新增 `PrincipalStateProvider` trait，新增 `PrincipalVerifyError` |
| `libs/common-http/src/client.rs` | `ResilientClient::call` 加 `service_jwt_injector: Option<Arc<ServiceJwtInjector>>` |
| `services/token-service/src/main.rs` | 注入 `private_key_pem` + `bootstrap_tokens` 到 `TokenServerState`；注册 `/tokens/service-jwt` 路由 |
| `services/token-service/src/lib.rs` | 导出 `ServiceJwtRequest` / `ServiceJwtResponse` |
| `services/token-service/src/state.rs` | 增字段（如果 state 在独立文件） |
| `services/iam-service/src/main.rs` | 启动时拉 JWKS + service-jwt；AppState 增字段；/iam/internal 加 `verify_aud` middleware；outbound 调 token-service 用 service-jwt 注入 |
| `services/iam-service/Cargo.toml` | 增 `common-jwt` 依赖 |
| `services/iam-service/src/routes/internal_users.rs` | 注入强制 aud="iam-service" 的 Principal 提取（已在 `b5ae6dd` 加 Principal，但无验签） |
| `services/token-service/src/lib.rs` / `routes/refresh.rs` / `routes/access.rs` | `fetch_json` 调用现传 `None` 的位置改为 `Some(user_jwt)`（来自 request）—— **前提是调用方能拿到 user-jwt**；access/refresh 路径目前没 user-jwt，仍是 `None`，但 code path 完整 |
| `services/notification-service/src/main.rs` | 启动 boilerplate（JWKS + service-jwt）；AppState；改 iam_client 调用传 service-jwt 替代 `None` |
| `services/notification-service/Cargo.toml` | 增 `common-jwt` 依赖 |
| `services/audit-service/src/main.rs` / `Cargo.toml` | 同上 |
| `services/api-gateway/src/main.rs` / `Cargo.toml` | 启动拉 JWKS；user-jwt 验证 (aud=user)；outbound 不需要 service-jwt（gateway 不调其他 service） |
| `services/prue_backend/src/main.rs` / `Cargo.toml` | 启动拉 JWKS + service-jwt；如有 outbound call 加 service-jwt 注入 |
| `services/iam-outbox-publisher/src/main.rs` | 启动拉 service-jwt；outbound RabbitMQ 不需要（业务调用 iam 是 SQL 直连） |
| `services/notification-outbox-publisher/src/main.rs` | 同上 |
| `infrastructure/docker-compose-services.yml` | 每个 service 增 `SERVICE_BOOTSTRAP_TOKEN` / `TENANT_ID` / `TOKEN_SERVICE_URL` env；token-service 增 `SERVICE_BOOTSTRAP_TOKENS`（多个 token，逗号分隔） |
| `infrastructure/.env` 或 secrets 文件 | bootstrap tokens 实际值（256-bit random，dev 用固定 string 例如 `dev-token-iam-...`） |
| `infrastructure/config/api-gateway/api-gateway.toml` | 增 `service_jwt` 块（如果 gateway 需要 outbound） |
| `docs-site/ai-context.md` | §2 服务地图加 service-jwt 列；§3 架构加 common-jwt；§11 TODO 标记 phase5c |
| `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md` | §2.2 「Phase 5-B: JWT 签名验证」段标 superseded |

### 6.3 共享依赖增删

```toml
# Cargo.toml [workspace.dependencies] 新增
jsonwebtoken = { version = "9", default-features = false }
# 各 service Cargo.toml 新增
common-jwt = { workspace = true }
```

### 6.4 共享依赖不动

`reqwest` / `serde` / `tokio` / `thiserror` 已经在 workspace 复用；`jsonwebtoken` 是唯一新增外部 crate。

---

## 7. 验证步骤

### 7.1 单元测试

**`libs/common-jwt`**：
- `JWKSCache::refresh` 解析 mock JWKS，keys HashMap 包含期望 kid
- `verify_token` 对 valid / expired / wrong-sig / wrong-aud / wrong-kid 各返回对应错误
- `sign_token` 产出 token 立即能 verify 回原 claims
- `ServiceJwtCache::get` 第一次触发 refresh，后续命中 cache；临近 exp 自动 refresh

**`libs/jwt-ctx`**：
- `Principal::from_token` 接受 valid token 返回 Principal
- 接受 expired 返回 `VerifyError::Expired`
- 接受 wrong-sig 返回 `VerifyError::SignatureInvalid`
- `Principal::from_jwt_payload`（旧）行为不变

### 7.2 集成测试

**token-service `POST /tokens/service-jwt`**：
- valid bootstrap token + valid body → 200，access_token 是 RS256 JWT，verify 用 public key 成功
- missing / wrong bootstrap token → 401
- missing aud → 400
- claims 包含 iss=token-service、sub=SERVICE_NAME、aud=requested、roles=[service]

**service-side**：
- 启动时 mock token-service 端点，service 能拉 JWKS + service-jwt
- 出站 ResilientClient 自动注入 Authorization 头（mock token-service 验签通过）
- service-jwt 过期前 1min 后台 refresh

### 7.3 端到端（CI）

`pnpm test:e2e` 跑跨 service 关键 path：

- 端到端：用户登录（gateway → token-service → iam）→ token 验签通过 → JWT 携带
- 端到端：notification 发布公告（notification → iam by-role）→ service-jwt 验签通过
- 端到端：audience 不匹配（手动篡改 aud）→ iam 返回 401

### 7.4 Acceptance criteria

1. `cargo test --workspace --lib` 全绿
2. `node scripts/check-spec.js` 绿
3. `pnpm test:e2e` 全绿
4. `cargo audit` 干净（无新 RUSTSEC 警告）
5. dev compose 启动后，notification → iam 实际调用 200（不再 401）
6. JWKS 缓存错配场景手动验证：手动改 token-service 的 private key，service 仍能 1h 后自动 refresh（mock）

---

## 8. 风险

| 风险 | 缓解 |
|---|---|
| `jsonwebtoken` 9.x 引入新 CVE | 锁版本（`=9.x`），`cargo audit` 在 CI |
| service-jwt 1h TTL 太短导致服务 restart 时 burst | 把 TTL 提到 4h（bootstrap token 仍是 1次交换） |
| Bootstrap token 通过 env 注入，泄漏风险 | 文档要求生产用 secret manager（Vault / K8s Secret）；dev 写明 .env 不入 git |
| Service 主进程 panic if 启动时 JWKS 拉不到 | 启动 retry 3 次（间隔 1s/2s/4s），仍 fail 才 panic；K8s pod 重新调度解决 |
| common-jwt lib 集中化后任一 service 升级 lag 整个 mesh 失效 | lib 接口稳定（avro-style），独立 versioned；变更必须 e2e 验证 |
| audience middleware 漏配（漏 `.layer()`） | 写一个 integration test 跑遍所有 internal 端点 + 各种 aud |
| 服务间调用循环（a→b→a 死锁） | circuit breaker（已有 ResilientClient） + service-jwt 不跨租户传递 |
| 现有 authz 测试（如果有）失败 | Phase 5-B 第一周就 run test，确认 0 failures 才能继续 |

---

## 9. 关联文档 / 后续 TODO

- 后续 spec：`2026-XX-XX-phase5b-tls-termination-design.md`（spec §11 已埋）—— dev 仍 HTTP，生产路径需补 alb / nginx 终结 TLS
- 后续 spec：`2026-XX-XX-phase5b-event-sync-design.md`（spec §0.5.6 #2）—— notification → iam 事件化
- 后续 spec：`2026-XX-XX-phase5c-refresh-revocation-design.md`（spec §0.5.5 #1）—— refresh_token 撤销与 JWT 验签的耦合
- 关联 ADR：本 spec §0.5 + plan `2026-XX-XX-phase5b-jwt-signature-and-service-jwt.md` 实施 ADR

---

## 10. 决策日志

### 10.1 已发生

- 2026-07-07 12:30+0800 — 5a-2 落地 commit `b8955b7`；spec §0.5.6 标 4 个 open question
- 2026-07-07 13:00+0800 — 用户选 5b-auth / JWKS / token-service / 全栈 / new common-jwt
- 2026-07-07 13:30+0800 — 9 个 design section 草稿
- 2026-07-07 14:00+0800 — spec v1 写完（待评审）

### 10.2 待发生

- 2026-07-XX — 用户 sign-off
- 2026-07-XX — 写 plan `2026-XX-XX-phase5b-jwt-signature-and-service-jwt.md`
- 2026-07-XX — Phase A 开始（common-jwt crate）
- ...

---

## 11. spec 内部审稿

### 11.1 占位符扫描

- §0.5.5 「开放问题」3 个：标记为暂定答案，不假装已解决
- §3.4.1 `with_authorization` 参数：API 形态，spec 一致
- §3.4.3 「TENANT_ID」 env 变量名：与现有 `iam-service` 等的「tid」claim 关联，名称需 review

### 11.2 内部一致性

- §0.5.4 理由 2 说「Principal extractor 是空壳」，§1.1 也说「`from_jwt_payload` 只 decode 不 verify」—— 一致
- §3.2 `from_token` 是 async；§3.5 middleware 在 service main 注册 —— 一致
- §3.3 端点路径 `/tokens/service-jwt`；§2.2 也用这个路径 —— 一致
- §6.2 各 service Cargo.toml 增 `common-jwt`，§3.4.3 boilerplate 引用 common_jwt —— 一致

### 11.3 Scope 检查

- 单 spec 覆盖：① crypto lib 新建 ② token-service 端点 ③ jwt-ctx 改造 ④ 7 service 迁移 ⑤ doc ⑥ CI 验收
- 跨子系统风险：authz middleware 重用 — 但 authz 库已经成熟（plan 2026-06-29 §6 已用），无大改
- 范围适合一个 plan 落地（12 commits），不需拆

### 11.4 歧义检查

- §3.1 `expected_aud: Option<&str>` —— 写明 None 跳过，service-to-service 强制，gateway 路径可选
- §3.3 bootstrap token 来源 —— `SERVICE_BOOTSTRAP_TOKENS` env，逗号分隔，注入 `TokenServerState`
- §3.4.2 `ServiceJwtCache::get(target)` —— 缓存按 target 分键（一个 service 可能调多个被调方，token 不同 aud）
- §3.5 verify_aud middleware vs Principal extractor —— 拆成两个 layer：Principal 提取 + aud 校验

### 11.5 reviewer 可能问

1. **为什么不用 HS256 共享密钥？** —— spec §0.5.2 已答（与现代最佳实践相悖）
2. **为什么 bootstrap token 不走 mTLS？** —— 同上，phase5a-2 已撤 mesh
3. **Service-jwt 跟 user-jwt 同一对 key 安全吗？** —— 安全，泄漏 private key 的后果是双重的；但运维简化（rotation 一次）。生产应拆分（spec §0.5.3 #3 推）
4. **为什么 cache JWKS 而不是 hardcode PEM？** —— §0.5.2，dev / prod key 切换免重 build
5. **10 commits 一个 spec 不嫌大？** —— 拆分 phase（§8）：A lib → B 端点 → C jwt-ctx → D1-D7 业务 service → E doc → F 验收
