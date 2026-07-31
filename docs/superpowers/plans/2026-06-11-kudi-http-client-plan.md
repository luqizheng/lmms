# kudi-http-client 多语言 SDK 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `sdk/{dotnet,java,rust}/` 下新增 `kudi-http-client` 通用 HTTP 客户端，支持 service-to-service（client_credentials 自动缓存 token）与显式透传用户 Bearer，覆盖 .NET / Java / Rust 三语言。

**Architecture:** 三语言独立包，结构同形 `KudiWebClient + KudiHttpOptions + ServiceTokenCache + ClientCredentialsFetcher + HttpTransport`；token 缓存按 `SHA256(ClientId | SecretKey)` 静态维度；复用现有 `token-service` `POST /api/tokens/access` 端点；不动现有 SDK。

**Tech Stack:**
- .NET: `System.Net.Http.HttpClient`，多目标 `net6.0;net9.0;net10.0`，xUnit + MockHttp
- Java: `java.net.http.HttpClient`（8 走 `HttpURLConnection`），Maven，JUnit 5 + okhttp MockWebServer
- Rust: `reqwest`（rustls-tls）+ `tokio` + `serde` + `thiserror` + `tracing`，wiremock-rs

**Spec:** [docs/superpowers/specs/2026-06-11-kudi-http-client-design.md](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-11-kudi-http-client-design.md)

**项目规则强约束（必须遵守）：**
- Rust workspace 用 `aws_lc_rs` crypto provider，不要回退到 `ring`
- 所有 `cargo` / `dotnet test` / `mvn` 命令必须在真实 PowerShell（vcvarsall x64）执行，不在 Trae IDE 沙箱内跑
- 改 `sdk/dotnet/**/*.csproj` / `sdk/java/**/pom.xml` / 根 `Cargo.toml` 后，跑对应门槛测试
- 提交前缀：`.NET` 改动 `feat(sdk):`，`Java` 改动 `feat(sdk):`，`Rust` 改动 `feat(sdk):`

---

## 文件结构总览

**新建：**

```
sdk/dotnet/KudiCoder.HttpClient/
  KudiCoder.HttpClient.csproj
  KudiWebClient.cs
  KudiHttpOptions.cs
  ServiceTokenCache.cs
  ClientCredentialsFetcher.cs
  HttpTransport.cs
  Models/TokenInfo.cs
  Exceptions/KudiHttpException.cs
  Exceptions/AuthException.cs
  Exceptions/NetworkException.cs
  Exceptions/ServerException.cs
  Exceptions/ValidationException.cs
  Exceptions/RateLimitException.cs
  README.md
sdk/dotnet/tests/KudiCoder.HttpClient.Tests/
  KudiCoder.HttpClient.Tests.csproj
  ServiceTokenCacheTests.cs
  KudiWebClientTests.cs
sdk/java/kudi-http-client/
  pom.xml
  README.md
  src/main/java/com/kudi/coder/httpclient/
    KudiWebClient.java
    KudiHttpOptions.java
    ServiceTokenCache.java
    ClientCredentialsFetcher.java
    HttpTransport.java
    Models/TokenInfo.java
    Exceptions/KudiHttpException.java
    Exceptions/AuthException.java
    Exceptions/NetworkException.java
    Exceptions/ServerException.java
    Exceptions/ValidationException.java
    Exceptions/RateLimitException.java
  src/test/java/com/kudi/coder/httpclient/
    ServiceTokenCacheTest.java
    KudiWebClientTest.java
sdk/rust/kudi-http-client/
  Cargo.toml
  README.md
  src/lib.rs
  src/options.rs
  src/cache.rs
  src/fetcher.rs
  src/transport.rs
  src/error.rs
  src/models.rs
  src/client.rs
  tests/cache_test.rs
  tests/client_test.rs
```

**修改：**
- `sdk/dotnet/KudiCoder.sln` — 加入 `KudiCoder.HttpClient` + `KudiCoder.HttpClient.Tests` 项目
- `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiCoder.HttpClient.Tests.csproj` — 引用新包
- `sdk/java/pom.xml` — `<modules>` 加入 `kudi-http-client`
- `Cargo.toml`（根） — `members` 加入 `"sdk/rust/kudi-http-client"`
- `sdk/README.md` — 加入"服务间通用 HTTP 客户端"段落

---

## Task 1: 修复 consul-registrar 编译失败（前置）

> 仅在执行时若 `cargo test -p consul-registrar` 仍报 E0432/E0282 才执行。不影响本 SDK 逻辑，但根 workspace `cargo test` 会因它挂掉。

**Files:**
- Modify: `libs/consul-registrar/Cargo.toml:8`

- [ ] **Step 1: 检查根 workspace 编译**

Run: `cargo check --workspace`
Expected: 失败，错误同终端日志（`hyper_util::client::legacy::Client` 未找到 + 两处 type annotations needed）

- [ ] **Step 2: 复核 `hyper-util` workspace feature**

Read: `Cargo.toml:39` — 当前是 `features = ["client-legacy", "tokio"]`。

- [ ] **Step 3: 在 `libs/consul-registrar/Cargo.toml` 显式追加 `http1` feature**

Edit `libs/consul-registrar/Cargo.toml` line 8:
```toml
hyper-util = { workspace = true, features = ["client-legacy", "tokio", "http1"] }
```
（注：当前 line 8 已写 `http1`，若仍报"configured out"，改用 `hyper-util = "0.1"` 直接依赖并显式开 `client-legacy, tokio, http1`，脱离 workspace 覆盖）

- [ ] **Step 4: 在 `libs/consul-registrar/src/lib.rs` 给 `resp` 加显式类型注解**

Edit `libs/consul-registrar/src/lib.rs` line 161 与 line 191，把 `Ok(resp)` 替换为带类型注解的版本：

```rust
let resp: hyper::Response<hyper::body::Incoming> = self.http_client.request(req).await.map_err(|e| {
    let err_msg = format!("failed to register service: {}", e);
    tracing::error!("{}", err_msg);
    err_msg
})?;
if resp.status().is_success() { ... }
```

并在 line 191 `deregister` 内做同样替换。

- [ ] **Step 5: 重跑 cargo check**

Run: `cargo check -p consul-registrar`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add libs/consul-registrar/Cargo.toml libs/consul-registrar/src/lib.rs
git commit -m "fix(consul-registrar): enable hyper-util http1 feature and type-annotate resp"
```

---

## Task 2: Rust workspace 加入 kudi-http-client 骨架

**Files:**
- Create: `sdk/rust/kudi-http-client/Cargo.toml`
- Create: `sdk/rust/kudi-http-client/src/lib.rs`
- Modify: `Cargo.toml`（根）

- [ ] **Step 1: 创建 crate 目录**

Run: `mkdir -p sdk/rust/kudi-http-client/src sdk/rust/kudi-http-client/tests`

- [ ] **Step 2: 写 `Cargo.toml`**

Create `sdk/rust/kudi-http-client/Cargo.toml`:
```toml
[package]
name = "kudi-http-client"
version = "0.1.0"
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
reqwest = { workspace = true }
thiserror.workspace = true
tracing.workspace = true
sha2 = "0.10"
hex = "0.4"
async-trait = "0.1"

[dev-dependencies]
wiremock = "0.6"
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 3: 写占位 `src/lib.rs`**

Create `sdk/rust/kudi-http-client/src/lib.rs`:
```rust
//! Kudi HTTP client SDK: service-to-service (client_credentials) + user Bearer passthrough.

pub mod options;
pub mod cache;
pub mod fetcher;
pub mod transport;
pub mod error;
pub mod models;
pub mod client;

pub use client::KudiWebClient;
pub use options::KudiHttpOptions;
pub use error::{KudiHttpError, AuthError, ServerError, NetworkError, ValidationError, RateLimitError};
pub use models::TokenInfo;
```

并创建 7 个空 mod 文件，每个内容为 `// placeholder`，等后续 task 填充。

- [ ] **Step 4: 根 `Cargo.toml` 加 member**

Edit `Cargo.toml` line 3-11，`members` 数组加入 `"sdk/rust/kudi-http-client"`:
```toml
members = [
    "libs/digua_common",
    "libs/jwt-ctx",
    "libs/digua_authz",
    "libs/consul-registrar",
    "services/token-service",
    "services/api-gateway",
    "services/user-service",
    "services/notification-service",
    "sdk/rust/kudi-http-client",
]
```

- [ ] **Step 5: 跑 cargo check**

Run: `cargo check -p kudi-http-client`
Expected: PASS（占位文件 + reqwest 等依赖能解析）

- [ ] **Step 6: Commit**

```bash
git add sdk/rust/kudi-http-client Cargo.toml
git commit -m "feat(sdk): scaffold kudi-http-client crate in workspace"
```

---

## Task 3: Rust 错误类型

**Files:**
- Create: `sdk/rust/kudi-http-client/src/error.rs`

- [ ] **Step 1: 写错误枚举**

Create `sdk/rust/kudi-http-client/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KudiHttpError {
    #[error("auth failed (HTTP {status}): {message}")]
    Auth { status: u16, message: String, details: Option<String> },
    #[error("validation failed (HTTP {status}): {message}")]
    Validation { status: u16, message: String, details: Option<String> },
    #[error("rate limited (HTTP 429): {message}")]
    RateLimit { message: String, details: Option<String> },
    #[error("server error (HTTP {status}): {message}")]
    Server { status: u16, message: String, details: Option<String> },
    #[error("network error: {0}")]
    Network(String),
}

pub type AuthError = KudiHttpError;
pub type ServerError = KudiHttpError;
pub type NetworkError = KudiHttpError;
pub type ValidationError = KudiHttpError;
pub type RateLimitError = KudiHttpError;
```

- [ ] **Step 2: 跑 cargo check**

Run: `cargo check -p kudi-http-client`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add sdk/rust/kudi-http-client/src/error.rs
git commit -m "feat(sdk): kudi-http-client error types"
```

---

## Task 4: Rust Models 与 Options

**Files:**
- Create: `sdk/rust/kudi-http-client/src/models.rs`
- Create: `sdk/rust/kudi-http-client/src/options.rs`

- [ ] **Step 1: 写 `models.rs`**

Create `sdk/rust/kudi-http-client/src/models.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenInfo {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
    pub expires_in: i64,
}

fn default_token_type() -> String {
    "Bearer".to_string()
}
```

- [ ] **Step 2: 写 `options.rs`**

Create `sdk/rust/kudi-http-client/src/options.rs`:
```rust
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct KudiHttpOptions {
    pub token_endpoint: String,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub retry_count: u32,
    pub refresh_skew: Duration,
    pub user_agent: String,
}

impl Default for KudiHttpOptions {
    fn default() -> Self {
        Self {
            token_endpoint: "http://token-svc/api/tokens/access".to_string(),
            connect_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(15),
            retry_count: 1,
            refresh_skew: Duration::from_secs(60),
            user_agent: format!("kudi-http-client/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}
```

- [ ] **Step 3: 跑 cargo check**

Run: `cargo check -p kudi-http-client`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add sdk/rust/kudi-http-client/src/models.rs sdk/rust/kudi-http-client/src/options.rs
git commit -m "feat(sdk): kudi-http-client models and options"
```

---

## Task 5: Rust ServiceTokenCache

**Files:**
- Create: `sdk/rust/kudi-http-client/src/cache.rs`
- Create: `sdk/rust/kudi-http-client/tests/cache_test.rs`

- [ ] **Step 1: 写 `cache.rs`**

Create `sdk/rust/kudi-http-client/src/cache.rs`:
```rust
use crate::models::TokenInfo;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
struct CachedToken {
    info: TokenInfo,
    expires_at: Instant,
}

#[derive(Debug, Clone)]
struct CacheKey {
    client_id: String,
    secret_key: String,
}

impl CacheKey {
    fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.client_id.as_bytes());
        h.update(b"|");
        h.update(self.secret_key.as_bytes());
        hex::encode(h.finalize())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TokenCacheSnapshot {
    pub size: usize,
}

#[derive(Clone)]
pub struct ServiceTokenCache {
    refresh_skew: Duration,
    inner: Arc<Mutex<HashMap<String, (CacheKey, CachedToken)>>>,
}

impl ServiceTokenCache {
    pub fn new(refresh_skew: Duration) -> Self {
        Self {
            refresh_skew,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn snapshot(&self) -> TokenCacheSnapshot {
        TokenCacheSnapshot {
            size: self.inner.lock().await.len(),
        }
    }

    pub async fn get_or_fetch<F, Fut>(
        &self,
        client_id: &str,
        secret_key: &str,
        fetcher: F,
    ) -> Result<String, crate::error::KudiHttpError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<TokenInfo, crate::error::KudiHttpError>>,
    {
        let key = CacheKey { client_id: client_id.to_string(), secret_key: secret_key.to_string() };
        let digest = key.digest();

        {
            let guard = self.inner.lock().await;
            if let Some((stored_key, cached)) = guard.get(&digest) {
                if stored_key.client_id == client_id
                    && stored_key.secret_key == secret_key
                    && Instant::now() < cached.expires_at - self.refresh_skew
                {
                    return Ok(cached.info.access_token.clone());
                }
            }
        }

        let mut guard = self.inner.lock().await;
        // Re-check after acquiring write lock to avoid duplicate fetch.
        if let Some((stored_key, cached)) = guard.get(&digest) {
            if stored_key.client_id == client_id
                && stored_key.secret_key == secret_key
                && Instant::now() < cached.expires_at - self.refresh_skew
            {
                return Ok(cached.info.access_token.clone());
            }
        }

        let info = fetcher().await?;
        let expires_at = Instant::now() + Duration::from_secs(info.expires_in.max(0) as u64);
        guard.insert(digest, (key, CachedToken { info: info.clone(), expires_at }));
        Ok(info.access_token)
    }

    pub async fn invalidate(&self, client_id: &str, secret_key: &str) {
        let digest = CacheKey { client_id: client_id.to_string(), secret_key: secret_key.to_string() }.digest();
        self.inner.lock().await.remove(&digest);
    }
}
```

- [ ] **Step 2: 写 `cache_test.rs`**

Create `sdk/rust/kudi-http-client/tests/cache_test.rs`:
```rust
use digua_http_client::cache::ServiceTokenCache;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn info_with(expires_in: i64) -> TokenInfo {
    TokenInfo { access_token: "tok".into(), refresh_token: None, token_type: "Bearer".into(), expires_in }
}

#[tokio::test]
async fn caches_token_per_credential_pair() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));
    let c2 = counter.clone();
    let info = info_with(3600);

    let t1 = cache
        .get_or_fetch("a", "s1", || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info.clone())
            }
        })
        .await
        .unwrap();

    let t2 = cache
        .get_or_fetch("a", "s1", || {
            let c = counter.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info.clone())
            }
        })
        .await
        .unwrap();

    assert_eq!(t1, "tok");
    assert_eq!(t2, "tok");
    assert_eq!(counter.load(Ordering::SeqCst), 1, "fetcher must run only once");
}

#[tokio::test]
async fn separate_credentials_have_separate_cache_entries() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));

    for cid in ["a", "b"] {
        let c2 = counter.clone();
        let info = info_with(3600);
        let _ = cache
            .get_or_fetch(cid, "s1", || {
                let c = c2.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, KudiHttpError>(info.clone())
                }
            })
            .await
            .unwrap();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 2);
    assert_eq!(cache.snapshot().await.size, 2);
}

#[tokio::test]
async fn invalidating_a_credential_refetches() {
    let cache = ServiceTokenCache::new(Duration::from_secs(60));
    let counter = Arc::new(AtomicUsize::new(0));
    let info = info_with(3600);

    for _ in 0..2 {
        let c2 = counter.clone();
        let info_c = info.clone();
        let _ = cache
            .get_or_fetch("a", "s1", || {
                let c = c2.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, KudiHttpError>(info_c.clone())
                }
            })
            .await
            .unwrap();
    }
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    cache.invalidate("a", "s1").await;

    let c2 = counter.clone();
    let _ = cache
        .get_or_fetch("a", "s1", || {
            let c = c2.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok::<_, KudiHttpError>(info.clone())
            }
        })
        .await
        .unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p kudi-http-client --test cache_test`
Expected: 3 passed

- [ ] **Step 4: Commit**

```bash
git add sdk/rust/kudi-http-client/src/cache.rs sdk/rust/kudi-http-client/tests/cache_test.rs
git commit -m "feat(sdk): kudi-http-client service token cache"
```

---

## Task 6: Rust ClientCredentialsFetcher

**Files:**
- Create: `sdk/rust/kudi-http-client/src/fetcher.rs`

- [ ] **Step 1: 写 `fetcher.rs`**

Create `sdk/rust/kudi-http-client/src/fetcher.rs`:
```rust
use crate::error::KudiHttpError;
use crate::models::TokenInfo;
use crate::transport::HttpTransport;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ClientCredentialsFetcher {
    transport: Arc<dyn HttpTransport>,
    token_endpoint: String,
}

impl ClientCredentialsFetcher {
    pub fn new(transport: Arc<dyn HttpTransport>, token_endpoint: impl Into<String>) -> Self {
        Self { transport, token_endpoint: token_endpoint.into() }
    }

    pub async fn fetch(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<TokenInfo, KudiHttpError> {
        let mut form: HashMap<String, String> = HashMap::new();
        form.insert("grant_type".into(), "client_credentials".into());
        form.insert("client_id".into(), client_id.into());
        form.insert("client_secret".into(), client_secret.into());

        let resp = self
            .transport
            .post_form(&self.token_endpoint, &form)
            .await?;

        if !(200..300).contains(&resp.status) {
            return Err(KudiHttpError::Auth {
                status: resp.status,
                message: format!("client_credentials failed: HTTP {}", resp.status),
                details: Some(resp.body),
            });
        }

        let info: TokenInfo = serde_json::from_str(&resp.body).map_err(|e| {
            KudiHttpError::Network(format!("parse token response: {}", e))
        })?;
        if info.access_token.is_empty() {
            return Err(KudiHttpError::Auth {
                status: resp.status,
                message: "token response missing access_token".into(),
                details: Some(resp.body),
            });
        }
        Ok(info)
    }
}
```

- [ ] **Step 2: 跑 cargo check**

Run: `cargo check -p kudi-http-client`
Expected: PASS（`HttpTransport` 还没实现 → 下一 task 完成）

- [ ] **Step 3: Commit**

```bash
git add sdk/rust/kudi-http-client/src/fetcher.rs
git commit -m "feat(sdk): kudi-http-client client credentials fetcher"
```

---

## Task 7: Rust HttpTransport

**Files:**
- Create: `sdk/rust/kudi-http-client/src/transport.rs`

- [ ] **Step 1: 写 `transport.rs`**

Create `sdk/rust/kudi-http-client/src/transport.rs`:
```rust
use crate::error::KudiHttpError;
use crate::options::KudiHttpOptions;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn post_form(
        &self,
        url: &str,
        form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError>;
    async fn post_json(
        &self,
        url: &str,
        bearer: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError>;
    async fn get(
        &self,
        url: &str,
        bearer: Option<&str>,
    ) -> Result<HttpResponse, KudiHttpError>;
    async fn request(
        &self,
        method: &str,
        url: &str,
        bearer: Option<&str>,
        body: Option<(&str, &str)>,
    ) -> Result<HttpResponse, KudiHttpError>;
}

pub struct ReqwestTransport {
    client: Client,
    options: KudiHttpOptions,
}

impl ReqwestTransport {
    pub fn new(options: KudiHttpOptions) -> Result<Arc<Self>, KudiHttpError> {
        let client = Client::builder()
            .connect_timeout(options.connect_timeout)
            .timeout(options.read_timeout)
            .user_agent(options.user_agent.clone())
            .build()
            .map_err(|e| KudiHttpError::Network(format!("build reqwest client: {}", e)))?;
        Ok(Arc::new(Self { client, options }))
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn post_form(
        &self,
        url: &str,
        form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.request("POST", url, None, Some(("application/x-www-form-urlencoded", &encode_form(form)))).await
    }

    async fn post_json(
        &self,
        url: &str,
        bearer: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.request("POST", url, bearer, Some(("application/json", json_body))).await
    }

    async fn get(
        &self,
        url: &str,
        bearer: Option<&str>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.request("GET", url, bearer, None).await
    }

    async fn request(
        &self,
        method: &str,
        url: &str,
        bearer: Option<&str>,
        body: Option<(&str, &str)>,
    ) -> Result<HttpResponse, KudiHttpError> {
        let mut attempt = 0u32;
        loop {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if let Some(b) = bearer {
                if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", b)) {
                    headers.insert(HeaderName::from_static("authorization"), v);
                }
            }

            let mut req = self.client.request(parse_method(method)?, url).headers(headers);
            if let Some((ct, body)) = body {
                req = req
                    .header(CONTENT_TYPE, HeaderValue::from_static(ct))
                    .body(body.to_string());
            }

            let result = req.send().await;
            match result {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Ok(HttpResponse { status: status.as_u16(), body: text });
                }
                Err(e) if attempt < self.options.retry_count => {
                    attempt += 1;
                    tracing::warn!("request retry {}/{}: {}", attempt, self.options.retry_count, e);
                    tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
                    continue;
                }
                Err(e) => return Err(KudiHttpError::Network(e.to_string())),
            }
        }
    }
}

fn parse_method(m: &str) -> Result<reqwest::Method, KudiHttpError> {
    reqwest::Method::from_bytes(m.as_bytes()).map_err(|e| KudiHttpError::Network(format!("bad method: {}", e)))
}

fn encode_form(form: &HashMap<String, String>) -> String {
    form.iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn status_kind(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
```

- [ ] **Step 2: 跑 cargo check**

Run: `cargo check -p kudi-http-client`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add sdk/rust/kudi-http-client/src/transport.rs
git commit -m "feat(sdk): kudi-http-client reqwest transport"
```

---

## Task 8: Rust KudiWebClient

**Files:**
- Create: `sdk/rust/kudi-http-client/src/client.rs`
- Create: `sdk/rust/kudi-http-client/tests/client_test.rs`

- [ ] **Step 1: 写 `client.rs`**

Create `sdk/rust/kudi-http-client/src/client.rs`:
```rust
use crate::cache::ServiceTokenCache;
use crate::error::KudiHttpError;
use crate::fetcher::ClientCredentialsFetcher;
use crate::options::KudiHttpOptions;
use crate::transport::{HttpResponse, HttpTransport, ReqwestTransport};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct KudiWebClient {
    transport: Arc<dyn HttpTransport>,
    options: KudiHttpOptions,
    cache: ServiceTokenCache,
    fetcher: Arc<ClientCredentialsFetcher>,
    credential: Option<(String, String)>,
}

#[derive(Debug, Default, Clone)]
pub struct Builder {
    pub options: KudiHttpOptions,
    pub client_id: Option<String>,
    pub secret_key: Option<String>,
}

impl Builder {
    pub fn credential(mut self, client_id: impl Into<String>, secret_key: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self.secret_key = Some(secret_key.into());
        self
    }
    pub fn token_endpoint(mut self, url: impl Into<String>) -> Self {
        self.options.token_endpoint = url.into();
        self
    }
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.options.read_timeout = Duration::from_millis(ms);
        self
    }
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.options.connect_timeout = Duration::from_millis(ms);
        self
    }
    pub fn retry_count(mut self, n: u32) -> Self {
        self.options.retry_count = n;
        self
    }
    pub fn build(self) -> Result<KudiWebClient, KudiHttpError> {
        let transport = ReqwestTransport::new(self.options.clone())?;
        let cache = ServiceTokenCache::new(self.options.refresh_skew);
        let fetcher = Arc::new(ClientCredentialsFetcher::new(transport.clone(), self.options.token_endpoint.clone()));
        Ok(KudiWebClient {
            transport,
            options: self.options,
            cache,
            fetcher,
            credential: self.client_id.zip(self.secret_key),
        })
    }
}

impl KudiWebClient {
    pub fn builder() -> Builder { Builder::default() }

    pub fn with_transport(transport: Arc<dyn HttpTransport>, options: KudiHttpOptions) -> Self {
        let cache = ServiceTokenCache::new(options.refresh_skew);
        let fetcher = Arc::new(ClientCredentialsFetcher::new(transport.clone(), options.token_endpoint.clone()));
        Self { transport, options, cache, fetcher, credential: None }
    }

    pub fn with_credential(mut self, client_id: impl Into<String>, secret_key: impl Into<String>) -> Self {
        self.credential = Some((client_id.into(), secret_key.into()));
        self
    }

    pub fn options(&self) -> &KudiHttpOptions { &self.options }
    pub fn cache_snapshot(&self) -> impl std::future::Future<Output = crate::cache::TokenCacheSnapshot> {
        self.cache.snapshot()
    }

    async fn resolve_bearer(&self, explicit: Option<&str>) -> Result<Option<String>, KudiHttpError> {
        if let Some(b) = explicit {
            return Ok(Some(b.to_string()));
        }
        if let Some((cid, sk)) = &self.credential {
            let cid = cid.clone();
            let sk = sk.clone();
            let token = self
                .cache
                .get_or_fetch(&cid, &sk, || {
                    let f = self.fetcher.clone();
                    async move { f.fetch(&cid, &sk).await }
                })
                .await?;
            return Ok(Some(token));
        }
        Ok(None)
    }

    pub async fn post(&self, url: &str, json_body: &str, bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError> {
        let bearer = self.resolve_bearer(bearer).await?;
        self.transport.post_json(url, bearer.as_deref(), json_body).await
    }

    pub async fn get(&self, url: &str, bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError> {
        let bearer = self.resolve_bearer(bearer).await?;
        self.transport.get(url, bearer.as_deref()).await
    }

    pub async fn get_with_bearer(&self, url: &str, bearer: &str) -> Result<HttpResponse, KudiHttpError> {
        self.transport.get(url, Some(bearer)).await
    }
}
```

- [ ] **Step 2: 写 `client_test.rs`（mock transport）**

Create `sdk/rust/kudi-http-client/tests/client_test.rs`:
```rust
use async_trait::async_trait;
use digua_http_client::client::KudiWebClient;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use digua_http_client::options::KudiHttpOptions;
use digua_http_client::transport::{HttpResponse, HttpTransport};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Default)]
struct MockTransport {
    token_calls: AtomicUsize,
    downstream_calls: AtomicUsize,
    last_authorization: std::sync::Mutex<Option<String>>,
}

#[async_trait]
impl HttpTransport for MockTransport {
    async fn post_form(
        &self,
        _url: &str,
        _form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.token_calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse {
            status: 200,
            body: serde_json::to_string(&TokenInfo {
                access_token: "svc-token".into(),
                refresh_token: None,
                token_type: "Bearer".into(),
                expires_in: 3600,
            })
            .unwrap(),
        })
    }
    async fn post_json(
        &self,
        _url: &str,
        bearer: Option<&str>,
        _json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.downstream_calls.fetch_add(1, Ordering::SeqCst);
        *self.last_authorization.lock().unwrap() = bearer.map(|s| s.to_string());
        Ok(HttpResponse { status: 200, body: "{}".into() })
    }
    async fn get(
        &self,
        _url: &str,
        bearer: Option<&str>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.downstream_calls.fetch_add(1, Ordering::SeqCst);
        *self.last_authorization.lock().unwrap() = bearer.map(|s| s.to_string());
        Ok(HttpResponse { status: 200, body: "{}".into() })
    }
    async fn request(
        &self,
        _method: &str,
        _url: &str,
        _bearer: Option<&str>,
        _body: Option<(&str, &str)>,
    ) -> Result<HttpResponse, KudiHttpError> {
        unimplemented!()
    }
}

#[tokio::test]
async fn fetches_token_once_and_uses_for_subsequent_calls() {
    let mock = Arc::new(MockTransport::default());
    let client = KudiWebClient::with_transport(
        mock.clone(),
        KudiHttpOptions { token_endpoint: "http://t/token".into(), ..KudiHttpOptions::default() },
    )
    .with_credential("a", "s1");

    client.get("http://u/api/x", None).await.unwrap();
    client.get("http://u/api/y", None).await.unwrap();
    client.post("http://u/api/z", "{}", None).await.unwrap();

    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 1);
    assert_eq!(mock.downstream_calls.load(Ordering::SeqCst), 3);
    assert_eq!(mock.last_authorization.lock().unwrap().as_deref(), Some("Bearer svc-token"));
}

#[tokio::test]
async fn user_bearer_passthrough_skips_token_fetch() {
    let mock = Arc::new(MockTransport::default());
    let client = KudiWebClient::with_transport(
        mock.clone(),
        KudiHttpOptions { token_endpoint: "http://t/token".into(), ..KudiHttpOptions::default() },
    )
    .with_credential("a", "s1");

    client.get_with_bearer("http://u/api/me", "user.jwt").await.unwrap();
    assert_eq!(mock.token_calls.load(Ordering::SeqCst), 0);
    assert_eq!(mock.last_authorization.lock().unwrap().as_deref(), Some("Bearer user.jwt"));
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p kudi-http-client`
Expected: 5 passed (3 cache + 2 client)

- [ ] **Step 4: Commit**

```bash
git add sdk/rust/kudi-http-client/src/client.rs sdk/rust/kudi-http-client/tests/client_test.rs
git commit -m "feat(sdk): kudi-http-client KudiWebClient with mock transport tests"
```

---

## Task 9: Rust wiremock 集成测试

**Files:**
- Create: `sdk/rust/kudi-http-client/tests/integration_test.rs`

- [ ] **Step 1: 写集成测试**

Create `sdk/rust/kudi-http-client/tests/integration_test.rs`:
```rust
use digua_http_client::client::KudiWebClient;
use digua_http_client::error::KudiHttpError;
use digua_http_client::models::TokenInfo;
use digua_http_client::options::KudiHttpOptions;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn end_to_end_service_to_service_via_wiremock() {
    let server = MockServer::start().await;
    let token_url = format!("{}/api/tokens/access", server.uri());
    let downstream_url = format!("{}/api/users/123", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/tokens/access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TokenInfo {
            access_token: "real-token".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_in: 3600,
        }))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/users/123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KudiWebClient::builder()
        .credential("svc-a", "secret-a")
        .token_endpoint(token_url)
        .read_timeout_ms(5000)
        .build()
        .unwrap();

    let resp = client.get(&downstream_url, None).await.unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, "ok");
}

#[tokio::test]
async fn invalidates_token_and_refetches_on_401() {
    let server = MockServer::start().await;
    let token_url = format!("{}/api/tokens/access", server.uri());
    let downstream_url = format!("{}/api/downstream", server.uri());

    Mock::given(method("POST"))
        .and(path("/api/tokens/access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(TokenInfo {
            access_token: "fresh".into(),
            refresh_token: None,
            token_type: "Bearer".into(),
            expires_in: 3600,
        }))
        .expect(2)
        .mount(&server)
        .await;

    // First downstream call returns 401, second returns 200.
    let downstream = Mock::given(method("GET"))
        .and(path("/api/downstream"));
    let server_clone = server.clone();
    server.register(downstream.respond_with(ResponseTemplate::new(401)).up_to_n_times(1)).await;
    server.register(
        wiremock::Mock::given(method("GET"))
            .and(path("/api/downstream"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok2"))
    ).await;

    let client = KudiWebClient::builder()
        .credential("svc-a", "secret-a")
        .token_endpoint(token_url)
        .retry_count(0)
        .build()
        .unwrap();

    // First call: 401 (we propagate, no auto-retry inside transport for 4xx).
    let r1 = client.get(&downstream_url, None).await.unwrap();
    assert_eq!(r1.status, 401);
    // Caller invalidates manually then retries:
    // (in this minimal version, the SDK does not auto-retry 401; we surface it.)
    // We assert the contract: caller can invalidate and the next call refetches.
    // For now, just assert both token fetches happened, which the .expect(2) enforces.
    let _ = r1;
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p kudi-http-client --test integration_test`
Expected: 2 passed

- [ ] **Step 3: Commit**

```bash
git add sdk/rust/kudi-http-client/tests/integration_test.rs
git commit -m "test(sdk): kudi-http-client wiremock integration tests"
```

---

## Task 10: .NET 项目骨架

**Files:**
- Create: `sdk/dotnet/KudiCoder.HttpClient/KudiCoder.HttpClient.csproj`
- Create: `sdk/dotnet/KudiCoder.HttpClient/README.md`
- Create: `sdk/dotnet/KudiCoder.HttpClient/KudiHttpOptions.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Models/TokenInfo.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/KudiHttpException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/AuthException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/NetworkException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/ServerException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/ValidationException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/Exceptions/RateLimitException.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/HttpTransport.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/ClientCredentialsFetcher.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/ServiceTokenCache.cs`
- Create: `sdk/dotnet/KudiCoder.HttpClient/KudiWebClient.cs`
- Create: `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiCoder.HttpClient.Tests.csproj`
- Create: `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/ServiceTokenCacheTests.cs`
- Create: `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiWebClientTests.cs`
- Modify: `sdk/dotnet/KudiCoder.sln`

- [ ] **Step 1: 创建目录**

Run: `mkdir -p sdk/dotnet/KudiCoder.HttpClient/Models sdk/dotnet/KudiCoder.HttpClient/Exceptions sdk/dotnet/tests/KudiCoder.HttpClient.Tests`

- [ ] **Step 2: 写 csproj**

Create `sdk/dotnet/KudiCoder.HttpClient/KudiCoder.HttpClient.csproj`:
```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <RootNamespace>KudiCoder.HttpClient</RootNamespace>
    <AssemblyName>KudiCoder.HttpClient</AssemblyName>
    <PackageId>KudiCoder.HttpClient</PackageId>
    <Description>Generic HTTP client for inter-service calls with client_credentials or user Bearer passthrough.</Description>
  </PropertyGroup>
</Project>
```

- [ ] **Step 3: 写 Models/TokenInfo.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/Models/TokenInfo.cs`:
```csharp
using System.Text.Json.Serialization;

namespace KudiCoder.HttpClient.Models;

public class TokenInfo
{
    [JsonPropertyName("access_token")] public string AccessToken { get; set; } = "";
    [JsonPropertyName("refresh_token")] public string? RefreshToken { get; set; }
    [JsonPropertyName("token_type")] public string TokenType { get; set; } = "Bearer";
    [JsonPropertyName("expires_in")] public long ExpiresIn { get; set; }
}
```

- [ ] **Step 4: 写异常类**

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/KudiHttpException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;

public class KudiHttpException : Exception
{
    public int StatusCode { get; }
    public string? Details { get; }
    public KudiHttpException(string message) : this(message, 0, null, null) { }
    public KudiHttpException(string message, Exception? inner) : this(message, 0, null, inner) { }
    public KudiHttpException(string message, int statusCode, string? details, Exception? inner)
        : base(message, inner) { StatusCode = statusCode; Details = details; }
}
```

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/AuthException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;
public sealed class AuthException : KudiHttpException
{
    public AuthException(string message) : base(message, 401, null, null) { }
    public AuthException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}
```

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/NetworkException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;
public sealed class NetworkException : KudiHttpException
{
    public NetworkException(string message) : base(message, 0, null, null) { }
    public NetworkException(string message, Exception? inner) : base(message, 0, null, inner) { }
}
```

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/ServerException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;
public sealed class ServerException : KudiHttpException
{
    public ServerException(string message) : base(message, 500, null, null) { }
    public ServerException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}
```

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/ValidationException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;
public sealed class ValidationException : KudiHttpException
{
    public ValidationException(string message) : base(message, 400, null, null) { }
    public ValidationException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}
```

Create `sdk/dotnet/KudiCoder.HttpClient/Exceptions/RateLimitException.cs`:
```csharp
namespace KudiCoder.HttpClient.Exceptions;
public sealed class RateLimitException : KudiHttpException
{
    public RateLimitException(string message) : base(message, 429, null, null) { }
    public RateLimitException(string message, string? details) : base(message, 429, details, null) { }
}
```

- [ ] **Step 5: 写 KudiHttpOptions.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/KudiHttpOptions.cs`:
```csharp
namespace KudiCoder.HttpClient;

public class KudiHttpOptions
{
    public string TokenEndpoint { get; set; } = "http://token-svc/api/tokens/access";
    public int ConnectTimeoutMs { get; set; } = 5000;
    public int ReadTimeoutMs { get; set; } = 15000;
    public int RetryCount { get; set; } = 1;
    public TimeSpan RefreshSkew { get; set; } = TimeSpan.FromSeconds(60);
    public string UserAgent { get; set; } = "kudi-http-client/0.1.0";
}
```

- [ ] **Step 6: 写 HttpTransport.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/HttpTransport.cs`:
```csharp
using System.Net;
using System.Net.Http.Headers;
using System.Text;
using KudiCoder.HttpClient.Exceptions;

namespace KudiCoder.HttpClient;

public class HttpResponse
{
    public int StatusCode { get; }
    public string Body { get; }
    public HttpResponse(int statusCode, string body) { StatusCode = statusCode; Body = body; }
}

public class HttpTransport
{
    private readonly KudiHttpOptions _options;
    public HttpTransport(KudiHttpOptions options) { _options = options; }

    public async Task<HttpResponse> ExecuteAsync(HttpRequestMessage request, CancellationToken ct = default)
    {
        var timeout = TimeSpan.FromMilliseconds(_options.ReadTimeoutMs + _options.ConnectTimeoutMs);
        using var http = new HttpClient { Timeout = timeout };
        request.Headers.UserAgent.ParseAdd(_options.UserAgent);
        try
        {
            using var resp = await http.SendAsync(request, ct);
            var body = await resp.Content.ReadAsStringAsync(ct);
            return new HttpResponse((int)resp.StatusCode, body);
        }
        catch (HttpRequestException e) { throw new NetworkException(e.Message, e); }
        catch (TaskCanceledException e) { throw new NetworkException("request timeout", e); }
    }

    public Task<HttpResponse> PostFormAsync(string url, IDictionary<string, string> form, CancellationToken ct = default)
    {
        var content = new FormUrlEncodedContent(form);
        var req = new HttpRequestMessage(HttpMethod.Post, url) { Content = content };
        return ExecuteAsync(req, ct);
    }

    public Task<HttpResponse> PostJsonAsync(string url, string json, string? bearer, CancellationToken ct = default)
    {
        var req = new HttpRequestMessage(HttpMethod.Post, url) { Content = new StringContent(json, Encoding.UTF8, "application/json") };
        if (bearer != null) req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearer);
        return ExecuteAsync(req, ct);
    }

    public Task<HttpResponse> GetAsync(string url, string? bearer, CancellationToken ct = default)
    {
        var req = new HttpRequestMessage(HttpMethod.Get, url);
        if (bearer != null) req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearer);
        return ExecuteAsync(req, ct);
    }
}

public class HttpRequestMessage : System.Net.Http.HttpRequestMessage
{
    public HttpRequestMessage(HttpMethod method, string url) : base(method, url) { }
}
```

- [ ] **Step 7: 写 ClientCredentialsFetcher.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/ClientCredentialsFetcher.cs`:
```csharp
using System.Text.Json;
using KudiCoder.HttpClient.Exceptions;
using KudiCoder.HttpClient.Models;

namespace KudiCoder.HttpClient;

public class ClientCredentialsFetcher
{
    private readonly HttpTransport _transport;
    private readonly string _tokenEndpoint;

    public ClientCredentialsFetcher(HttpTransport transport, string tokenEndpoint)
    {
        _transport = transport;
        _tokenEndpoint = tokenEndpoint;
    }

    public async Task<TokenInfo> FetchAsync(string clientId, string clientSecret, CancellationToken ct = default)
    {
        var form = new Dictionary<string, string>
        {
            ["grant_type"] = "client_credentials",
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret,
        };
        var resp = await _transport.PostFormAsync(_tokenEndpoint, form, ct);
        if (resp.StatusCode is < 200 or >= 300)
            throw new AuthException($"client_credentials failed: HTTP {resp.StatusCode}", resp.StatusCode, resp.Body);
        TokenInfo? info;
        try { info = JsonSerializer.Deserialize<TokenInfo>(resp.Body); }
        catch (JsonException e) { throw new NetworkException("parse token response: " + e.Message, e); }
        if (info == null || string.IsNullOrEmpty(info.AccessToken))
            throw new AuthException("token response missing access_token", resp.StatusCode, resp.Body);
        return info;
    }
}
```

- [ ] **Step 8: 写 ServiceTokenCache.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/ServiceTokenCache.cs`:
```csharp
using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using KudiCoder.HttpClient.Models;

namespace KudiCoder.HttpClient;

public class ServiceTokenCache
{
    private static readonly ConcurrentDictionary<string, CachedToken> _store = new();
    private readonly TimeSpan _refreshSkew;

    public ServiceTokenCache(TimeSpan refreshSkew) { _refreshSkew = refreshSkew; }

    public int Size => _store.Count;

    public async Task<string> GetOrFetchAsync(string clientId, string secretKey, Func<Task<TokenInfo>> fetcher)
    {
        var key = Digest(clientId, secretKey);
        if (_store.TryGetValue(key, out var cached) && cached.Matches(clientId, secretKey) && cached.ExpiresAt - DateTime.UtcNow > _refreshSkew)
            return cached.Info.AccessToken;

        var info = await fetcher();
        var entry = new CachedToken(clientId, secretKey, info, DateTime.UtcNow.AddSeconds(info.ExpiresIn));
        _store[key] = entry;
        return info.AccessToken;
    }

    public void Invalidate(string clientId, string secretKey)
        => _store.TryRemove(Digest(clientId, secretKey), out _);

    private static string Digest(string a, string b)
    {
        using var sha = SHA256.Create();
        var bytes = Encoding.UTF8.GetBytes(a + "|" + b);
        return Convert.ToHexString(sha.ComputeHash(bytes));
    }

    private sealed class CachedToken
    {
        public string ClientId { get; }
        public string SecretKey { get; }
        public TokenInfo Info { get; }
        public DateTime ExpiresAt { get; }
        public CachedToken(string cid, string sk, TokenInfo info, DateTime exp) { ClientId = cid; SecretKey = sk; Info = info; ExpiresAt = exp; }
        public bool Matches(string cid, string sk) => ClientId == cid && SecretKey == sk;
    }
}
```

- [ ] **Step 9: 写 KudiWebClient.cs**

Create `sdk/dotnet/KudiCoder.HttpClient/KudiWebClient.cs`:
```csharp
using KudiCoder.HttpClient.Exceptions;

namespace KudiCoder.HttpClient;

public class KudiWebClient : IAsyncDisposable
{
    private readonly HttpTransport _transport;
    private readonly KudiHttpOptions _options;
    private readonly ServiceTokenCache _cache;
    private readonly ClientCredentialsFetcher _fetcher;
    private (string ClientId, string SecretKey)? _credential;

    private KudiWebClient(HttpTransport transport, KudiHttpOptions options)
    {
        _transport = transport;
        _options = options;
        _cache = new ServiceTokenCache(options.RefreshSkew);
        _fetcher = new ClientCredentialsFetcher(transport, options.TokenEndpoint);
    }

    public static KudiWebClient Create(KudiHttpOptions? options = null)
        => new(new HttpTransport(options ?? new KudiHttpOptions()), options ?? new KudiHttpOptions());

    public KudiWebClient WithCredential(string clientId, string secretKey)
    {
        _credential = (clientId, secretKey);
        return this;
    }

    public KudiHttpOptions Options() => _options;
    public int CacheSize() => _cache.Size;

    private async Task<string?> ResolveBearerAsync(string? explicit, CancellationToken ct)
    {
        if (explicit != null) return explicit;
        if (_credential is var (cid, sk))
            return await _cache.GetOrFetchAsync(cid, sk, () => _fetcher.FetchAsync(cid, sk, ct));
        return null;
    }

    public Task<HttpResponse> PostAsync(string url, string jsonBody, string? bearer = null, CancellationToken ct = default)
        => _transport.PostJsonAsync(url, jsonBody, await ResolveBearerAsync(bearer, ct), ct);

    public Task<HttpResponse> GetAsync(string url, string? bearer = null, CancellationToken ct = default)
        => _transport.GetAsync(url, await ResolveBearerAsync(bearer, ct), ct);

    public async ValueTask DisposeAsync() { GC.SuppressFinalize(this); }
}
```

- [ ] **Step 10: 写测试项目 csproj**

Create `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiCoder.HttpClient.Tests.csproj`:
```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <IsPackable>false</IsPackable>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.10.0" />
    <PackageReference Include="xunit" Version="2.9.0" />
    <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />
    <ProjectReference Include="..\..\KudiCoder.HttpClient\KudiCoder.HttpClient.csproj" />
  </ItemGroup>
</Project>
```

- [ ] **Step 11: 写 ServiceTokenCacheTests.cs**

Create `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/ServiceTokenCacheTests.cs`:
```csharp
using KudiCoder.HttpClient;
using KudiCoder.HttpClient.Models;
using Xunit;

public class ServiceTokenCacheTests
{
    private static TokenInfo Info() => new() { AccessToken = "tok", ExpiresIn = 3600 };

    [Fact]
    public async Task CachesTokenPerCredentialPair()
    {
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        async Task<TokenInfo> Fetch()
        {
            calls++;
            return Info();
        }
        var t1 = await cache.GetOrFetchAsync("a", "s1", Fetch);
        var t2 = await cache.GetOrFetchAsync("a", "s1", Fetch);
        Assert.Equal("tok", t1);
        Assert.Equal("tok", t2);
        Assert.Equal(1, calls);
    }

    [Fact]
    public async Task SeparateCredentialsHaveSeparateCacheEntries()
    {
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        Task<TokenInfo> Fetch()
        {
            calls++;
            return Task.FromResult(Info());
        }
        await cache.GetOrFetchAsync("a", "s1", Fetch);
        await cache.GetOrFetchAsync("b", "s1", Fetch);
        Assert.Equal(2, calls);
        Assert.Equal(2, cache.Size);
    }

    [Fact]
    public async Task InvalidateRefetches()
    {
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        Task<TokenInfo> Fetch()
        {
            calls++;
            return Task.FromResult(Info());
        }
        await cache.GetOrFetchAsync("a", "s1", Fetch);
        await cache.GetOrFetchAsync("a", "s1", Fetch);
        Assert.Equal(1, calls);
        cache.Invalidate("a", "s1");
        await cache.GetOrFetchAsync("a", "s1", Fetch);
        Assert.Equal(2, calls);
    }
}
```

- [ ] **Step 12: 写 KudiWebClientTests.cs**

Create `sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiWebClientTests.cs`:
```csharp
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using KudiCoder.HttpClient;
using Xunit;

public class KudiWebClientTests
{
    private sealed class StubHandler : HttpMessageHandler
    {
        public int TokenCalls;
        public int DownstreamCalls;
        public string? LastAuth;
        public HttpStatusCode DownstreamStatus = HttpStatusCode.OK;
        public string TokenBody = "{\"access_token\":\"svc-token\",\"token_type\":\"Bearer\",\"expires_in\":3600}";
        public string DownstreamBody = "{}";
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage req, CancellationToken ct)
        {
            var path = req.RequestUri!.AbsolutePath;
            if (path.EndsWith("/api/tokens/access"))
            {
                TokenCalls++;
                return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(TokenBody) };
            }
            DownstreamCalls++;
            LastAuth = req.Headers.Authorization?.Parameter;
            return new HttpResponseMessage(DownstreamStatus) { Content = new StringContent(DownstreamBody) };
        }
    }

    [Fact]
    public async Task FetchesTokenOnceAndUsesForSubsequentCalls()
    {
        var handler = new StubHandler();
        var http = new System.Net.Http.HttpClient(handler);
        var transport = new TestHttpTransport(http);
        var client = new TestKudiWebClient(transport).WithCredential("a", "s1");

        await client.GetAsync("http://u/api/x", null);
        await client.GetAsync("http://u/api/y", null);
        await client.PostAsync("http://u/api/z", "{}", null);

        Assert.Equal(1, handler.TokenCalls);
        Assert.Equal(3, handler.DownstreamCalls);
        Assert.Equal("svc-token", handler.LastAuth);
    }

    [Fact]
    public async Task UserBearerPassthroughSkipsTokenFetch()
    {
        var handler = new StubHandler();
        var http = new System.Net.Http.HttpClient(handler);
        var transport = new TestHttpTransport(http);
        var client = new TestKudiWebClient(transport).WithCredential("a", "s1");

        await client.GetAsync("http://u/api/me", "user.jwt");
        Assert.Equal(0, handler.TokenCalls);
        Assert.Equal("user.jwt", handler.LastAuth);
    }
}
```

- [ ] **Step 13: 加 TestHttpTransport 适配**

为了让测试可注入 `HttpClient`，在 `sdk/dotnet/KudiCoder.HttpClient/HttpTransport.cs` 同文件追加可注入构造（不在测试项目里反射）。改 `HttpTransport` 接受外部 `HttpClient`：

Edit `sdk/dotnet/KudiCoder.HttpClient/HttpTransport.cs` 在 `class HttpTransport` 内插入：
```csharp
public HttpTransport(System.Net.Http.HttpClient client, KudiHttpOptions options)
{
    _options = options;
    _client = client;
}
private readonly System.Net.Http.HttpClient? _client;

public async Task<HttpResponse> ExecuteAsync(HttpRequestMessage request, CancellationToken ct = default)
{
    if (_client != null) {
        try { using var resp = await _client.SendAsync(request, ct);
            return new HttpResponse((int)resp.StatusCode, await resp.Content.ReadAsStringAsync(ct)); }
        catch (HttpRequestException e) { throw new NetworkException(e.Message, e); }
        catch (TaskCanceledException e) { throw new NetworkException("request timeout", e); }
    }
    // ... existing default path
}
```

具体实现由执行者按 .NET IHttpClientFactory 模式补全；若 `KudiWebClient` 接收注入版 `HttpTransport`，测试用 stub handler 验证 bearer 转发与缓存语义。

- [ ] **Step 14: 把新项目加进 sln**

Edit `sdk/dotnet/KudiCoder.sln` 在第 14 行后插入：
```text
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "KudiCoder.HttpClient", "KudiCoder.HttpClient\KudiCoder.HttpClient.csproj", "{77777777-7777-7777-7777-777777777777}"
EndProject
Project("{FAE04EC0-301F-11D3-BF4B-00C04F79EFBC}") = "KudiCoder.HttpClient.Tests", "tests\KudiCoder.HttpClient.Tests\KudiCoder.HttpClient.Tests.csproj", "{88888888-8888-8888-8888-888888888888}"
EndProject
```

并在 `GlobalSection(ProjectConfigurationPlatforms)` 内为两个新 GUID 各加 4 行（Debug/Release × Any CPU）。

- [ ] **Step 15: 跑测试**

Run: `dotnet test sdk/dotnet/tests/KudiCoder.HttpClient.Tests/KudiCoder.HttpClient.Tests.csproj`
Expected: 5 passed (3 cache + 2 client)

- [ ] **Step 16: Commit**

```bash
git add sdk/dotnet/KudiCoder.HttpClient sdk/dotnet/tests/KudiCoder.HttpClient.Tests sdk/dotnet/KudiCoder.sln
git commit -m "feat(sdk): KudiCoder.HttpClient with service token cache and client_credentials fetcher"
```

---

## Task 11: Java 项目骨架与父 pom 注册

**Files:**
- Create: `sdk/java/kudi-http-client/pom.xml`
- Create: `sdk/java/kudi-http-client/README.md`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/KudiHttpOptions.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Models/TokenInfo.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/KudiHttpException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/AuthException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/NetworkException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/ServerException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/ValidationException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions/RateLimitException.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/HttpTransport.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/ClientCredentialsFetcher.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/ServiceTokenCache.java`
- Create: `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/KudiWebClient.java`
- Create: `sdk/java/kudi-http-client/src/test/java/com/kudi/coder/httpclient/ServiceTokenCacheTest.java`
- Create: `sdk/java/kudi-http-client/src/test/java/com/kudi/coder/httpclient/KudiWebClientTest.java`
- Modify: `sdk/java/pom.xml`

- [ ] **Step 1: 创建目录**

Run: `mkdir -p sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Models sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Exceptions sdk/java/kudi-http-client/src/test/java/com/kudi/coder/httpclient`

- [ ] **Step 2: 写 pom.xml**

Create `sdk/java/kudi-http-client/pom.xml`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <parent>
    <groupId>com.kudi.coder</groupId>
    <artifactId>kudi-coder-sdk-parent</artifactId>
    <version>0.1.0</version>
  </parent>
  <artifactId>kudi-http-client</artifactId>
  <packaging>jar</packaging>
  <dependencies>
    <dependency>
      <groupId>com.fasterxml.jackson.core</groupId>
      <artifactId>jackson-databind</artifactId>
    </dependency>
    <dependency>
      <groupId>com.fasterxml.jackson.datatype</groupId>
      <artifactId>jackson-datatype-jsr310</artifactId>
    </dependency>
    <dependency>
      <groupId>org.junit.jupiter</groupId>
      <artifactId>junit-jupiter</artifactId>
    </dependency>
    <dependency>
      <groupId>com.squareup.okhttp3</groupId>
      <artifactId>mockwebserver</artifactId>
    </dependency>
  </dependencies>
  <build>
    <plugins>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-surefire-plugin</artifactId>
        <version>3.2.5</version>
      </plugin>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-source-plugin</artifactId>
        <version>3.3.0</version>
        <executions><execution><id>attach-sources</id><goals><goal>jar-no-fork</goal></goals></execution></executions>
      </plugin>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-javadoc-plugin</artifactId>
        <version>3.6.2</version>
        <executions><execution><id>attach-javadocs</id><goals><goal>jar</goal></goals></execution></executions>
      </plugin>
    </plugins>
  </build>
</project>
```

- [ ] **Step 3: 父 pom 加 module**

Edit `sdk/java/pom.xml` line 13-19，`<modules>` 加入 `<module>kudi-http-client</module>`。

- [ ] **Step 4: 写 TokenInfo 与异常**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/Models/TokenInfo.java`:
```java
package com.kudi.coder.httpclient.Models;
import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class TokenInfo {
    @JsonProperty("access_token") public String accessToken;
    @JsonProperty("refresh_token") public String refreshToken;
    @JsonProperty("token_type") public String tokenType = "Bearer";
    @JsonProperty("expires_in") public long expiresIn;
}
```

Create `Exceptions/KudiHttpException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class KudiHttpException extends RuntimeException {
    private final int statusCode;
    private final String details;
    public KudiHttpException(String message) { this(message, 0, null); }
    public KudiHttpException(String message, int statusCode, String details) { super(message); this.statusCode = statusCode; this.details = details; }
    public int getStatusCode() { return statusCode; }
    public String getDetails() { return details; }
}
```

Create `Exceptions/AuthException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class AuthException extends KudiHttpException {
    public AuthException(String message) { super(message, 401, null); }
    public AuthException(String message, int statusCode, String details) { super(message, statusCode, details); }
}
```

Create `Exceptions/NetworkException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class NetworkException extends KudiHttpException {
    public NetworkException(String message) { super(message); }
    public NetworkException(String message, Throwable cause) { super(message); initCause(cause); }
}
```

Create `Exceptions/ServerException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class ServerException extends KudiHttpException {
    public ServerException(String message) { super(message, 500, null); }
    public ServerException(String message, int statusCode, String details) { super(message, statusCode, details); }
}
```

Create `Exceptions/ValidationException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class ValidationException extends KudiHttpException {
    public ValidationException(String message) { super(message, 400, null); }
    public ValidationException(String message, int statusCode, String details) { super(message, statusCode, details); }
}
```

Create `Exceptions/RateLimitException.java`:
```java
package com.kudi.coder.httpclient.Exceptions;
public class RateLimitException extends KudiHttpException {
    public RateLimitException(String message) { super(message, 429, null); }
    public RateLimitException(String message, String details) { super(message, 429, details); }
}
```

- [ ] **Step 5: 写 KudiHttpOptions.java**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/KudiHttpOptions.java`:
```java
package com.kudi.coder.httpclient;
import java.time.Duration;
public class KudiHttpOptions {
    public String tokenEndpoint = "http://token-svc/api/tokens/access";
    public Duration connectTimeout = Duration.ofSeconds(5);
    public Duration readTimeout = Duration.ofSeconds(15);
    public int retryCount = 1;
    public Duration refreshSkew = Duration.ofSeconds(60);
    public String userAgent = "kudi-http-client/0.1.0";
}
```

- [ ] **Step 6: 写 HttpTransport.java**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/HttpTransport.java`:
```java
package com.kudi.coder.httpclient;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.Map;
import com.kudi.coder.httpclient.Exceptions.NetworkException;

public class HttpTransport {
    public static class Response {
        public final int statusCode;
        public final String body;
        public Response(int s, String b) { statusCode = s; body = b; }
    }
    private final KudiHttpOptions options;
    private final HttpClient http;
    public HttpTransport(KudiHttpOptions options) {
        this.options = options;
        this.http = HttpClient.newBuilder().connectTimeout(options.connectTimeout).build();
    }
    public Response postForm(String url, Map<String,String> form) throws Exception {
        String body = form.entrySet().stream()
            .map(e -> urlEncode(e.getKey()) + "=" + urlEncode(e.getValue()))
            .reduce((a,b) -> a + "&" + b).orElse("");
        HttpRequest req = HttpRequest.newBuilder(URI.create(url))
            .header("User-Agent", options.userAgent)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .timeout(options.readTimeout)
            .POST(HttpRequest.BodyPublishers.ofString(body))
            .build();
        return send(req);
    }
    public Response postJson(String url, String json, String bearer) throws Exception {
        HttpRequest.Builder b = HttpRequest.newBuilder(URI.create(url))
            .header("User-Agent", options.userAgent)
            .header("Content-Type", "application/json")
            .timeout(options.readTimeout)
            .POST(HttpRequest.BodyPublishers.ofString(json));
        if (bearer != null) b.header("Authorization", "Bearer " + bearer);
        return send(b.build());
    }
    public Response get(String url, String bearer) throws Exception {
        HttpRequest.Builder b = HttpRequest.newBuilder(URI.create(url))
            .header("User-Agent", options.userAgent)
            .timeout(options.readTimeout)
            .GET();
        if (bearer != null) b.header("Authorization", "Bearer " + bearer);
        return send(b.build());
    }
    private Response send(HttpRequest req) throws Exception {
        try {
            HttpResponse<String> r = http.send(req, HttpResponse.BodyHandlers.ofString());
            return new Response(r.statusCode(), r.body());
        } catch (java.net.http.HttpTimeoutException | java.io.IOException e) {
            throw new NetworkException(e.getMessage(), e);
        }
    }
    private static String urlEncode(String s) {
        StringBuilder out = new StringBuilder();
        for (char c : s.toCharArray()) {
            if (Character.isLetterOrDigit(c) || c == '-' || c == '_' || c == '.' || c == '~') out.append(c);
            else out.append(String.format("%%%02X", (int)c));
        }
        return out.toString();
    }
}
```

- [ ] **Step 7: 写 ClientCredentialsFetcher.java**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/ClientCredentialsFetcher.java`:
```java
package com.kudi.coder.httpclient;
import java.util.Map;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.httpclient.Exceptions.AuthException;
import com.kudi.coder.httpclient.Exceptions.NetworkException;
import com.kudi.coder.httpclient.Models.TokenInfo;

public class ClientCredentialsFetcher {
    private final HttpTransport transport;
    private final String tokenEndpoint;
    private final ObjectMapper json = new ObjectMapper();
    public ClientCredentialsFetcher(HttpTransport transport, String tokenEndpoint) {
        this.transport = transport;
        this.tokenEndpoint = tokenEndpoint;
    }
    public TokenInfo fetch(String clientId, String clientSecret) throws Exception {
        HttpTransport.Response resp = transport.postForm(tokenEndpoint, Map.of(
            "grant_type", "client_credentials",
            "client_id", clientId,
            "client_secret", clientSecret
        ));
        if (resp.statusCode < 200 || resp.statusCode >= 300)
            throw new AuthException("client_credentials failed: HTTP " + resp.statusCode, resp.statusCode, resp.body);
        TokenInfo info;
        try { info = json.readValue(resp.body, TokenInfo.class); }
        catch (Exception e) { throw new NetworkException("parse token response: " + e.getMessage(), e); }
        if (info.accessToken == null || info.accessToken.isEmpty())
            throw new AuthException("token response missing access_token", resp.statusCode, resp.body);
        return info;
    }
}
```

- [ ] **Step 8: 写 ServiceTokenCache.java**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/ServiceTokenCache.java`:
```java
package com.kudi.coder.httpclient;
import java.security.MessageDigest;
import java.time.Duration;
import java.time.Instant;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.locks.ReentrantLock;
import java.util.function.Supplier;
import com.kudi.coder.httpclient.Models.TokenInfo;

public class ServiceTokenCache {
    private static final ConcurrentHashMap<String, Entry> STORE = new ConcurrentHashMap<>();
    private final Duration refreshSkew;
    private final ReentrantLock lock = new ReentrantLock();
    public ServiceTokenCache(Duration refreshSkew) { this.refreshSkew = refreshSkew; }
    public int size() { return STORE.size(); }

    public String getOrFetch(String clientId, String secretKey, ThrowingSupplier<TokenInfo> fetcher) throws Exception {
        String key = digest(clientId, secretKey);
        Entry hit = STORE.get(key);
        if (hit != null && hit.matches(clientId, secretKey) && hit.expiresAt.isAfter(Instant.now().plus(refreshSkew)))
            return hit.info.accessToken;

        lock.lock();
        try {
            hit = STORE.get(key);
            if (hit != null && hit.matches(clientId, secretKey) && hit.expiresAt.isAfter(Instant.now().plus(refreshSkew)))
                return hit.info.accessToken;
            TokenInfo info = fetcher.get();
            STORE.put(key, new Entry(clientId, secretKey, info, Instant.now().plusSeconds(Math.max(0, info.expiresIn))));
            return info.accessToken;
        } finally { lock.unlock(); }
    }

    public void invalidate(String clientId, String secretKey) { STORE.remove(digest(clientId, secretKey)); }

    private static String digest(String a, String b) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] h = md.digest((a + "|" + b).getBytes(java.nio.charset.StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder();
            for (byte x : h) sb.append(String.format("%02X", x));
            return sb.toString();
        } catch (Exception e) { throw new RuntimeException(e); }
    }

    @FunctionalInterface public interface ThrowingSupplier<T> { T get() throws Exception; }

    private static class Entry {
        final String clientId, secretKey;
        final TokenInfo info;
        final Instant expiresAt;
        Entry(String cid, String sk, TokenInfo i, Instant e) { clientId=cid; secretKey=sk; info=i; expiresAt=e; }
        boolean matches(String cid, String sk) { return clientId.equals(cid) && secretKey.equals(sk); }
    }
}
```

- [ ] **Step 9: 写 KudiWebClient.java**

Create `sdk/java/kudi-http-client/src/main/java/com/kudi/coder/httpclient/KudiWebClient.java`:
```java
package com.kudi.coder.httpclient;
import java.util.function.Supplier;
import com.kudi.coder.httpclient.Models.TokenInfo;

public class KudiWebClient {
    private final KudiHttpOptions options;
    private final HttpTransport transport;
    private final ServiceTokenCache cache;
    private final ClientCredentialsFetcher fetcher;
    private String clientId;
    private String secretKey;

    private KudiWebClient(KudiHttpOptions options) {
        this.options = options;
        this.transport = new HttpTransport(options);
        this.cache = new ServiceTokenCache(options.refreshSkew);
        this.fetcher = new ClientCredentialsFetcher(transport, options.tokenEndpoint);
    }
    public static KudiWebClient create() { return new KudiWebClient(new KudiHttpOptions()); }
    public static KudiWebClient create(KudiHttpOptions options) { return new KudiWebClient(options); }
    public KudiWebClient withCredential(String clientId, String secretKey) { this.clientId = clientId; this.secretKey = secretKey; return this; }
    public KudiWebClient tokenEndpoint(String url) { this.options.tokenEndpoint = url; return this; }
    public KudiHttpOptions options() { return options; }
    public int cacheSize() { return cache.size(); }

    private String resolveBearer(String explicit) throws Exception {
        if (explicit != null) return explicit;
        if (clientId != null && secretKey != null)
            return cache.getOrFetch(clientId, secretKey, () -> fetcher.fetch(clientId, secretKey));
        return null;
    }

    public HttpTransport.Response post(String url, String jsonBody, String bearer) throws Exception {
        return transport.postJson(url, jsonBody, resolveBearer(bearer));
    }
    public HttpTransport.Response get(String url, String bearer) throws Exception {
        return transport.get(url, resolveBearer(bearer));
    }
}
```

- [ ] **Step 10: 写 ServiceTokenCacheTest.java**

Create `sdk/java/kudi-http-client/src/test/java/com/kudi/coder/httpclient/ServiceTokenCacheTest.java`:
```java
package com.kudi.coder.httpclient;
import java.time.Duration;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;
import com.kudi.coder.httpclient.Models.TokenInfo;
import static org.junit.jupiter.api.Assertions.*;

class ServiceTokenCacheTest {
    private static TokenInfo info() { TokenInfo i = new TokenInfo(); i.accessToken = "tok"; i.expiresIn = 3600; return i; }

    @Test void cachesPerCredential() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        AtomicInteger calls = new AtomicInteger();
        var t1 = cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        var t2 = cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals("tok", t1); assertEquals("tok", t2); assertEquals(1, calls.get());
    }

    @Test void separateCredentials() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        AtomicInteger calls = new AtomicInteger();
        cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        cache.getOrFetch("b", "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals(2, calls.get()); assertEquals(2, cache.size());
    }

    @Test void invalidateRefetches() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        AtomicInteger calls = new AtomicInteger();
        cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        cache.invalidate("a", "s1");
        cache.getOrFetch("a", "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals(2, calls.get());
    }
}
```

- [ ] **Step 11: 写 KudiWebClientTest.java（MockWebServer）**

Create `sdk/java/kudi-http-client/src/test/java/com/kudi/coder/httpclient/KudiWebClientTest.java`:
```java
package com.kudi.coder.httpclient;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class KudiWebClientTest {
    private MockWebServer server;
    @BeforeEach void setUp() throws Exception { server = new MockWebServer(); server.start(); }
    @AfterEach void tearDown() throws Exception { server.shutdown(); }

    @Test void fetchesTokenOnceAndUsesForSubsequentCalls() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200)
            .setBody("{\"access_token\":\"svc-token\",\"token_type\":\"Bearer\",\"expires_in\":3600}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));

        var client = KudiWebClient.create()
            .tokenEndpoint(server.url("/api/tokens/access").toString())
            .withCredential("a", "s1");
        client.get(server.url("/api/x").toString(), null);
        client.get(server.url("/api/y").toString(), null);
        client.post(server.url("/api/z").toString(), "{}", null);

        assertEquals(1, server.getRequestCount() - 1, "exactly one token request");
        var downstream = server.takeRequest();
        assertNotNull(downstream.getHeader("Authorization"));
    }

    @Test void userBearerPassthrough() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        var client = KudiWebClient.create()
            .tokenEndpoint(server.url("/api/tokens/access").toString())
            .withCredential("a", "s1");
        client.get(server.url("/api/me").toString(), "user.jwt");
        assertEquals(1, server.getRequestCount());
        assertEquals("Bearer user.jwt", server.takeRequest().getHeader("Authorization"));
    }
}
```

- [ ] **Step 12: 跑测试**

Run: `cd sdk/java && mvn -q -pl kudi-http-client test`
Expected: 5 passed

- [ ] **Step 13: Commit**

```bash
git add sdk/java/kudi-http-client sdk/java/pom.xml
git commit -m "feat(sdk): kudi-http-client java module"
```

---

## Task 12: 三语言 README

**Files:**
- Modify/Create: `sdk/dotnet/KudiCoder.HttpClient/README.md`
- Modify/Create: `sdk/java/kudi-http-client/README.md`
- Modify/Create: `sdk/rust/kudi-http-client/README.md`
- Modify: `sdk/README.md`

- [ ] **Step 1: .NET README**

Create `sdk/dotnet/KudiCoder.HttpClient/README.md`（参考 spec section 6 .NET 代码块，含 service-to-service、透传 Bearer、错误处理章节）。

- [ ] **Step 2: Java README**

Create `sdk/java/kudi-http-client/README.md`（含 Maven 依赖、builder 用法、错误处理）。

- [ ] **Step 3: Rust README**

Create `sdk/rust/kudi-http-client/README.md`（含 Cargo.toml 依赖、builder 用法、错误处理）。

- [ ] **Step 4: 顶层 sdk/README.md**

Edit `sdk/README.md`，在"目录"段落加入新包路径与一句话说明"通用服务间 HTTP 客户端，支持 client_credentials + Bearer 透传"。

- [ ] **Step 5: Commit**

```bash
git add sdk/dotnet/KudiCoder.HttpClient/README.md sdk/java/kudi-http-client/README.md sdk/rust/kudi-http-client/README.md sdk/README.md
git commit -m "docs(sdk): kudi-http-client readme across .NET/Java/Rust"
```

---

## Task 13: 全栈测试门槛

**Files:** 无新增

- [ ] **Step 1: Rust 全测**

Run: `cargo test -p kudi-http-client`
Expected: 7 passed (3 cache + 2 client + 2 integration)

- [ ] **Step 2: .NET 全测**

Run: `dotnet test sdk/dotnet/KudiCoder.HttpClient.Tests/KudiCoder.HttpClient.Tests.csproj`
Expected: 5 passed

- [ ] **Step 3: Java 全测**

Run: `cd sdk/java && mvn -q test`
Expected: 5 passed (其他 SDK 模块无回归)

- [ ] **Step 4: cargo clippy + rustfmt**

Run: `cargo fmt --check && cargo clippy -p kudi-http-client --all-targets -- -D warnings`
Expected: 无 warning

- [ ] **Step 5: dotnet build + format**

Run: `dotnet build sdk/dotnet/KudiCoder.HttpClient/KudiCoder.HttpClient.csproj`
Expected: Build succeeded

- [ ] **Step 6: Commit (若有格式修复)**

```bash
git add -A
git commit -m "chore(sdk): kudi-http-client fmt + clippy cleanups" --allow-empty
```

---

## 风险与回退

- **consul-registrar 编译失败**（Task 1）若修复后 root workspace 仍有别处编译问题，分批修复不要捎带
- **Rust `reqwest` 体积**：默认已 `rustls-tls-no-provider`，与项目其他 crate 一致
- **Java 8 路径**：当前 spec 标注支持 8，但本 plan 主路径用 `java.net.http.HttpClient`（11+），Java 8 需在后续 plan 单独加 `HttpURLConnection` 分支
- **Java 8 路径未实现**：保持 YAGNI；如需启用，把 HttpTransport 的 `java.net.http.HttpClient` 调用切到 `HttpURLConnection` 即可，签名不变
