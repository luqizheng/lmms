# Phase 4: 性能压测与熔断限流

> 完成日期：2026-06-14

## 熔断器 (Circuit Breaker)

### 状态机

```
        ┌──────────────────────────────────────┐
        │                                      │
        ▼                                      │
    ┌───────┐    failure >= threshold    ┌─────┴───┐
    │Closed │ ──────────────────────────►│  Open   │
    └───┬───┘                             └────┬────┘
        │    success >= half_open_max           │
        ▲                                      │
        │    timeout elapsed                ┌────▼────┐
        └────────────────────────────────── │Half-Open│
                                           └─────────┘
```

### 默认配置

| 环境 | 阈值 | 超时 |
|------|------|------|
| 开发 | 3 | 30s |
| 生产 | 10 | 30s |

### 使用示例

```rust
use common_http::{CircuitBreaker, CircuitState};
use std::time::Duration;

let cb = CircuitBreaker::new(5, Duration::from_secs(60));

// 检查是否可用
if cb.is_available() {
    // 调用服务
    cb.record_success();
} else {
    // 返回降级响应
    cb.record_failure();
}

// 查看状态
match cb.state() {
    CircuitState::Closed => println!("正常"),
    CircuitState::Open => println!("熔断中"),
    CircuitState::HalfOpen => println!("半开"),
}
```

---

## 限流 (Rate Limiter)

### 令牌桶算法

- 容量：桶中最大令牌数
- 补充速率：每秒补充的令牌数

### 默认配置

| 环境 | 容量 | 速率 |
|------|------|------|
| 开发 | 50 | 20/s |
| 生产 | 1000 | 500/s |

### 使用示例

```rust
use common_http::RateLimiter;

let limiter = RateLimiter::new(100, 50); // 容量=100, 速率=50/s

if limiter.try_acquire() {
    // 处理请求
} else {
    // 返回 429 Too Many Requests
}
```

---

## Axum 中间件

### CircuitBreakerMiddleware

```rust
use axum::{
    Router,
    routing::get,
    middleware::from_fn_with_state,
};
use common_http::{CircuitBreakerMiddleware, HttpDefaultsConfig};

let config = HttpDefaultsConfig::production();
let cb_middleware = config.circuit_breaker();

let app = Router::new()
    .route("/api/data", get(handler))
    .layer(from_fn_with_state(
        Arc::new(cb_middleware),
        |state, request, next| async move {
            state.circuit_breaker().handle(request, next).await
        },
    ));
```

### RateLimitMiddleware

```rust
use common_http::{RateLimitMiddleware, HttpDefaultsConfig};

let config = HttpDefaultsConfig::production();
let rl_middleware = config.rate_limiter();

let app = Router::new()
    .route("/api/data", get(handler))
    .layer(from_fn_with_state(
        Arc::new(rl_middleware),
        |state, request, next| async move {
            state.rate_limiter().handle(request, next).await
        },
    ));
```

---

## 性能基准测试

### 运行测试

```bash
# 单元测试 + 基准
cargo test -p common-http -- --nocapture

# 压测脚本
node scripts/bench.js audit-service 60 10
```

### 预期指标

| 操作 | 目标 QPS | 延迟 P99 |
|------|----------|----------|
| CircuitBreaker::is_available | >10M | <100ns |
| CircuitBreaker::record_success | >1M | <1μs |
| RateLimiter::try_acquire | >5M | <200ns |

### 基准测试输出示例

```
CircuitBreaker::record_success:
  ops/s: 1,234,567.89
  ns/op: 0.81

RateLimiter::try_acquire:
  ops/s: 5,678,901.23
  ns/op: 0.18
```

---

## HTTP 客户端默认配置

```rust
use common_http::HttpDefaultsConfig;

let config = HttpDefaultsConfig::production();
// 自动启用:
// - CircuitBreaker: threshold=10, timeout=30s
// - RateLimiter: capacity=1000, rps=500
// - RetryPolicy: max_attempts=3, base_delay=200ms
```

### 环境配置

| 环境 | CircuitBreaker | RateLimiter | Retry |
|------|-----------------|-------------|-------|
| 开发 | 3 failures, 30s | 50 cap, 20/s | 2 attempts |
| 生产 | 10 failures, 30s | 1000 cap, 500/s | 3 attempts |

---

## 压测脚本使用

```bash
# 基本用法
node scripts/bench.js <service> [duration] [concurrency]

# 示例
node scripts/bench.js audit-service 60 10
node scripts/bench.js config-service 30 5

# 压测所有服务
pnpm bench:audit
pnpm bench:config
pnpm bench:file
```

### 输出格式

```
Starting load test:
  URL: http://localhost:8080
  Duration: 60s
  Concurrency: 10

=== Load Test Results ===
Duration:    60.00s
Total Req:   150000
RPS:         2500.00

Responses:
  Success:   149500
  Errors:    500
  Timeouts:  0

Latency (ms):
  Avg:       3.21
  P50:       2.50
  P90:       5.00
  P99:       10.00
  Max:       150.00
```

---

## CI 集成

### 性能回归检测

```yaml
# .github/workflows/perf-test.yml
name: Performance Tests

on:
  schedule:
    - cron: '0 6 * * *'  # 每日

jobs:
  perf-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run benchmarks
        run: cargo test -p common-http --bench
      - name: Run load test
        run: node scripts/bench.js audit-service 30 10
      - name: Check thresholds
        run: node scripts/check-perf.js
```
