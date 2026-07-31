# Gateway 三件套加固 — 设计

**日期**：2026-07-07 · 起草
**状态**：草案
**关联**：
- 5B-fu2：上游 [`2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md`](2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md) 的遗留项（大票 §5B-fu2）
- 4.4-fu3：上游 [`2026-06-27-phase4.4-permission-changed-design.md`](2026-06-27-phase4.4-permission-changed-design.md) 的遗留项（TODO.md §4.4-fu3）
- Redis 限流：TODO.md §Phase 10 "gateway 限流统一走 Redis"

---

## 0. 一句话

> gateway 是目前唯一还在用 `jwt-simple` 自研验签的服务（其余 7 个已在 Phase 5B 迁移 `digua_common-jwt`），且 perm_epoch 每请求读 Redis、限流走进程内 governor 不共享。本 spec 一次性收束三处：① 接 `digua_common-jwt::JWKSCache` + `verify_token_with_claims()`，② perm_epoch 加 5s 本地缓存，③ 限流从 governor 迁 `digua_common-ratelimit` Redis 滑动窗口。

---

## 0.5. 决策路径回溯

| # | 触发 | 选项 / 决定 | 理由 |
|---|------|-------------|------|
| 1 | 迁移 JWKSCache 后 verify_token 不返回 claims | **digua_common-jwt 加 verify_token_with_claims** | gateway 需要 claims 做下游透传，不复用 verify_token 二次 decode 浪费 |
| 2 | perm_epoch 缓存 TTL | **5s** | 保守，权限变更即使延迟 5s 也不影响安全 |
| 3 | 限流中间件异步化 | **接受异步** | governor 同步 → digua_common-ratelimit Redis 异步，多一次 await，延迟影响 <2ms |
| 4 | 限流身份提取 | **IP 优先** | 限流在认证前执行，只能用 X-Real-IP / peer addr |

---

## 1. 5B-fu2 — JWKSCache 迁移

### 1.1 现状

| 组件 | 文件 | 问题 |
|------|------|------|
| JWT 验证 | `services/api-gateway/src/jwt.rs:83-131` | `jwt_simple::prelude`，单 `RS256PublicKey`，同步 `validate()` |
| JWKS 获取 | `services/api-gateway/src/jwks.rs:95-131` | 启动时 fetch 一次，无后台刷新，不按 `kid` 匹配，无优雅关闭 |
| Claims 提取 | `services/api-gateway/src/jwt.rs:22-81` | `extract_claims()` 从 `jwt_simple::JWTClaims<HashMap>` 展平 |
| 依赖 | `Cargo.toml` | `jwt-simple`, `rsa`, `base64`（其余 7 服务已移除） |

### 1.2 目标

```
删除: src/jwt.rs (131行)
删除: src/jwks.rs (131行)
删除: Cargo.toml 中 jwt-simple, rsa, base64
新增: Cargo.toml 中 libs/digua_common-jwt
修改: libs/digua_common-jwt/src/verify.rs — 新增 verify_token_with_claims()
修改: src/middleware/mod.rs — JwtValidator → JWKSCache + verify_token_with_claims()
修改: src/main.rs — 初始化 JWKSCache + spawn_refresh_loop()
```

### 1.3 verify_token_with_claims 签名

```rust
// libs/digua_common-jwt/src/verify.rs 新增
pub async fn verify_token_with_claims(
    token: &str,
    cache: &JWKSCache,
    expected_aud: Option<&str>,
) -> Result<serde_json::Value, VerifyError>
```

内部复用 `verify_token()` 的 header 解析 + key 查找 + `jsonwebtoken::decode` 逻辑，返回 `TokenData.claims` 为 `serde_json::Value`。

### 1.4 gateway 侧改动

**middleware/mod.rs — auth 中间件**：

```rust
// 旧 (同步)
let validated = state.jwt_validator.validate(&token)?;
let claims = extract_claims(&validated)?;

// 新 (异步)
let claims_value = digua_common_jwt::verify_token_with_claims(
    &token, &state.jwks_cache, Some(&config.audience)
).await?;
let claims = extract_claims_from_value(&claims_value)?;
```

保留 `JwtClaims` 结构体，新增 `extract_claims_from_value(Value) -> Result<JwtClaims>` 替代 `extract_claims(JWTClaims<HashMap>)`。

**main.rs — 初始化**：

```rust
// 旧
let public_key = jwks::fetch_rs256_key(&http_client, &cfg.jwks_url).await?;
let jwt_validator = JwtValidator::from_key(public_key, &cfg.issuer)?;

// 新
let jwks_cache = JWKSCache::new(
    http_client.clone(),
    JwksConfig { url: cfg.jwks_url.clone(), ..Default::default() },
);
jwks_cache.refresh().await?; // 冷启动确保有 key
let jwks_cache = Arc::new(jwks_cache);
digua_common_jwt::spawn_refresh_loop(Arc::clone(&jwks_cache), shutdown_rx);
```

**AppState**：`jwt_validator: Arc<JwtValidator>` → `jwks_cache: Arc<JWKSCache>`。

---

## 2. 4.4-fu3 — perm_epoch 本地缓存

### 2.1 现状

每个已认证请求 per-request 流程（`middleware/mod.rs:105-130`）：

```
1. JWT 验签
2. Redis EXISTS blacklist:{jti}
3. Redis GET perm_epoch:{sub}          ← 每请求一次 Redis
4. 注入 claims → 转发
```

Redis 失败 → 失败开放（所有请求通过）。

### 2.2 目标

在 ③ 前加一层本地进程内缓存：

```
3a. 查本地 perm_epoch_cache.get(sub)
    → 命中 & 未过期 → 跳过 Redis
    → 未命中/过期 → Redis GET → 写入本地 → 返回
```

### 2.3 实现

**新文件**：`services/api-gateway/src/perm_epoch_cache.rs`

```rust
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

pub struct PermEpochCache {
    inner: RwLock<HashMap<u64, (Instant, Option<i64>)>>,
    ttl: Duration,
}

impl PermEpochCache {
    pub fn new(ttl: Duration) -> Self { ... }

    /// 返回 (epoch, was_hit)
    pub fn get(&self, user_id: u64) -> (Option<i64>, bool) {
        if let Some((ts, val)) = self.inner.read().unwrap().get(&user_id) {
            if ts.elapsed() < self.ttl {
                return (*val, true);
            }
        }
        (None, false)
    }

    pub fn set(&self, user_id: u64, epoch: Option<i64>) {
        self.inner.write().unwrap().insert(user_id, (Instant::now(), epoch));
    }
}
```

**middleware/mod.rs**：

```rust
use crate::perm_epoch_cache::PermEpochCache;

// 在 check_revoked 调用前
let (cached_epoch, hit) = state.perm_epoch_cache.get(user_id);
let epoch = if hit {
    cached_epoch
} else {
    let epoch = digua_common_event::perm_epoch::get_perm_epoch(&mut conn, &prefix, sub).await?;
    state.perm_epoch_cache.set(user_id, epoch);
    epoch
};

if is_revoked(epoch, iat) { return 401; }
```

**AppState**：加 `perm_epoch_cache: PermEpochCache`。

**不缓存 blacklist**：JWT jti 是 UUID，缓存收益低（jti 很少重复），复杂度高于收益，不在本次范围。

---

## 3. 限流统一走 Redis

### 3.1 现状

| 组件 | 文件 | 问题 |
|------|------|------|
| 限流引擎 | `src/middleware/rate_limit.rs:84-119` | `governor::RateLimiter<NotKeyed, InMemoryState, DefaultClock>` — 多实例不共享、不区分用户 |
| 配置 | `infrastructure/config/api-gateway/rate-limits.toml` | 8 条 per-route 规则 + 默认 100rps/200burst |
| 身份 | `rate_limit_middleware:148-153` | `JwtClaims.sub` > `X-Real-IP` > peer addr |
| 依赖 | `Cargo.toml` | `governor`（工作区依赖） |

### 3.2 目标

用 `libs/digua_common-ratelimit::RateLimiter`（Redis ZSET 滑动窗口）替换 `governor`。

```
删除: Cargo.toml 中 governor 依赖 (gateway 是唯一使用者, workspace 一起删)
保留: RateLimitConfig / RateLimitRule / path_matches (TOML 解析 + 路径匹配)
保留: rate_limit_middleware 结构、429 JSON 响应、Prometheus 指标
替换: RouteLimiterSet → digua_common-ratelimit::RateLimiter
```

### 3.3 改动

**middleware/rate_limit.rs**：

```rust
// 旧
use governor::{RateLimiter, Quota, ...};
pub struct RouteLimiterSet {
    limiters: HashMap<String, Arc<governor::RateLimiter<...>>>,
}

// 新
use digua_common_ratelimit::RateLimiter;

pub struct RouteLimiterSet {
    rate_limiter: RateLimiter,
    rules: HashMap<String, RateLimitRule>, // path pattern → { max, burst }
}

impl RouteLimiterSet {
    pub fn new(redis: RedisCli, config: RateLimitConfig) -> Self {
        let rate_limiter = RateLimiter::new(redis);
        // 从 config.rules 构造 limits HashMap → rate_limiter.update_limits(limits)
        Self { rate_limiter, rules }
    }

    pub async fn check(&self, path: &str, identity: &str) -> Result<bool, AppError> {
        let rule = self.rule_for(path);
        self.rate_limiter.check(
            &self.rate_limiter.redis_conn(),
            &format!("gateway:{}", path),
            identity,
            rule.max,
            rule.window_secs,
        ).await
    }
}
```

**身份提取**：限流在认证前执行，改回 `X-Real-IP` > peer addr（去掉 `JwtClaims.sub` 分支，此时还没 claims）。

**main.rs**：

```rust
// 旧
let route_limiters = RouteLimiterSet::from_config(&config);

// 新
let rate_limiter = RouteLimiterSet::new(redis_conn.clone(), config);
```

### 3.4 digua_common-ratelimit 适配

当前 `RateLimiter::check(&self, identity: &str, limit: &Limit)` 已接受显式 `Limit` 参数，gateway 可直接构造 `Limit { route, max, window_secs }` 传入。无需 API 变更（ZADD member bug 见 §5）。

### 3.5 删除 governor workspace 依赖

`governor` 仅 gateway 使用。删除 `workspace.dependencies` 中的 `governor` 声明 + gateway Cargo.toml 引用。

---

## 4. 不在本次范围

| 项 | 原因 |
|----|------|
| JWT blacklist 缓存 | 缓存 jti 命中率低（jti one-shot），收益小 |
| perm_epoch 主动过期通知 | 当前 5s TTL 已够；未来可改为 Redis Pub/Sub |
| 限流配置热更新 | 重启生效即可，dev 阶段无热更新需求 |
| gateway 健康检查三级探针 | 留到 K3s/Phase 7 |
| Envoy/Pingora 替换评估 | 独立调研，不影响本次 |

---

## 5. 前置修复：digua_common-ratelimit ZADD member bug

`libs/digua_common-ratelimit/src/lib.rs:130` 当前成员固定为 `identity`：

```rust
.zadd(&key, &identity, now_ms)
```

ZSET member 必须唯一，同一 identity 永远只有一个 member，`ZCARD` 恒为 1，`max > 1` 的限流规则实为 no-op。需改为唯一值：

```rust
.zadd(&key, format!("{}:{}", identity, now_ms), now_ms)
```

本次迁移前在 `digua_common-ratelimit` 修复，并在 gateway 迁移后验证。

---

## 6. 风险与回滚

| 风险 | 缓解 |
|------|------|
| `verify_token_with_claims` 引入 bug 影响全部请求 | `libs/digua_common-jwt` 单测覆盖；gateway 集成测验证 200/401 |
| perm_epoch 缓存导致权限变更延迟 > 5s | 当前 token 15min TTL，5s 缓存可接受；压测确认 |
| Redis 限流增加网关延迟 | Redis 在本地 docker network，延迟 < 1ms；失败开放 |
| 删除 governor 后限流逻辑不准确 | digua_common-ratelimit 已有 ZSET 滑动窗口实测；e2e 加限流测试 |
