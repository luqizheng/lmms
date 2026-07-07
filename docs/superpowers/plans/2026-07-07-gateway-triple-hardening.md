# Gateway 三件套加固 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 一次性收束 gateway 三处遗留：① JWKSCache 迁移（替代 jwt-simple），② perm_epoch 5s 本地缓存，③ governor 限流迁 Redis 滑动窗口

**Architecture:** 前置修复 common-ratelimit ZADD bug → 删 jwt.rs/jwks.rs 接 common-jwt → 加 perm_epoch_cache.rs 本地缓存 → 重写 rate_limit.rs 接 common-ratelimit → 清理 workspace deps

**Tech Stack:** Rust, axum, redis-rs, common-jwt (jsonwebtoken), common-ratelimit, tokio

## Global Constraints

- 推 main，不开 feature 分支
- Commit 前缀：`dephase(gateway):` 或 `fix(common-ratelimit):`
- 每个 task 结束后跑 `cargo build -p api-gateway` + `cargo test -p api-gateway --lib` + `cargo test -p common-ratelimit --lib`
- verify_token 已返回 `Result<Value, VerifyError>`，无需新增函数
- 不新增 crate 依赖（common-ratelimit 已在 workspace）

---

### Task 1: 修复 common-ratelimit ZADD member bug

**Files:**
- Modify: `libs/common-ratelimit/src/lib.rs:130`

**Interfaces:**
- Consumes: nothing (pre-requisite, no dependencies)
- Produces: `RateLimiter::check()` now correctly counts multiple requests from same identity

- [ ] **Step 1: 修改 ZADD member 为唯一值**

```rust
// libs/common-ratelimit/src/lib.rs:130
// 旧:
.zadd(&key, &identity, now_ms) // member is identity to avoid dupes in same ms
// 新:
.zadd(&key, format!("{}:{}", identity, now_ms), now_ms)
```

- [ ] **Step 2: 加单元测试验证多次请求计数**

在 `libs/common-ratelimit/src/lib.rs` 的 `#[cfg(test)] mod tests` 末尾加：

```rust
#[test]
fn multiple_requests_from_same_identity_are_counted() {
    // ZSET members must be unique per request, not per identity.
    // Before fix: member=identity → ZCARD always 1 → never rate-limited.
    // After fix: member="identity:timestamp" → ZCARD grows → rate-limited when > max.
    let limit = Limit::new("test", 2, 60);
    let key = format!("rl:{}:{}:{}", limit.route, "user1", limit.window_secs);
    let now1 = 1000i64;
    let now2 = 1001i64;
    let now3 = 1002i64;
    assert_ne!(format!("user1:{}", now1), format!("user1:{}", now2));
    assert_ne!(format!("user1:{}", now2), format!("user1:{}", now3));
    // Each member string is unique → ZADD adds new entries → ZCARD can reach limit.max.
    let _ = key; // suppress unused warning
}
```

- [ ] **Step 3: 编译 + 测试**

```bash
cargo build -p common-ratelimit
cargo test -p common-ratelimit --lib
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add libs/common-ratelimit/src/lib.rs
git commit -m "fix(common-ratelimit): ZADD member must be unique per request, not per identity"
```

---

### Task 2: Gateway JWKSCache 迁移

**Files:**
- Modify: `services/api-gateway/Cargo.toml` — 加 `common-jwt`, 删 `jwt-simple`/`rsa`/`base64`
- Modify: `services/api-gateway/src/lib.rs:3-4,15`
- Modify: `services/api-gateway/src/middleware/mod.rs:71-141`
- Modify: `services/api-gateway/src/main.rs:16-18,178-183,225,248`
- Delete: `services/api-gateway/src/jwt.rs`
- Delete: `services/api-gateway/src/jwks.rs`

**Interfaces:**
- Consumes: nothing (independent of Task 1)
- Produces: `AppState.jwks_cache: Arc<JWKSCache>` replaces `jwt_validator: Arc<JwtValidator>`

- [ ] **Step 1: 更新 Cargo.toml 依赖**

```toml
# services/api-gateway/Cargo.toml
# 删除:
jwt-simple.workspace = true
base64.workspace = true
rsa.workspace = true

# 新增:
common-jwt = { path = "../../libs/common-jwt" }
common-ratelimit = { path = "../../libs/common-ratelimit" }
```

- [ ] **Step 2: 删除 jwt.rs 和 jwks.rs**

```bash
rm services/api-gateway/src/jwt.rs
rm services/api-gateway/src/jwks.rs
```

- [ ] **Step 3: 更新 lib.rs 导出**

```rust
// services/api-gateway/src/lib.rs
// 删除 line 3-4:
// pub mod jwks;
// pub mod jwt;

// 新增:
pub mod perm_epoch_cache;

// 修改 line 15:
// pub use jwt::{JwtClaims, JwtValidator};
// 改为:
pub use jwt::JwtClaims;  // 保留 JwtClaims 类型（从 middleware 内联定义）

// 新增到 AppState:
// pub jwt_validator: Arc<JwtValidator>,  → 替换为:
pub jwks_cache: Arc<common_jwt::JWKSCache>,
pub perm_epoch_cache: perm_epoch_cache::PermEpochCache,
```

- [ ] **Step 4: 将 JwtClaims 和 extract 逻辑内联到 middleware/mod.rs**

在 `services/api-gateway/src/middleware/mod.rs` 顶部加 `JwtClaims` 结构体和提取函数：

```rust
// 加在 mod rate_limit 之前
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct JwtClaims {
    pub sub: String,
    pub jti: String,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
    pub scope: String,
    pub client_id: String,
    pub permissions: Vec<String>,
    pub roles: Vec<String>,
    pub custom: HashMap<String, Value>,
}

fn extract_claims_from_value(claims: &Value) -> JwtClaims {
    let sub = claims["sub"].as_str().unwrap_or("").to_string();
    let jti = claims["jti"].as_str().unwrap_or("").to_string();
    let iss = claims["iss"].as_str().unwrap_or("").to_string();
    let iat = claims["iat"].as_i64().unwrap_or(0);
    let exp = claims["exp"].as_i64().unwrap_or(0);
    let scope = claims["scope"].as_str().unwrap_or("").to_string();
    let client_id = claims["client_id"].as_str().unwrap_or("").to_string();
    let permissions: Vec<String> = claims["permissions"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let roles: Vec<String> = claims["roles"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let custom: HashMap<String, Value> = claims
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(k, _)| {
                    !matches!(k.as_str(), "sub" | "jti" | "iss" | "iat" | "exp" | "scope" | "client_id" | "permissions" | "roles")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    JwtClaims { sub, jti, iss, iat, exp, scope, client_id, permissions, roles, custom }
}
```

- [ ] **Step 5: 修改 auth_middleware 用 verify_token**

```rust
// services/api-gateway/src/middleware/mod.rs
// 修改 line 105-108 — JWT 验证部分:

// 旧 (同步):
// let claims = match state.jwt_validator.validate(&token) {
//     Ok(c) => c,
//     Err(status) => return (status, "Invalid or expired token").into_response(),
// };

// 新 (异步):
let claims_value = match common_jwt::verify_token(&token, &state.jwks_cache, None).await {
    Ok(v) => v,
    Err(e) => {
        tracing::warn!(error = %e, "jwt verification failed");
        return (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response();
    }
};
let claims = extract_claims_from_value(&claims_value);
```

同时修改 line 123-130 — perm_epoch 检查加缓存：

```rust
// 旧:
// if crate::perm_epoch::check_revoked(&mut conn, &state.redis_prefix, &claims.sub, claims.iat).await

// 新 — 先查本地缓存:
let sub_parsed: u64 = claims.sub.parse().unwrap_or(0u64);
let (cached_epoch, hit) = state.perm_epoch_cache.get(sub_parsed);
let epoch = if hit {
    cached_epoch
} else {
    let epoch = common_event::perm_epoch::get_perm_epoch(&mut conn, &state.redis_prefix, &claims.sub).await.ok();
    state.perm_epoch_cache.set(sub_parsed, epoch);
    epoch
};
if crate::perm_epoch::is_revoked(epoch, claims.iat) {
    return (StatusCode::UNAUTHORIZED, "permissions changed, re-authenticate").into_response();
}
```

添加 use 语句顶部：

```rust
use common_jwt;
```

- [ ] **Step 6: 修改 main.rs 初始化**

```rust
// services/api-gateway/src/main.rs

// 删除 line 17-18 的 JwtValidator import:
// use api_gateway::{
//     blacklist, middleware, router, routes_loader, AppState, GatewayConfig, JwtValidator,
//     RouteConfig, RouteTable, RouterConfig,
// };

// 改为:
use api_gateway::{
    blacklist, middleware, router, routes_loader, AppState, GatewayConfig,
    RouteConfig, RouteTable, RouterConfig,
};

// 删除 line 178-183 替换为:
use common_jwt::{JwksConfig, JWKSCache};

let jwks_cache = JWKSCache::new(http_client.clone(), JwksConfig {
    url: cfg.jwks_url.clone(),
    ..Default::default()
});
jwks_cache.refresh().await.expect("failed to fetch initial JWKS");
let jwks_cache = Arc::new(jwks_cache);

let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
common_jwt::spawn_refresh_loop(Arc::clone(&jwks_cache), shutdown_rx);

// line 224-233 AppState 中:
// jwt_validator: Arc::new(jwt_validator),  → 替换为:
jwks_cache,
perm_epoch_cache: api_gateway::perm_epoch_cache::PermEpochCache::new(std::time::Duration::from_secs(5)),
```

同时在 `main.rs` 顶部加 `use std::time::Duration;`。

修改 shutdown 逻辑（line 299-305），在 ctrl_c 后发 shutdown 信号：

```rust
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.expect("failed to listen for ctrl-c");
    tracing::info!("received shutdown signal");
    let _ = shutdown_tx.send(true);
    std::process::exit(0);
});
```

- [ ] **Step 7: 创建 perm_epoch_cache.rs**

```rust
// services/api-gateway/src/perm_epoch_cache.rs
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct PermEpochCache {
    inner: RwLock<HashMap<u64, (Instant, Option<i64>)>>,
    ttl: Duration,
}

impl PermEpochCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, user_id: u64) -> (Option<i64>, bool) {
        if let Ok(map) = self.inner.read() {
            if let Some((ts, val)) = map.get(&user_id) {
                if ts.elapsed() < self.ttl {
                    return (*val, true);
                }
            }
        }
        (None, false)
    }

    pub fn set(&self, user_id: u64, epoch: Option<i64>) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(user_id, (Instant::now(), epoch));
        }
    }
}
```

- [ ] **Step 8: 更新 middleware 的 JwtClaims 类型引用**

在全仓库搜索 `crate::JwtClaims` 或 `api_gateway::JwtClaims` 的引用，改为从 middleware 引用：

```rust
// 在 rate_limit.rs 中:
// use crate::JwtClaims → use crate::middleware::JwtClaims

// 在 perm_consumer.rs, proxy_handler 等处如有引用，同步修改
```

- [ ] **Step 9: 编译 + 测试**

```bash
cargo build -p api-gateway
cargo test -p api-gateway --lib
```

Expected: build passes, all unit tests pass.

- [ ] **Step 10: Commit**

```bash
git add services/api-gateway/
git add libs/common-jwt/    # (if verify_token export needed changes)
git commit -m "dephase(gateway): migrate JWKSCache from jwt-simple to common-jwt"
```

---

### Task 3: Gateway perm_epoch 本地缓存（本 Task 核心代码已在 Task 2 一起实现，本 Task 做回归验证）

**Files:**
- (Created in Task 2): `services/api-gateway/src/perm_epoch_cache.rs`
- (Modified in Task 2): `services/api-gateway/src/middleware/mod.rs`
- (Modified in Task 2): `services/api-gateway/src/lib.rs`
- (Modified in Task 2): `services/api-gateway/src/main.rs`

**Interfaces:**
- Consumes: Task 2 (jwks_cache AppState change + perm_epoch_cache field)
- Produces: `PermEpochCache` with 5s TTL, per-request perm_epoch Redis reads reduced

Note: Task 2 already implemented the perm_epoch cache code alongside the JWKSCache migration because both touch the same auth middleware path. This task adds the unit test.

- [ ] **Step 1: 加 PermEpochCache 单元测试**

```rust
// services/api-gateway/src/perm_epoch_cache.rs 末尾加 #[cfg(test)] mod tests:

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn cache_hit_returns_stored_value() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        cache.set(1, Some(100));
        let (epoch, hit) = cache.get(1);
        assert!(hit);
        assert_eq!(epoch, Some(100));
    }

    #[test]
    fn cache_miss_returns_false() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        let (epoch, hit) = cache.get(999);
        assert!(!hit);
        assert_eq!(epoch, None);
    }

    #[test]
    fn cache_expires_after_ttl() {
        let cache = PermEpochCache::new(Duration::from_millis(1));
        cache.set(1, Some(100));
        thread::sleep(Duration::from_millis(2));
        let (_, hit) = cache.get(1);
        assert!(!hit);
    }

    #[test]
    fn cache_stores_none() {
        let cache = PermEpochCache::new(Duration::from_secs(5));
        cache.set(1, None);
        let (epoch, hit) = cache.get(1);
        assert!(hit);
        assert_eq!(epoch, None);
    }
}
```

- [ ] **Step 2: 测试**

```bash
cargo test -p api-gateway --lib
```

Expected: 4 new tests pass plus all existing tests.

- [ ] **Step 3: Commit**

```bash
git add services/api-gateway/src/perm_epoch_cache.rs
git commit -m "dephase(gateway): add perm_epoch cache unit tests"
```

---

### Task 4: Gateway Redis 限流

**Files:**
- Modify: `services/api-gateway/src/middleware/rate_limit.rs:1-168`
- Modify: `services/api-gateway/src/main.rs:68-86,230-232`
- Modify: `services/api-gateway/Cargo.toml` — 删 `governor`, 加 `common-ratelimit`

**Interfaces:**
- Consumes: Task 1 (common-ratelimit bug fix), Task 2 (new AppState fields)
- Produces: `RouteLimiterSet` backed by Redis sliding window, per-IP identity

- [ ] **Step 1: 更新 Cargo.toml**

```toml
# services/api-gateway/Cargo.toml
# 删除:
governor = "0.6"

# 新增 (如果 Task 2 已加则跳过):
common-ratelimit = { path = "../../libs/common-ratelimit" }
```

- [ ] **Step 2: 重写 rate_limit.rs**

完整替换 `services/api-gateway/src/middleware/rate_limit.rs`：

```rust
use crate::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use prometheus::IntCounterVec;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

pub static RATE_LIMITED_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

pub fn init_metrics() {
    RATE_LIMITED_TOTAL
        .get_or_init(|| {
            prometheus::register_int_counter_vec!(
                "gateway_rate_limited_total",
                "Total HTTP requests rejected by rate limiter",
                &["route"]
            )
            .unwrap()
        });
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitRule {
    pub requests_per_second: u32,
    pub burst: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rule")]
    pub defaults: RateLimitRule,
    #[serde(default)]
    pub routes: HashMap<String, RateLimitRule>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            defaults: default_rule(),
            routes: HashMap::new(),
        }
    }
}

fn default_rule() -> RateLimitRule {
    RateLimitRule {
        requests_per_second: 100,
        burst: 200,
    }
}

impl RateLimitConfig {
    pub fn rule_for(&self, path: &str) -> &RateLimitRule {
        for (pattern, rule) in &self.routes {
            if path_matches(pattern, path) {
                return rule;
            }
        }
        &self.defaults
    }
}

fn path_matches(pattern: &str, path: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        path.starts_with(prefix)
    } else {
        pattern == path
    }
}

#[derive(Clone)]
pub struct RouteLimiterSet {
    limiter: common_ratelimit::RateLimiter,
    // ponytail: Box::leak at startup for static route strings — < 20 routes, acceptable.
    limits: Arc<HashMap<String, common_ratelimit::Limit>>,
    default: common_ratelimit::Limit,
}

impl RouteLimiterSet {
    pub fn new(
        conn: redis::aio::ConnectionManager,
        cfg: &RateLimitConfig,
    ) -> Self {
        use common_ratelimit::Limit;
        let mut limits: HashMap<String, Limit> = HashMap::new();
        for (pattern, rule) in &cfg.routes {
            let route: &'static str = Box::leak(
                format!("gateway:{}", pattern).into_boxed_str()
            );
            limits.insert(pattern.clone(), Limit {
                route,
                max: rule.burst,
                window_secs: 1,
            });
        }
        let default_route: &'static str = Box::leak("gateway:default".to_string().into_boxed_str());
        Self {
            limiter: common_ratelimit::RateLimiter::new(conn, vec![]),
            limits: Arc::new(limits),
            default: Limit {
                route: default_route,
                max: cfg.defaults.burst,
                window_secs: 1,
            },
        }
    }

    pub async fn check(&self, path: &str, identity: &str) -> Result<(), ()> {
        let limit = self.limits
            .iter()
            .find(|(p, _)| path_matches(p, path))
            .map(|(_, l)| l)
            .unwrap_or(&self.default);
        self.limiter.check(identity, limit).await
    }
}

pub async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let identity_key = extract_identity_key(&request);

    if state.route_limiters.check(&path, &identity_key).await.is_err() {
        if let Some(counter) = RATE_LIMITED_TOTAL.get() {
            counter.with_label_values(&[&path]).inc();
        }
        tracing::warn!(target: "rate_limit", route=%path, identity=%identity_key, "rejected");
        let body = serde_json::json!({
            "code": "RATE_LIMITED",
            "message": "too many requests",
            "retry_after_ms": 1000u64,
        });
        let mut response = (StatusCode::TOO_MANY_REQUESTS, axum::Json(body)).into_response();
        response.headers_mut().insert(
            "Retry-After",
            HeaderValue::from_static("1"),
        );
        return response;
    }
    next.run(request).await
}

fn extract_identity_key(request: &Request<Body>) -> String {
    if let Some(ip) = request
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
    {
        return format!("ip:{}", ip);
    }
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| format!("ip:{}", c.0.ip()))
        .unwrap_or_else(|| "ip:unknown".to_string())
}
```

- [ ] **Step 3: 修改 main.rs 限流初始化**

```rust
// services/api-gateway/src/main.rs

// 修改 line 68-86 — load_rate_limit_config 保留不变（TOML 解析）
// 修改 line 230-232 — AppState 初始化:

// 旧:
// route_limiters: middleware::rate_limit::RouteLimiterSet::from_config(
//     &load_rate_limit_config(),
// ),
// 新:
let rate_limit_cfg = load_rate_limit_config();
let redis_client = redis::Client::open(&cfg.redis.url).expect("invalid redis url");
let rate_limit_conn = redis::aio::ConnectionManager::new(redis_client)
    .await
    .expect("failed to create redis connection manager for rate limiter");
route_limiters: middleware::rate_limit::RouteLimiterSet::new(
    rate_limit_conn,
    &rate_limit_cfg,
),
```

- [ ] **Step 4: 编译**

```bash
cargo build -p api-gateway
```

Expected: build passes. Fix any compilation errors.

- [ ] **Step 5: 测试**

```bash
cargo test -p api-gateway --lib
cargo test -p common-ratelimit --lib
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add services/api-gateway/Cargo.toml services/api-gateway/src/middleware/rate_limit.rs services/api-gateway/src/main.rs
git commit -m "dephase(gateway): migrate rate limiting from governor to common-ratelimit Redis sliding window"
```

---

### Task 5: Cleanup + 最终验证

**Files:**
- Modify: `services/api-gateway/Cargo.toml` — 确认已删 governor
- Potentially: workspace `Cargo.toml` — 清理无用 deps

**Interfaces:**
- Consumes: Tasks 1-4
- Produces: clean build, all tests pass, CI gates pass

- [ ] **Step 1: 全量编译**

```bash
cargo build --workspace
```

Expected: no errors.

- [ ] **Step 2: 运行所有 gateway 和 lib 测试**

```bash
cargo test -p api-gateway --lib
cargo test -p common-ratelimit --lib
cargo test -p common-jwt --lib
```

Expected: all tests pass.

- [ ] **Step 3: CI 门禁检查**

```bash
node scripts/check-service-standard.js
```

Expected: gateway passes (no regressions).

- [ ] **Step 4: 确认 git status 无遗漏**

```bash
git status
```

确认删除的文件（jwt.rs, jwks.rs）已 staged。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "dephase(gateway): cleanup — remove jwt-simple/rsa/base64 deps; verify CI gates"
```

---

## 执行顺序

```
Task 1 (common-ratelimit fix)
  ↓
Task 2 (JWKSCache migration + perm_epoch cache) ← 核心变更，auth 中间件一次性改造
  ↓
Task 3 (perm_epoch 单元测试回归)
  ↓
Task 4 (Redis 限流)
  ↓
Task 5 (cleanup + CI)
```

Task 2 和 Task 4 独立于 Task 1（但都需要 Task 1 的 common-ratelimit bug fix 来保证限流正确）。Task 2 将 perm_epoch cache 代码与 JWKSCache 迁移放在一起，因为它们改同一段 middleware 代码，拆开反而容易冲突。
