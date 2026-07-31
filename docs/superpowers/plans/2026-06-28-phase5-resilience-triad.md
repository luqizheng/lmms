# Phase 5-B 韧性三件套（超时/重试/熔断）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `libs/digua_common-http` 提供通用 `ResilientClient`（包 reqwest 的超时/重试/熔断），并接入 api-gateway 的 `forward_to_service`，每下游 service 一个独立熔断器，状态经 `ResilienceObserver` 暴露到 Prometheus。

**Architecture:** 删除 `digua_common-http` 里从未被使用的韧性脚手架，重写 `CircuitBreaker`（修 Open 计时 bug + HalfOpen 原子占额）与 `RetryPolicy`（rand jitter + 策略归属），新增 `ResilientClient`：收到任何响应即读成 owned `UpstreamResponse`，错误枚举只表达传输层失败；overall deadline 封顶重试总耗时；按 service_name 读锁优先懒创建 breaker；通过库定义的 `ResilienceObserver` trait 解耦 Prometheus。gateway 把裸 reqwest 调用替换为 `ResilientClient::call` 并实现 observer 写指标。

**Tech Stack:** Rust, reqwest 0.12, tokio, axum 0.8（gateway + 测试 mock server），dashmap，bytes，rand 0.8，prometheus 0.13（仅 gateway 侧）。

## Global Constraints

- 数据库/表命名前缀规范不涉及本计划（无 DB 改动）。
- `digua_common-http` **不得**直接依赖 `prometheus`——监控栈解耦由 `ResilienceObserver` trait 承担（决策 3A）。
- 重试**仅**对幂等方法（GET/HEAD/PUT/DELETE/OPTIONS）+ 可重试错误。
- 熔断器**进程内**，不复用 Redis（决策 1A）。
- 服务发现（Consul `discover`）保持在 `forward_to_service` 内、`call` 之前，**不进** `ResilientClient`、不喂 breaker（决策 9A/14B）。
- 默认配置（决策 4A/spec §5.4）：`timeout_ms=3000`、`overall_timeout_ms=8000`、`retry_max_attempts=3`、`retry_base_delay_ms=200`、`retry_max_delay_ms=10000`、`retry_jitter=0.1`、`breaker_failure_threshold=10`、`breaker_open_timeout_ms=30000`、`breaker_half_open_max_calls=3`。
- Spec 参考：`docs/superpowers/specs/2026-06-28-phase5-resilience-triad-design.md`。
- 每个 task 末尾 commit；TDD：先写失败测试再实现。

---

## File Structure

| 文件 | 责任 |
|------|------|
| `libs/digua_common-http/Cargo.toml` | 依赖：加 `dashmap`/`bytes`/`rand`，dev-dep 加 `axum`；移除 `axum`/`tower` 可选 feature |
| `libs/digua_common-http/src/lib.rs` | 模块声明 + 导出表（删孤儿、加新模块） |
| `libs/digua_common-http/src/circuit_breaker.rs` | `CircuitBreaker` 三态状态机（重写） |
| `libs/digua_common-http/src/retry.rs` | `RetryPolicy`：退避 + `should_retry` 策略（重写） |
| `libs/digua_common-http/src/config.rs` | `ResilienceConfig`（重写） |
| `libs/digua_common-http/src/observer.rs` | `ResilienceObserver` trait + `NoopObserver` + `Outcome`（新增） |
| `libs/digua_common-http/src/resilient.rs` | `ResilientClient` / `ResilientError` / `UpstreamResponse`（新增） |
| `libs/digua_common-http/src/client.rs` | 保留现状（`HttpClient`，不动） |
| `libs/digua_common-http/tests/resilient_test.rs` | 集成测试（axum mock 下游）（新增） |
| `services/api-gateway/Cargo.toml` | 加 `digua_common-http` path 依赖 |
| `services/api-gateway/src/router.rs` | `RouterConfig` 改 `resilient`，`forward_to_service` 接线 |
| `services/api-gateway/src/config.rs` | `GatewayConfig` 加 `resilience` 段 |
| `services/api-gateway/src/resilience_metrics.rs` | gateway 实现 `ResilienceObserver` 写 prometheus（新增） |
| `services/api-gateway/src/main.rs` | 构造 `ResilientClient` + observer 注入 |
| `services/api-gateway/src/lib.rs` | 导出新模块 |
| `services/api-gateway/tests/resilience_test.rs` | 错误映射集成测试（新增） |
| `test-case/51-resilience-breaker.md` | 手工验收脚本（新增） |

被删除文件：`libs/digua_common-http/src/axum.rs`、`benchmarks.rs`、`rate_limit.rs`、`middleware.rs`、`service_discovery.rs`。

---

## Task 1: 清理孤儿脚手架 + 依赖调整（绿编译基线）

**Files:**
- Delete: `libs/digua_common-http/src/axum.rs`, `libs/digua_common-http/src/benchmarks.rs`, `libs/digua_common-http/src/rate_limit.rs`, `libs/digua_common-http/src/middleware.rs`, `libs/digua_common-http/src/service_discovery.rs`
- Modify: `libs/digua_common-http/src/lib.rs`, `libs/digua_common-http/Cargo.toml`
- Modify: `Cargo.toml`（workspace，加 `dashmap`）

**Interfaces:**
- Consumes: 无。
- Produces: 一个仅含 `client`（保留）+ 空占位的 `circuit_breaker`/`retry`/`config`/`observer`/`resilient` 模块、可编译的 `digua_common-http`。

- [ ] **Step 1: 确认孤儿文件无外部引用**

Run:
```bash
cd /home/kudi/projects/templates/low-memory-micro-services
grep -rn "digua_common_http::\(CircuitBreaker\|RateLimiter\|RetryPolicy\|CircuitBreakerMiddleware\|RateLimitMiddleware\|ConsulResolver\|DefaultMiddlewareConfig\|HttpDefaultsConfig\)" services/ libs/ --include=*.rs | grep -v "libs/digua_common-http/"
```
Expected: 无输出（确认无外部使用者）。若有输出，停下报告。

- [ ] **Step 2: 删除孤儿文件**

```bash
cd /home/kudi/projects/templates/low-memory-micro-services
git rm libs/digua_common-http/src/axum.rs libs/digua_common-http/src/benchmarks.rs libs/digua_common-http/src/rate_limit.rs libs/digua_common-http/src/middleware.rs libs/digua_common-http/src/service_discovery.rs
```

- [ ] **Step 3: 在 workspace `Cargo.toml` 加 dashmap**

`Cargo.toml` 的 `[workspace.dependencies]` 段（紧邻现有 `bytes = "1"` 之后）加：
```toml
dashmap = "6"
```

- [ ] **Step 4: 重写 `libs/digua_common-http/Cargo.toml`**

整文件替换为：
```toml
[package]
name = "digua_common-http"
version = "0.1.0"
edition = "2021"

[dependencies]
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls-webpki-roots"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
digua_common-error = { path = "../digua_common-error" }
dashmap = { workspace = true }
bytes = { workspace = true }
rand = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
axum = { workspace = true }
```

- [ ] **Step 5: 重写 `libs/digua_common-http/src/lib.rs` 为新模块骨架**

整文件替换为：
```rust
pub mod circuit_breaker;
pub mod client;
pub mod config;
pub mod observer;
pub mod resilient;
pub mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use client::HttpClient;
pub use config::ResilienceConfig;
pub use observer::{NoopObserver, Outcome, ResilienceObserver};
pub use resilient::{ResilientClient, ResilientError, UpstreamResponse};
pub use retry::RetryPolicy;
```

- [ ] **Step 6: 写空占位模块让其编译**

`libs/digua_common-http/src/circuit_breaker.rs`：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct CircuitBreaker;
```

`libs/digua_common-http/src/retry.rs`：
```rust
#[derive(Debug, Clone)]
pub struct RetryPolicy;
```

`libs/digua_common-http/src/config.rs`：
```rust
#[derive(Debug, Clone)]
pub struct ResilienceConfig;
```

`libs/digua_common-http/src/observer.rs`：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Timeout,
    ConnectError,
    UpstreamError,
    CircuitOpen,
}

pub trait ResilienceObserver: Send + Sync {}

pub struct NoopObserver;
impl ResilienceObserver for NoopObserver {}
```

`libs/digua_common-http/src/resilient.rs`：
```rust
pub struct UpstreamResponse;
pub enum ResilientError {
    CircuitOpen,
    Timeout,
    Connect(reqwest::Error),
}
pub struct ResilientClient;
```

- [ ] **Step 7: 编译验证**

Run: `cargo build -p digua_common-http 2>&1 | tail -5`
Expected: 编译成功（可能有 unused warning，可忽略）。

- [ ] **Step 8: Commit**

```bash
git add libs/digua_common-http/ Cargo.toml
git commit -m "refactor(digua_common-http): delete unused resilience scaffolding, scaffold new modules"
```

---

## Task 2: 重写 `CircuitBreaker`（含 HalfOpen 原子占额 + 并发测试）

**Files:**
- Modify: `libs/digua_common-http/src/circuit_breaker.rs`

**Interfaces:**
- Consumes: 无。
- Produces:
  - `enum CircuitState { Closed, Open, HalfOpen }`
  - `struct CircuitBreaker`，方法：
    - `fn new(failure_threshold: u64, open_timeout: Duration, half_open_max_calls: u32) -> Self`
    - `fn is_available(&self) -> bool`
    - `fn record_success(&self)`
    - `fn record_failure(&self)`
    - `fn state(&self) -> CircuitState`

- [ ] **Step 1: 写失败测试 + 类型签名（实现留 `todo!()`）**

`libs/digua_common-http/src/circuit_breaker.rs` 整文件：
```rust
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

// 进程级单调时钟锚点（仅供 breaker 跨请求共享的 opened_at 使用；
// overall deadline 用 call 栈上的 Instant，不走这里 —— 决策 8C）
fn anchor() -> Instant {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    *ANCHOR.get_or_init(Instant::now)
}
fn now_ms() -> u64 {
    anchor().elapsed().as_millis() as u64
}

/// 三态熔断器。
///
/// Closed --failure>=threshold--> Open(记 opened_at_ms)
/// Open --now-opened>=open_timeout--> HalfOpen(清计数)
/// HalfOpen --success>=half_open_max--> Closed
/// HalfOpen --任一 failure--> Open
/// HalfOpen 放行用 fetch_add 原子占额，至多 half_open_max_calls 次（决策 2A）
#[derive(Debug)]
pub struct CircuitBreaker {
    state: AtomicU8, // 0=Closed,1=Open,2=HalfOpen
    failure_count: AtomicU64,
    success_count: AtomicU64,
    half_open_calls: AtomicU64,
    opened_at_ms: AtomicU64,
    failure_threshold: u64,
    open_timeout_ms: u64,
    half_open_max_calls: u64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, open_timeout: Duration, half_open_max_calls: u32) -> Self {
        todo!()
    }
    pub fn state(&self) -> CircuitState {
        todo!()
    }
    pub fn is_available(&self) -> bool {
        todo!()
    }
    pub fn record_success(&self) {
        todo!()
    }
    pub fn record_failure(&self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn cb() -> CircuitBreaker {
        CircuitBreaker::new(3, Duration::from_millis(100), 2)
    }

    #[test]
    fn closed_by_default() {
        let b = cb();
        assert_eq!(b.state(), CircuitState::Closed);
        assert!(b.is_available());
    }

    #[test]
    fn opens_after_threshold() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), CircuitState::Open);
        assert!(!b.is_available());
    }

    #[test]
    fn success_resets_failures_in_closed() {
        let b = cb();
        b.record_failure();
        b.record_failure();
        b.record_success();
        b.record_failure();
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn open_transitions_to_halfopen_after_timeout() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), CircuitState::Open);
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available()); // 计时到 → 转 HalfOpen 放行
        assert_eq!(b.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn halfopen_success_quota_closes() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available());
        b.record_success();
        b.record_success(); // 达 half_open_max_calls=2
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn halfopen_failure_reopens() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available());
        b.record_failure();
        assert_eq!(b.state(), CircuitState::Open);
    }

    // 决策 10A：并发占额，放行数必须 == half_open_max_calls
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn halfopen_concurrent_admission_capped() {
        let b = Arc::new(CircuitBreaker::new(3, Duration::from_millis(50), 5));
        for _ in 0..3 {
            b.record_failure();
        }
        tokio::time::sleep(Duration::from_millis(70)).await;
        let mut handles = Vec::new();
        for _ in 0..100 {
            let bb = b.clone();
            handles.push(tokio::spawn(async move { bb.is_available() }));
        }
        let mut admitted = 0;
        for h in handles {
            if h.await.unwrap() {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 5, "admitted {admitted}, expected exactly half_open_max_calls");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p digua_common-http circuit_breaker 2>&1 | tail -15`
Expected: panic `not yet implemented`。

- [ ] **Step 3: 实现 `CircuitBreaker`**

把五个 `todo!()` 替换为：
```rust
    pub fn new(failure_threshold: u64, open_timeout: Duration, half_open_max_calls: u32) -> Self {
        Self {
            state: AtomicU8::new(0),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            half_open_calls: AtomicU64::new(0),
            opened_at_ms: AtomicU64::new(0),
            failure_threshold,
            open_timeout_ms: open_timeout.as_millis() as u64,
            half_open_max_calls: half_open_max_calls as u64,
        }
    }

    pub fn state(&self) -> CircuitState {
        match self.state.load(Ordering::SeqCst) {
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    fn to_half_open(&self) {
        self.state.store(2, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
    }

    pub fn is_available(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let opened = self.opened_at_ms.load(Ordering::SeqCst);
                if now_ms().saturating_sub(opened) >= self.open_timeout_ms {
                    self.to_half_open();
                    // 转入 HalfOpen 后，本次调用也走占额逻辑
                    self.half_open_calls.fetch_add(1, Ordering::SeqCst) < self.half_open_max_calls
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // 决策 2A：原子占额，超额拒绝
                self.half_open_calls.fetch_add(1, Ordering::SeqCst) < self.half_open_max_calls
            }
        }
    }

    pub fn record_success(&self) {
        match self.state() {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let s = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if s >= self.half_open_max_calls {
                    self.state.store(0, Ordering::SeqCst);
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        match self.state() {
            CircuitState::Closed => {
                let f = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if f >= self.failure_threshold {
                    self.opened_at_ms.store(now_ms(), Ordering::SeqCst);
                    self.state.store(1, Ordering::SeqCst);
                }
            }
            CircuitState::HalfOpen => {
                self.opened_at_ms.store(now_ms(), Ordering::SeqCst);
                self.state.store(1, Ordering::SeqCst);
            }
            CircuitState::Open => {}
        }
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p digua_common-http circuit_breaker 2>&1 | tail -15`
Expected: 全部 PASS（含 `halfopen_concurrent_admission_capped`）。

- [ ] **Step 5: Commit**

```bash
git add libs/digua_common-http/src/circuit_breaker.rs
git commit -m "feat(digua_common-http): rewrite CircuitBreaker with fixed open-timer + atomic half-open admission"
```

---

## Task 3: 重写 `RetryPolicy`（rand jitter + `should_retry` 策略归属）

**Files:**
- Modify: `libs/digua_common-http/src/retry.rs`

**Interfaces:**
- Consumes: `crate::resilient::ResilientError`（Task 1 占位已存在：`CircuitOpen`/`Timeout`/`Connect`）。
- Produces:
  - `fn is_idempotent(method: &reqwest::Method) -> bool`（pub，无状态）
  - `struct RetryPolicy { max_attempts, base_delay, max_delay, jitter }`，方法：
    - `fn new(max_attempts, base_delay, max_delay, jitter) -> Self`
    - `fn delay_for(&self, attempt: u32) -> Duration`
    - `fn should_retry_err(&self, attempt: u32, err: &ResilientError, method: &reqwest::Method) -> bool`
    - `fn should_retry_status(&self, attempt: u32, status: reqwest::StatusCode, method: &reqwest::Method) -> bool`

> 说明（决策 6A）：拆成 `should_retry_err`（传输层错误）与 `should_retry_status`（拿到响应的 status），因为 5A 后两类信号类型不同；二者都内部调用 `is_idempotent`。

- [ ] **Step 1: 写失败测试 + 签名**

`libs/digua_common-http/src/retry.rs` 整文件：
```rust
use crate::resilient::ResilientError;
use rand::Rng;
use reqwest::{Method, StatusCode};
use std::time::Duration;

/// 幂等方法才允许重试。
pub fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS
    )
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter: f64,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration, jitter: f64) -> Self {
        todo!()
    }
    pub fn delay_for(&self, attempt: u32) -> Duration {
        todo!()
    }
    pub fn should_retry_err(&self, attempt: u32, err: &ResilientError, method: &Method) -> bool {
        todo!()
    }
    pub fn should_retry_status(&self, attempt: u32, status: StatusCode, method: &Method) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> RetryPolicy {
        RetryPolicy::new(3, Duration::from_millis(200), Duration::from_secs(10), 0.1)
    }

    #[test]
    fn exponential_backoff_within_jitter_bound() {
        let p = p();
        let d1 = p.delay_for(1).as_millis() as u64;
        assert!((200..=220).contains(&d1), "d1={d1}");
        let d2 = p.delay_for(2).as_millis() as u64;
        assert!((400..=440).contains(&d2), "d2={d2}");
    }

    #[test]
    fn backoff_capped_at_max() {
        let p = RetryPolicy::new(5, Duration::from_secs(8), Duration::from_secs(10), 0.0);
        assert_eq!(p.delay_for(3), Duration::from_secs(10)); // 8*4=32 capped to 10
    }

    #[test]
    fn circuit_open_never_retries() {
        let p = p();
        assert!(!p.should_retry_err(1, &ResilientError::CircuitOpen, &Method::GET));
    }

    #[test]
    fn timeout_retry_only_idempotent() {
        let p = p();
        assert!(p.should_retry_err(1, &ResilientError::Timeout, &Method::GET));
        assert!(!p.should_retry_err(1, &ResilientError::Timeout, &Method::POST));
    }

    #[test]
    fn status_retry_matrix() {
        let p = p();
        assert!(p.should_retry_status(1, StatusCode::BAD_GATEWAY, &Method::GET)); // 502
        assert!(p.should_retry_status(1, StatusCode::SERVICE_UNAVAILABLE, &Method::PUT)); // 503
        assert!(p.should_retry_status(1, StatusCode::TOO_MANY_REQUESTS, &Method::GET)); // 429
        assert!(!p.should_retry_status(1, StatusCode::BAD_GATEWAY, &Method::POST)); // 非幂等
        assert!(!p.should_retry_status(1, StatusCode::INTERNAL_SERVER_ERROR, &Method::GET)); // 500
        assert!(!p.should_retry_status(1, StatusCode::OK, &Method::GET)); // 2xx
        assert!(!p.should_retry_status(1, StatusCode::BAD_REQUEST, &Method::GET)); // 4xx
    }

    #[test]
    fn exhausted_attempts_no_retry() {
        let p = p();
        assert!(!p.should_retry_err(3, &ResilientError::Timeout, &Method::GET)); // attempt==max
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p digua_common-http retry 2>&1 | tail -15`
Expected: panic `not yet implemented`。

- [ ] **Step 3: 实现**

替换四个 `todo!()`：
```rust
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration, jitter: f64) -> Self {
        Self { max_attempts, base_delay, max_delay, jitter }
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        let capped = self.base_delay.saturating_mul(factor).min(self.max_delay);
        let extra_ms = (capped.as_millis() as f64 * self.jitter) as u64;
        let jitter_ms = if extra_ms > 0 {
            rand::thread_rng().gen_range(0..=extra_ms)
        } else {
            0
        };
        capped + Duration::from_millis(jitter_ms)
    }

    pub fn should_retry_err(&self, attempt: u32, err: &ResilientError, method: &Method) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        match err {
            ResilientError::CircuitOpen => false,
            ResilientError::Timeout | ResilientError::Connect(_) => is_idempotent(method),
        }
    }

    pub fn should_retry_status(&self, attempt: u32, status: StatusCode, method: &Method) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        let retriable = matches!(status.as_u16(), 502 | 503 | 429);
        retriable && is_idempotent(method)
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p digua_common-http retry 2>&1 | tail -15`
Expected: 全部 PASS。

- [ ] **Step 5: Commit**

```bash
git add libs/digua_common-http/src/retry.rs
git commit -m "feat(digua_common-http): rewrite RetryPolicy with rand jitter + typed should_retry"
```

---

## Task 4: `ResilienceConfig`（配置 + 默认值）

**Files:**
- Modify: `libs/digua_common-http/src/config.rs`

**Interfaces:**
- Consumes: `crate::circuit_breaker::CircuitBreaker`, `crate::retry::RetryPolicy`。
- Produces:
  - `struct ResilienceConfig`（字段全 pub，见下）+ `impl Default`
  - `fn timeout(&self) -> Duration`, `fn overall_timeout(&self) -> Duration`, `fn retry_policy(&self) -> RetryPolicy`, `fn new_breaker(&self) -> CircuitBreaker`

- [ ] **Step 1: 写失败测试 + 实现（配置简单，一步写完）**

`libs/digua_common-http/src/config.rs` 整文件：
```rust
use crate::circuit_breaker::CircuitBreaker;
use crate::retry::RetryPolicy;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ResilienceConfig {
    pub timeout_ms: u64,
    pub overall_timeout_ms: u64,
    pub retry_max_attempts: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter: f64,
    pub breaker_failure_threshold: u64,
    pub breaker_open_timeout_ms: u64,
    pub breaker_half_open_max_calls: u32,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 3000,
            overall_timeout_ms: 8000,
            retry_max_attempts: 3,
            retry_base_delay_ms: 200,
            retry_max_delay_ms: 10000,
            retry_jitter: 0.1,
            breaker_failure_threshold: 10,
            breaker_open_timeout_ms: 30000,
            breaker_half_open_max_calls: 3,
        }
    }
}

impl ResilienceConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
    pub fn overall_timeout(&self) -> Duration {
        Duration::from_millis(self.overall_timeout_ms)
    }
    pub fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::new(
            self.retry_max_attempts,
            Duration::from_millis(self.retry_base_delay_ms),
            Duration::from_millis(self.retry_max_delay_ms),
            self.retry_jitter,
        )
    }
    pub fn new_breaker(&self) -> CircuitBreaker {
        CircuitBreaker::new(
            self.breaker_failure_threshold,
            Duration::from_millis(self.breaker_open_timeout_ms),
            self.breaker_half_open_max_calls,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = ResilienceConfig::default();
        assert_eq!(c.timeout_ms, 3000);
        assert_eq!(c.overall_timeout_ms, 8000);
        assert_eq!(c.retry_max_attempts, 3);
        assert_eq!(c.breaker_failure_threshold, 10);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let c: ResilienceConfig = serde_json::from_str(r#"{"timeout_ms":1500}"#).unwrap();
        assert_eq!(c.timeout_ms, 1500);
        assert_eq!(c.overall_timeout_ms, 8000); // 默认补齐
    }

    #[test]
    fn builds_policy_and_breaker() {
        let c = ResilienceConfig::default();
        assert_eq!(c.retry_policy().max_attempts, 3);
        let _ = c.new_breaker();
        assert_eq!(c.timeout(), Duration::from_millis(3000));
    }
}
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cargo test -p digua_common-http config 2>&1 | tail -15`
Expected: 全部 PASS。

- [ ] **Step 3: Commit**

```bash
git add libs/digua_common-http/src/config.rs
git commit -m "feat(digua_common-http): add ResilienceConfig with spec defaults + builders"
```

---

## Task 5: `ResilienceObserver` trait + `NoopObserver` + `Outcome`

**Files:**
- Modify: `libs/digua_common-http/src/observer.rs`

**Interfaces:**
- Consumes: `crate::circuit_breaker::CircuitState`。
- Produces:
  - `enum Outcome { Success, Timeout, ConnectError, UpstreamError, CircuitOpen }`
  - `trait ResilienceObserver: Send + Sync`，方法 `on_state_change(&self, service: &str, state: CircuitState)` / `on_outcome(&self, service: &str, outcome: Outcome)` / `on_retry(&self, service: &str)`，均带空默认实现
  - `struct NoopObserver`

- [ ] **Step 1: 写测试 + 实现（一步到位）**

`libs/digua_common-http/src/observer.rs` 整文件：
```rust
use crate::circuit_breaker::CircuitState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Timeout,
    ConnectError,
    UpstreamError,
    CircuitOpen,
}

/// 库定义契约，调用方实现写监控（决策 3A）。默认方法为空，
/// 仅关心部分事件的实现可只覆盖需要的方法。
pub trait ResilienceObserver: Send + Sync {
    fn on_state_change(&self, _service: &str, _state: CircuitState) {}
    fn on_outcome(&self, _service: &str, _outcome: Outcome) {}
    fn on_retry(&self, _service: &str) {}
}

/// 零成本默认实现，供不接监控的调用方使用。
pub struct NoopObserver;
impl ResilienceObserver for NoopObserver {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        states: Mutex<Vec<(String, CircuitState)>>,
        outcomes: Mutex<Vec<(String, Outcome)>>,
        retries: Mutex<Vec<String>>,
    }
    impl ResilienceObserver for RecordingObserver {
        fn on_state_change(&self, service: &str, state: CircuitState) {
            self.states.lock().unwrap().push((service.into(), state));
        }
        fn on_outcome(&self, service: &str, outcome: Outcome) {
            self.outcomes.lock().unwrap().push((service.into(), outcome));
        }
        fn on_retry(&self, service: &str) {
            self.retries.lock().unwrap().push(service.into());
        }
    }

    #[test]
    fn recording_observer_captures_events() {
        let o = RecordingObserver::default();
        o.on_outcome("iam", Outcome::Success);
        o.on_state_change("iam", CircuitState::Open);
        o.on_retry("iam");
        assert_eq!(o.outcomes.lock().unwrap().len(), 1);
        assert_eq!(o.states.lock().unwrap()[0].1, CircuitState::Open);
        assert_eq!(o.retries.lock().unwrap()[0], "iam");
    }

    #[test]
    fn noop_observer_is_silent() {
        let o = NoopObserver;
        o.on_outcome("x", Outcome::Timeout); // 不 panic 即可
    }
}
```

- [ ] **Step 2: 运行测试确认通过**

Run: `cargo test -p digua_common-http observer 2>&1 | tail -10`
Expected: PASS。

- [ ] **Step 3: Commit**

```bash
git add libs/digua_common-http/src/observer.rs
git commit -m "feat(digua_common-http): add ResilienceObserver trait + NoopObserver (metrics decoupling)"
```

---

## Task 6: `ResilientClient`（call 循环）+ owned 响应 + 集成测试

**Files:**
- Modify: `libs/digua_common-http/src/resilient.rs`
- Create: `libs/digua_common-http/tests/resilient_test.rs`

**Interfaces:**
- Consumes: `CircuitBreaker`, `CircuitState`, `RetryPolicy`/`is_idempotent`, `ResilienceConfig`, `ResilienceObserver`/`Outcome`/`NoopObserver`。
- Produces:
  - `struct UpstreamResponse { status: reqwest::StatusCode, headers: reqwest::header::HeaderMap, body: bytes::Bytes }`（字段全 pub）
  - `enum ResilientError { CircuitOpen, Timeout, Connect(reqwest::Error) }`（derive Debug）
  - `struct ResilientClient`，方法：
    - `fn new(inner: reqwest::Client, cfg: ResilienceConfig, observer: Arc<dyn ResilienceObserver>) -> Self`
    - `async fn call(&self, service: &str, method: reqwest::Method, url: &str, headers: &reqwest::header::HeaderMap, body: bytes::Bytes) -> Result<UpstreamResponse, ResilientError>`

- [ ] **Step 1: 实现 `resilient.rs`**

`libs/digua_common-http/src/resilient.rs` 整文件替换：
```rust
use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::config::ResilienceConfig;
use crate::observer::{Outcome, ResilienceObserver};
use crate::retry::RetryPolicy;
use bytes::Bytes;
use dashmap::DashMap;
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, StatusCode};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::timeout;

/// 收到任何 HTTP 响应（含 5xx）都读成 owned 结构（决策 5A），
/// 消除 reqwest Response body 流读一次即消费与重试/透传的所有权冲突。
pub struct UpstreamResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// 仅表达「没拿到响应」的传输层失败（决策 5A）。
/// 5xx/429 不是 Err —— 它们作为 Ok(UpstreamResponse) 返回。
#[derive(Debug)]
pub enum ResilientError {
    CircuitOpen,
    Timeout,
    Connect(reqwest::Error),
}

/// outbound 韧性客户端：超时 + 重试 + 按 service 熔断（决策 1A/4A/15A）。
///
/// 服务发现在 call 之前完成、不进本结构、不喂 breaker（决策 9A）。
///
/// call 单次循环：
///   deadline 超 overall? -> Timeout
///   breaker 不可用? -> CircuitOpen
///   timeout(单次, send) -> Ok(resp 读 owned) / Connect / Timeout
///   按 status/err 决定重试(仅幂等) 或 返回
pub struct ResilientClient {
    inner: Client,
    cfg: ResilienceConfig,
    retry: RetryPolicy,
    breakers: DashMap<String, Arc<CircuitBreaker>>,
    observer: Arc<dyn ResilienceObserver>,
}

impl ResilientClient {
    pub fn new(inner: Client, cfg: ResilienceConfig, observer: Arc<dyn ResilienceObserver>) -> Self {
        let retry = cfg.retry_policy();
        Self {
            inner,
            cfg,
            retry,
            breakers: DashMap::new(),
            observer,
        }
    }

    // 决策 15A：读锁命中直接 clone Arc；未命中才写锁创建。
    fn breaker_for(&self, service: &str) -> Arc<CircuitBreaker> {
        if let Some(b) = self.breakers.get(service) {
            return b.clone();
        }
        self.breakers
            .entry(service.to_string())
            .or_insert_with(|| Arc::new(self.cfg.new_breaker()))
            .clone()
    }

    fn note_state(&self, service: &str, before: CircuitState, breaker: &CircuitBreaker) {
        let after = breaker.state();
        if after != before {
            self.observer.on_state_change(service, after);
        }
    }

    pub async fn call(
        &self,
        service: &str,
        method: Method,
        url: &str,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<UpstreamResponse, ResilientError> {
        let deadline = Instant::now(); // 决策 8C：栈上局部
        let breaker = self.breaker_for(service);
        let mut attempt = 0u32;

        loop {
            attempt += 1;

            // 决策 4A：overall deadline 封顶（含重试+退避）
            if deadline.elapsed() >= self.cfg.overall_timeout() {
                self.observer.on_outcome(service, Outcome::Timeout);
                return Err(ResilientError::Timeout);
            }

            let st_before = breaker.state();
            if !breaker.is_available() {
                self.note_state(service, st_before, &breaker);
                self.observer.on_outcome(service, Outcome::CircuitOpen);
                return Err(ResilientError::CircuitOpen);
            }
            self.note_state(service, st_before, &breaker);

            // 决策 16A：headers/body 入口准备一次，每轮用引用 + Bytes O(1) clone 重建
            let req = self
                .inner
                .request(method.clone(), url)
                .headers(headers.clone())
                .body(body.clone());

            let result = timeout(self.cfg.timeout(), req.send()).await;

            match result {
                // 拿到响应（含 5xx）—— 决策 5A：读成 owned
                Ok(Ok(resp)) => {
                    let status = resp.status();
                    let resp_headers = resp.headers().clone();
                    let body_bytes = resp.bytes().await.unwrap_or_default();

                    // breaker 计失败口径（spec §5.3）：502/503/429 计失败，其余成功
                    let is_failure = matches!(status.as_u16(), 502 | 503 | 429);
                    let st = breaker.state();
                    if is_failure {
                        breaker.record_failure();
                    } else {
                        breaker.record_success();
                    }
                    self.note_state(service, st, &breaker);

                    // 决策 6A：status 是否重试
                    if self.retry.should_retry_status(attempt, status, &method)
                        && deadline.elapsed() < self.cfg.overall_timeout()
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }

                    self.observer.on_outcome(
                        service,
                        if is_failure { Outcome::UpstreamError } else { Outcome::Success },
                    );
                    return Ok(UpstreamResponse {
                        status,
                        headers: resp_headers,
                        body: body_bytes,
                    });
                }
                // reqwest 传输错误（请求未送达）
                Ok(Err(e)) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Connect(e);
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < self.cfg.overall_timeout()
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::ConnectError);
                    return Err(err);
                }
                // 单次超时
                Err(_) => {
                    let st = breaker.state();
                    breaker.record_failure();
                    self.note_state(service, st, &breaker);
                    let err = ResilientError::Timeout;
                    if self.retry.should_retry_err(attempt, &err, &method)
                        && deadline.elapsed() < self.cfg.overall_timeout()
                    {
                        self.observer.on_retry(service);
                        tokio::time::sleep(self.retry.delay_for(attempt)).await;
                        continue;
                    }
                    self.observer.on_outcome(service, Outcome::Timeout);
                    return Err(err);
                }
            }
        }
    }
}
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p digua_common-http 2>&1 | tail -10`
Expected: 编译成功。

- [ ] **Step 3: 写集成测试（axum mock 下游；含 observer 回调 13A、deadline 11A）**

`libs/digua_common-http/tests/resilient_test.rs` 整文件：
```rust
use bytes::Bytes;
use digua_common_http::CircuitState;
use digua_common_http::{
    NoopObserver, Outcome, ResilienceConfig, ResilienceObserver, ResilientClient, ResilientError,
};
use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

async fn spawn_server(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn cfg() -> ResilienceConfig {
    ResilienceConfig {
        timeout_ms: 300,
        overall_timeout_ms: 1000,
        retry_max_attempts: 3,
        retry_base_delay_ms: 10,
        retry_max_delay_ms: 50,
        retry_jitter: 0.0,
        breaker_failure_threshold: 3,
        breaker_open_timeout_ms: 200,
        breaker_half_open_max_calls: 2,
    }
}

fn client(cfg: ResilienceConfig, obs: Arc<dyn ResilienceObserver>) -> ResilientClient {
    ResilientClient::new(Client::new(), cfg, obs)
}

#[derive(Default)]
struct Recorder {
    outcomes: Mutex<Vec<Outcome>>,
    states: Mutex<Vec<CircuitState>>,
    retries: AtomicU64,
}
impl ResilienceObserver for Recorder {
    fn on_state_change(&self, _s: &str, st: CircuitState) {
        self.states.lock().unwrap().push(st);
    }
    fn on_outcome(&self, _s: &str, o: Outcome) {
        self.outcomes.lock().unwrap().push(o);
    }
    fn on_retry(&self, _s: &str) {
        self.retries.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn success_returns_owned_body_and_records_success() {
    let app = axum::Router::new().route("/ok", axum::routing::get(|| async { "hello" }));
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let resp = c
        .call("svc", Method::GET, &format!("{base}/ok"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok");
    assert_eq!(resp.status, StatusCode::OK);
    assert_eq!(&resp.body[..], b"hello");
    assert_eq!(rec.outcomes.lock().unwrap()[0], Outcome::Success);
}

#[tokio::test]
async fn retries_502_for_idempotent_then_exhausts() {
    let hits = Arc::new(AtomicU64::new(0));
    let h2 = hits.clone();
    let app = axum::Router::new().route(
        "/bad",
        axum::routing::get(move || {
            let h = h2.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::BAD_GATEWAY
            }
        }),
    );
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let resp = c
        .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok upstream resp");
    assert_eq!(resp.status, StatusCode::BAD_GATEWAY); // 重试耗尽后透传 502（5A）
    assert_eq!(hits.load(Ordering::SeqCst), 3); // max_attempts 次
    assert!(rec.retries.load(Ordering::SeqCst) >= 1);
}

#[tokio::test]
async fn post_502_not_retried() {
    let hits = Arc::new(AtomicU64::new(0));
    let h2 = hits.clone();
    let app = axum::Router::new().route(
        "/bad",
        axum::routing::post(move || {
            let h = h2.clone();
            async move {
                h.fetch_add(1, Ordering::SeqCst);
                StatusCode::BAD_GATEWAY
            }
        }),
    );
    let base = spawn_server(app).await;
    let c = client(cfg(), Arc::new(NoopObserver));
    let _ = c
        .call("svc", Method::POST, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await;
    assert_eq!(hits.load(Ordering::SeqCst), 1); // 非幂等不重试
}

#[tokio::test]
async fn overall_deadline_caps_retries() {
    // 每次都慢于单次 timeout(300ms) → 触发超时重试，但 overall(1000ms) 封顶
    let app = axum::Router::new().route(
        "/slow",
        axum::routing::get(|| async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            "late"
        }),
    );
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let c = client(cfg(), rec.clone());
    let start = Instant::now();
    let err = c
        .call("svc", Method::GET, &format!("{base}/slow"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("should time out");
    let elapsed = start.elapsed();
    assert!(matches!(err, ResilientError::Timeout));
    assert!(elapsed < Duration::from_millis(1500), "elapsed={elapsed:?}"); // 受 overall 约束
}

#[tokio::test]
async fn breaker_opens_then_short_circuits() {
    let app = axum::Router::new()
        .route("/bad", axum::routing::get(|| async { StatusCode::SERVICE_UNAVAILABLE }));
    let base = spawn_server(app).await;
    let rec = Arc::new(Recorder::default());
    let mut c = cfg();
    c.retry_max_attempts = 1; // 不重试，便于快速累积失败计数
    let client = client(c, rec.clone());
    for _ in 0..3 {
        let _ = client
            .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
            .await;
    }
    let err = client
        .call("svc", Method::GET, &format!("{base}/bad"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("circuit open");
    assert!(matches!(err, ResilientError::CircuitOpen));
    assert!(rec.outcomes.lock().unwrap().contains(&Outcome::CircuitOpen));
    assert!(rec.states.lock().unwrap().contains(&CircuitState::Open));
}

#[tokio::test]
async fn breaker_recovers_after_open_timeout() {
    let fail = Arc::new(AtomicBool::new(true));
    let f2 = fail.clone();
    let app = axum::Router::new().route(
        "/flap",
        axum::routing::get(move || {
            let f = f2.clone();
            async move {
                if f.load(Ordering::SeqCst) {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::OK
                }
            }
        }),
    );
    let base = spawn_server(app).await;
    let mut c = cfg();
    c.retry_max_attempts = 1;
    let client = client(c, Arc::new(NoopObserver));
    for _ in 0..3 {
        let _ = client
            .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
            .await;
    }
    fail.store(false, Ordering::SeqCst); // 下游恢复
    tokio::time::sleep(Duration::from_millis(250)).await; // 过 open_timeout(200ms)
    let r1 = client
        .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("halfopen probe");
    assert_eq!(r1.status, StatusCode::OK);
    let r2 = client
        .call("svc", Method::GET, &format!("{base}/flap"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("recovered");
    assert_eq!(r2.status, StatusCode::OK);
}
```

- [ ] **Step 4: 运行集成测试确认通过**

Run: `cargo test -p digua_common-http --test resilient_test 2>&1 | tail -25`
Expected: 全部 PASS。若报 `axum::serve` 签名问题，确认 dev-dep axum 版本 = workspace 0.8。

- [ ] **Step 5: 跑全 crate 测试 + clippy**

Run: `cargo test -p digua_common-http 2>&1 | tail -10 && cargo clippy -p digua_common-http 2>&1 | tail -10`
Expected: 测试全 PASS；clippy 无 error。

- [ ] **Step 6: Commit**

```bash
git add libs/digua_common-http/src/resilient.rs libs/digua_common-http/tests/resilient_test.rs
git commit -m "feat(digua_common-http): add ResilientClient (timeout/retry/per-service breaker)"
```

---

## Task 7: gateway 接线（RouterConfig + forward_to_service + 配置段）

**Files:**
- Modify: `services/api-gateway/Cargo.toml`
- Modify: `services/api-gateway/src/router.rs`
- Modify: `services/api-gateway/src/config.rs`
- Modify: `services/api-gateway/src/main.rs`

**Interfaces:**
- Consumes: `digua_common_http::{ResilientClient, ResilientError, ResilienceConfig, NoopObserver}`；`crate::discovery::ServiceDiscovery`（`discover() -> Result<ServiceInstance{address:String,port:u16}, StatusCode>`）。
- Produces:
  - `RouterConfig { discovery: Arc<ServiceDiscovery>, resilient: Arc<ResilientClient> }`
  - `forward_to_service` 用 `resilient.call` + 四态映射（决策 5A）。
  - `GatewayConfig.resilience: ResilienceConfig`。

> 注：本任务 observer 先用 `NoopObserver` 占位，Task 8 换成真 observer。

- [ ] **Step 1: gateway Cargo 加 digua_common-http 依赖**

`services/api-gateway/Cargo.toml` 的 `[dependencies]` 段，在 `digua_common-event = { path = "../../libs/digua_common-event" }` 之后加：
```toml
digua_common-http = { path = "../../libs/digua_common-http" }
```

- [ ] **Step 2: `GatewayConfig` 加 resilience 段**

`services/api-gateway/src/config.rs`：
- 顶部 use 区加：`use digua_common_http::ResilienceConfig;`
- 在 `pub struct GatewayConfig { ... }` 的 `pub routes: Vec<RouteConfig>,` 字段**前**加：
```rust
    #[serde(default)]
    pub resilience: ResilienceConfig,
```
- 在 `impl Default for GatewayConfig` 的 `routes: Vec::new(),` **前**加：
```rust
            resilience: ResilienceConfig::default(),
```

- [ ] **Step 3: 改写 `RouterConfig` 与 `forward_to_service`**

`services/api-gateway/src/router.rs` 整文件替换为：
```rust
use crate::discovery::ServiceDiscovery;
use axum::body::Body;
use axum::http::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use digua_common_http::{ResilientClient, ResilientError};
use std::sync::Arc;

#[derive(Clone)]
pub struct RouterConfig {
    pub discovery: Arc<ServiceDiscovery>,
    pub resilient: Arc<ResilientClient>,
}

/// 转发请求到 Consul 发现的后端服务（target_path_query 已由模板渲染好）。
///
/// 服务发现在韧性边界之外（决策 9A）：discover 在此完成、不进 ResilientClient、
/// 不喂 breaker —— breaker 信号只反映下游实例健康，不被 Consul 故障污染。
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
) -> Response {
    // --- 韧性边界之外：服务发现 ---
    let instance = match cfg.discovery.discover(service_name).await {
        Ok(i) => i,
        Err(status) => return status.into_response(),
    };

    let target_url = format!(
        "http://{}:{}{}",
        instance.address, instance.port, target_path_query
    );

    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    // --- 韧性边界之内：超时/重试/熔断 ---
    match cfg
        .resilient
        .call(service_name, parts.method, &target_url, &parts.headers, bytes)
        .await
    {
        Ok(resp) => {
            let mut builder = Response::builder().status(resp.status);
            for (key, value) in resp.headers.iter() {
                let key_str = key.as_str();
                if !["transfer-encoding", "connection"].contains(&key_str) {
                    builder = builder.header(key_str, value.as_bytes());
                }
            }
            builder
                .body(Body::from(resp.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(ResilientError::CircuitOpen) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(serde_json::json!({
                "error": "circuit_open",
                "service": service_name
            })),
        )
            .into_response(),
        Err(ResilientError::Timeout) => {
            tracing::warn!("forward to {} timed out", service_name);
            (StatusCode::GATEWAY_TIMEOUT, "upstream timeout").into_response()
        }
        Err(ResilientError::Connect(e)) => {
            tracing::warn!("forward to {} connect error: {:?}", service_name, e);
            (StatusCode::BAD_GATEWAY, "upstream error").into_response()
        }
    }
}
```

- [ ] **Step 4: `main.rs` 构造 ResilientClient**

`services/api-gateway/src/main.rs`：
- 在 use 区加：`use digua_common_http::{ResilientClient, NoopObserver};`（`Arc` 已 in-scope，无需再加）。
- 把第 167-170 行：
```rust
    let router_cfg = Arc::new(RouterConfig {
        discovery: discovery.clone(),
        http_client: reqwest::Client::new(),
    });
```
替换为：
```rust
    let resilient = Arc::new(ResilientClient::new(
        reqwest::Client::new(),
        cfg.resilience.clone(),
        Arc::new(NoopObserver),
    ));
    let router_cfg = Arc::new(RouterConfig {
        discovery: discovery.clone(),
        resilient,
    });
```

- [ ] **Step 5: 编译验证**

Run: `cargo build -p api-gateway 2>&1 | tail -15`
Expected: 编译成功。

- [ ] **Step 6: 跑 gateway 现有测试确认未回归**

Run: `cargo test -p api-gateway 2>&1 | tail -15`
Expected: 现有测试全 PASS（接线不改路由/JWT 逻辑）。

- [ ] **Step 7: Commit**

```bash
git add services/api-gateway/Cargo.toml services/api-gateway/src/router.rs services/api-gateway/src/config.rs services/api-gateway/src/main.rs
git commit -m "feat(gateway): wire forward_to_service through ResilientClient"
```

---

## Task 8: gateway 实现 `ResilienceObserver` 写 Prometheus + 错误映射集成测试

**Files:**
- Create: `services/api-gateway/src/resilience_metrics.rs`
- Modify: `services/api-gateway/src/lib.rs`
- Modify: `services/api-gateway/src/main.rs`
- Create: `services/api-gateway/tests/resilience_test.rs`

**Interfaces:**
- Consumes: `digua_common_http::{ResilienceObserver, Outcome, CircuitState}`；`prometheus` 0.13（gateway 已依赖）。
- Produces: `GatewayMetricsObserver`（impl `ResilienceObserver`，写默认 prometheus registry）。

- [ ] **Step 1: 实现 gateway observer**

`services/api-gateway/src/resilience_metrics.rs` 整文件：
```rust
use digua_common_http::{CircuitState, Outcome, ResilienceObserver};
use prometheus::{register_int_counter_vec, register_int_gauge_vec, IntCounterVec, IntGaugeVec};
use std::sync::OnceLock;

struct Metrics {
    circuit_state: IntGaugeVec,
    requests: IntCounterVec,
    retries: IntCounterVec,
}

fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(|| Metrics {
        circuit_state: register_int_gauge_vec!(
            "gateway_circuit_state",
            "Circuit breaker state per downstream service (0=Closed,1=HalfOpen,2=Open)",
            &["service"]
        )
        .unwrap(),
        requests: register_int_counter_vec!(
            "gateway_upstream_requests_total",
            "Upstream request outcomes per downstream service",
            &["service", "outcome"]
        )
        .unwrap(),
        retries: register_int_counter_vec!(
            "gateway_upstream_retries_total",
            "Upstream retries per downstream service",
            &["service"]
        )
        .unwrap(),
    })
}

fn state_code(s: CircuitState) -> i64 {
    match s {
        CircuitState::Closed => 0,
        CircuitState::HalfOpen => 1,
        CircuitState::Open => 2,
    }
}

fn outcome_label(o: Outcome) -> &'static str {
    match o {
        Outcome::Success => "success",
        Outcome::Timeout => "timeout",
        Outcome::ConnectError => "connect_error",
        Outcome::UpstreamError => "upstream_error",
        Outcome::CircuitOpen => "circuit_open",
    }
}

/// gateway 侧 observer：把韧性事件写入默认 prometheus registry，
/// 经现有 /metrics 端点暴露（决策 3A）。
pub struct GatewayMetricsObserver;

impl ResilienceObserver for GatewayMetricsObserver {
    fn on_state_change(&self, service: &str, state: CircuitState) {
        metrics()
            .circuit_state
            .with_label_values(&[service])
            .set(state_code(state));
    }
    fn on_outcome(&self, service: &str, outcome: Outcome) {
        metrics()
            .requests
            .with_label_values(&[service, outcome_label(outcome)])
            .inc();
    }
    fn on_retry(&self, service: &str) {
        metrics().retries.with_label_values(&[service]).inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_records_without_panic() {
        let o = GatewayMetricsObserver;
        o.on_outcome("iam", Outcome::Success);
        o.on_state_change("iam", CircuitState::Open);
        o.on_retry("iam");
        let txt = {
            use prometheus::{Encoder, TextEncoder};
            let mut buf = Vec::new();
            TextEncoder::new()
                .encode(&prometheus::gather(), &mut buf)
                .unwrap();
            String::from_utf8(buf).unwrap()
        };
        assert!(txt.contains("gateway_circuit_state"));
        assert!(txt.contains("gateway_upstream_requests_total"));
    }
}
```

- [ ] **Step 2: lib.rs 导出 + main.rs 用真 observer**

`services/api-gateway/src/lib.rs`：在模块声明区（其他 `pub mod` 同处）加：
```rust
pub mod resilience_metrics;
```

`services/api-gateway/src/main.rs`：
- 把 Task 7 的 `use digua_common_http::{ResilientClient, NoopObserver};` 改为 `use digua_common_http::ResilientClient;`
- 加 `use api_gateway::resilience_metrics::GatewayMetricsObserver;`
- 把 Task 7 的 `Arc::new(NoopObserver)` 改为 `Arc::new(GatewayMetricsObserver)`。

- [ ] **Step 3: 编译 + observer 单测**

Run: `cargo test -p api-gateway resilience_metrics 2>&1 | tail -10`
Expected: `observer_records_without_panic` PASS。

- [ ] **Step 4: 写错误映射集成测试**

> 说明：`forward_to_service` 端到端需要真实 Consul 实例，故集成测试聚焦 `ResilientClient` 返回值（gateway 映射逻辑在 `router.rs`，类型受编译器保证、逻辑简单）；完整经 gateway 的四态由 `test-case/51` 手工验收覆盖。

`services/api-gateway/tests/resilience_test.rs` 整文件：
```rust
// 验证 ResilientClient 返回值与 gateway 映射的对应关系（决策 5A/12A）。
// Ok(resp) -> 透传; CircuitOpen->503; Timeout->504; Connect->502 (映射见 router.rs)。
use bytes::Bytes;
use digua_common_http::{NoopObserver, ResilienceConfig, ResilientClient, ResilientError};
use reqwest::{header::HeaderMap, Client, Method, StatusCode};
use std::sync::Arc;
use std::time::Duration;

async fn spawn(app: axum::Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    format!("http://{addr}")
}

fn fast_cfg() -> ResilienceConfig {
    ResilienceConfig {
        timeout_ms: 200,
        overall_timeout_ms: 600,
        retry_max_attempts: 2,
        retry_base_delay_ms: 5,
        retry_max_delay_ms: 20,
        retry_jitter: 0.0,
        breaker_failure_threshold: 100, // 不在本测试触发熔断
        breaker_open_timeout_ms: 1000,
        breaker_half_open_max_calls: 1,
    }
}

fn rc() -> ResilientClient {
    ResilientClient::new(Client::new(), fast_cfg(), Arc::new(NoopObserver))
}

#[tokio::test]
async fn retry_exhausted_5xx_passes_through_body() {
    let app = axum::Router::new()
        .route("/x", axum::routing::get(|| async { (StatusCode::SERVICE_UNAVAILABLE, "down") }));
    let base = spawn(app).await;
    let resp = rc()
        .call("svc", Method::GET, &format!("{base}/x"), &HeaderMap::new(), Bytes::new())
        .await
        .expect("ok upstream");
    assert_eq!(resp.status, StatusCode::SERVICE_UNAVAILABLE); // gateway 透传原 status+body
    assert_eq!(&resp.body[..], b"down");
}

#[tokio::test]
async fn connect_error_maps_to_502_class() {
    let err = rc()
        .call("svc", Method::GET, "http://127.0.0.1:1/none", &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("connect err");
    assert!(matches!(err, ResilientError::Connect(_))); // gateway -> 502
}

#[tokio::test]
async fn timeout_maps_to_504_class() {
    let app = axum::Router::new().route(
        "/slow",
        axum::routing::get(|| async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            "late"
        }),
    );
    let base = spawn(app).await;
    let err = rc()
        .call("svc", Method::GET, &format!("{base}/slow"), &HeaderMap::new(), Bytes::new())
        .await
        .expect_err("timeout");
    assert!(matches!(err, ResilientError::Timeout)); // gateway -> 504
}
```

- [ ] **Step 5: 运行集成测试**

Run: `cargo test -p api-gateway --test resilience_test 2>&1 | tail -20`
Expected: 三个测试 PASS。

- [ ] **Step 6: 全 gateway 测试 + clippy**

Run: `cargo test -p api-gateway 2>&1 | tail -12 && cargo clippy -p api-gateway 2>&1 | tail -10`
Expected: 全 PASS；clippy 无 error。

- [ ] **Step 7: Commit**

```bash
git add services/api-gateway/src/resilience_metrics.rs services/api-gateway/src/lib.rs services/api-gateway/src/main.rs services/api-gateway/tests/resilience_test.rs
git commit -m "feat(gateway): ResilienceObserver impl exposing per-service breaker metrics"
```

---

## Task 9: 手工验收脚本 `test-case/51-resilience-breaker.md`

**Files:**
- Create: `test-case/51-resilience-breaker.md`

**Interfaces:**
- Consumes: 运行中的 gateway + 一个下游服务（如 audit-service）+ Consul。
- Produces: 手工验收文档。

- [ ] **Step 1: 参考现有 test-case 风格**

Run: `sed -n '1,40p' test-case/44-permission-epoch.md`
Expected: 看到逐场景步骤表 + 前置条件格式，照此风格写。

- [ ] **Step 2: 写验收脚本**

`test-case/51-resilience-breaker.md` 整文件：
````markdown
# 51 — 韧性三件套（超时/重试/熔断）手工验收

> 关联：`docs/superpowers/specs/2026-06-28-phase5-resilience-triad-design.md`
> 前置：`docker compose` 起 gateway + 至少一个下游（以 audit-service 为例）+ Consul。
> 默认配置：timeout=3s, overall=8s, breaker_threshold=10, open_timeout=30s, half_open_max=3。

## 场景 1：熔断打开 → 短路 503

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | `docker compose stop audit-service`（令其拒连） | 下游不可用 |
| 2 | 连续 ≥10 次 `curl -i http://localhost:3000/api/v1/audit/health` | 前若干次 502/504，累积达阈值后转 503 |
| 3 | 再次 `curl -i .../audit/health` | **503** + body `{"error":"circuit_open","service":"audit-service"}`（短路，未打到下游） |
| 4 | `curl -s http://localhost:3000/metrics \| grep gateway_circuit_state` | `gateway_circuit_state{service="audit-service"} 2`（Open） |
| 5 | `curl -s .../metrics \| grep gateway_upstream_requests_total` | 含 `outcome="circuit_open"` 计数 > 0 |

## 场景 2：HalfOpen 探测 → 恢复 Closed

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 承场景 1（breaker 处于 Open） | — |
| 2 | `docker compose start audit-service`，等待其在 Consul 健康 | 下游恢复 |
| 3 | 等待 ≥30s（open_timeout） | breaker 允许进入 HalfOpen |
| 4 | `curl -i .../audit/health` 连续 3 次 | 探测成功，逐步恢复 |
| 5 | `curl -s .../metrics \| grep gateway_circuit_state` | 值回到 `0`（Closed） |

## 场景 3：超时映射 504

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 令下游对某路径响应 > 3s（临时在下游加 sleep handler，或用 tc 注入延迟） | 单次超时 |
| 2 | `curl -i .../<慢路径>` | **504** upstream timeout |
| 3 | 持续慢（每次 > 3s）| 总耗时 ≤ ~8s（overall deadline 封顶），仍 504 |

## 场景 4：幂等重试 vs POST 不重试

| 步骤 | 操作 | 期望 |
|------|------|------|
| 1 | 令下游对 GET 间歇性返回 502（前 1-2 次失败后成功） | — |
| 2 | `curl -i .../audit/operation`（GET）| 经重试后最终 2xx；`gateway_upstream_retries_total` 增加 |
| 3 | 对 POST 路径同样间歇 502 | POST **不重试**，首个 502 即透传 |
````

- [ ] **Step 3: Commit**

```bash
git add test-case/51-resilience-breaker.md
git commit -m "docs(test-case): add manual acceptance for resilience triad (breaker/timeout/retry)"
```

---

## Task 10: 收尾——TODO.md 勾选 + 全量验证

**Files:**
- Modify: `TODO.md`

- [ ] **Step 1: 勾选 Phase 5-B 完成项**

`TODO.md` 把 Phase 5「**B — 韧性三件套**」下三条主项 `- [ ]` 改为 `- [x]`（保留 `B-fu1` 未勾选），标题改为：
```markdown
- [ ] **B — 韧性三件套**（spec 已就绪，**核心已完成并合并**，仅剩 B-fu1）
  - [x] 在 `libs/digua_common-http` 提供通用 `ResilientClient`（包 reqwest）：超时 + 重试 + 熔断
  - [x] 本期接 gateway `forward_to_service`：每下游 service 独立 breaker，状态经 `ResilienceObserver` → Prometheus
  - [x] 删除并重写未使用的韧性脚手架（axum/benchmarks/rate_limit/middleware/service_discovery/circuit_breaker/retry/config）
```

- [ ] **Step 2: 全量编译 + 测试**

Run:
```bash
cargo build --workspace 2>&1 | tail -5
cargo test -p digua_common-http -p api-gateway 2>&1 | tail -15
```
Expected: 编译成功；所有相关测试 PASS。

- [ ] **Step 3: Commit**

```bash
git add TODO.md
git commit -m "docs(todo): mark Phase 5-B resilience triad core complete (B-fu1 deferred)"
```

---

## Self-Review

**Spec coverage（spec § → task）：**
- §4 架构（call 流程/owned 响应/服务发现边界）→ Task 6/7 ✓
- §5.1 CircuitBreaker（Open 计时修复 + 2A 占额）→ Task 2 ✓
- §5.2 RetryPolicy（rand + 6A 归属）→ Task 3 ✓
- §5.3 ResilientClient/ResilientError/UpstreamResponse（5A）→ Task 6 ✓
- §5.4 ResilienceConfig（含 overall_timeout）→ Task 4 ✓
- §6 gateway 接线 + 四态映射 + 9A 边界注释 → Task 7 ✓
- §7 ResilienceObserver + Prometheus（3A）→ Task 5 + Task 8 ✓
- §8.1 单测（含并发 10A、observer 13A）→ Task 2/3/5/6 ✓
- §8.2 集成测试（超时/deadline 11A/重试/短路/恢复/四态 12A）→ Task 6/8 ✓
- §8.3 手工验收 → Task 9 ✓
- §9 删除/重写清单 → Task 1（删）+ Task 2-6（重写）✓
- §11 落地顺序 9 步 → Task 1-9 对应 ✓
- §12 NOT in scope / §13 复用 / §14 失败模式 → 设计性章节，无需 task

**Placeholder scan：** 无 TBD/TODO/"later"；每个代码步骤含完整代码。

**Type consistency：**
- `ResilientError`（CircuitOpen/Timeout/Connect(reqwest::Error)）：Task 1 占位、Task 3 引用、Task 6 最终定义 —— 一致。
- `UpstreamResponse{status,headers,body}`：Task 6 定义、Task 7 消费 —— 一致。
- `Outcome` 五值（Success/Timeout/ConnectError/UpstreamError/CircuitOpen）：Task 1/5/8 —— 一致。
- `should_retry_err`/`should_retry_status`：Task 3 定义、Task 6 调用 —— 签名一致。
- `ResilientClient::new(inner, cfg, observer)` + `call(service, method, url, &headers, body)`：Task 6 定义、Task 7 调用 —— 一致。
- `ResilienceConfig` 九字段 + `timeout()/overall_timeout()/retry_policy()/new_breaker()`：Task 4 定义、Task 6/7 消费 —— 一致。
- `CircuitBreaker::new(threshold, open_timeout, half_open_max)` + `is_available/record_success/record_failure/state`：Task 2 定义、Task 4/6 消费 —— 一致。
- `ResilienceObserver`（on_state_change/on_outcome/on_retry）：Task 5 定义、Task 6 调用、Task 8 实现 —— 一致。

**潜在落地注意点（实现者留意，非阻塞）：**
- Task 4 `partial_json_fills_defaults` 依赖 `serde_json`（已在 digua_common-http deps）。
- Task 6/8 集成测试依赖 dev-dep `axum`（Task 1 已加），axum 0.8 `axum::serve` 签名以 workspace 版本为准。
- gateway observer 用默认 prometheus registry（与现有 `prometheus::gather()` 一致），多测试并发注册由 `OnceLock` 保证只注册一次。
