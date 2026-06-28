# Phase 5（子项目 B）— 韧性三件套：超时 / 重试 / 熔断

> 日期：2026-06-28
> 来源：TODO「Phase 5 — 服务间安全 & 韧性（P1）」中的「韧性三件套」条目。
> Phase 5 拆为两个独立子项目：**A = mTLS / 服务网格**、**B = 韧性三件套**。本 spec 仅覆盖 **B**，A 后续单独 spec。

## 1. 背景与问题

当前 api-gateway 的 `forward_to_service`（`services/api-gateway/src/router.rs:16`）直接用裸
`reqwest::Client` 转发到 Consul 发现的下游服务，**没有任何韧性保护**：

- 下游慢 → gateway 请求按 reqwest 默认行为挂着，无统一可配超时。
- 下游瞬时抖动（连接重置 / 502 / 503）→ 无重试。
- 下游持续故障 → gateway 仍不断打过去，无熔断短路，故障扩散、资源浪费。

同时 `libs/common-http` 里残留一批 **2026-06-14 留下的孤儿脚手架**
（`circuit_breaker.rs` / `retry.rs` / `rate_limit.rs` / `axum.rs` / `benchmarks.rs`，
对应旧 spec `phase4-perf-circuit-limit.md`）：

- **没有任何 service import 使用**（`grep` 确认仅 `common-http` 内部互引）。
- `CircuitBreaker::is_available()` 的 Open 态计时逻辑有 bug：
  `Instant::now().checked_duration_since(Instant::now() - Duration::from_secs(last))`
  —— `last` 存的是 `Instant::now().elapsed()`（≈0），整个计时不成立。
- `RetryPolicy` 用字符串 `error.contains("timeout")` 判可重试，脆弱；随机源用
  `RandomState` hasher 充当 jitter，劣质且不可控。
- 形态是独立 struct + `from_fn` 中间件，与 TODO 期望的「outbound 韧性」用途不符。

## 2. 目标 / 非目标

### 目标
- 在 `libs/common-http` 提供一个通用 **`ResilientClient`**（包 `reqwest::Client`），统一封装
  超时 / 重试 / 熔断三件套。
- 本期把 **api-gateway 的 `forward_to_service`** 接到 `ResilientClient`，**每个下游 service
  一个独立熔断器**，状态通过 Prometheus 暴露到 gateway 现有 `/metrics`。
- 删除并重写 `common-http` 里未被使用的韧性脚手架，消除双实现与死代码。

### 非目标（YAGNI）
- **不**接入其他服务间直调（token→user 等），后续迭代再扩面。
- **不**做限流（gateway 已有 `governor` 实现，notification 用 `common_ratelimit`；本期 `rate_limit.rs` 删除不替换）。
- **不**动 mTLS / `require_service_token`（属 Phase 5 子项目 A）。
- **不**改 reqwest 为 tower::Service 栈（见 §6 决策记录）。

## 3. 决策摘要（brainstorm 结论）

| 决策点 | 选择 |
|--------|------|
| 注入点 | 通用库 `common-http`，但**本期仅接 gateway**；其他服务后续迭代 |
| 技术形态 | `ResilientClient` 包 `reqwest`，韧性作为显式调用逻辑（不硬碰 tower::Layer） |
| 熔断粒度 | 按 `service_name` 一个 breaker，懒创建于 `DashMap` |
| 状态暴露 | Prometheus gauge/counter，写入 gateway 现有 `/metrics` |
| 重试幂等性 | **仅幂等方法**（GET/HEAD/PUT/DELETE/OPTIONS）+ 可重试错误重试；POST 不重试 |
| 旧脚手架 | **全删重写** |

## 4. 架构

```
gateway forward_to_service(svc, req)
   │  (parts.method, target_url, parts.headers, body: Bytes)
   ▼
ResilientClient (libs/common-http)            ── 每 gateway 实例一个
   ├─ inner: reqwest::Client                  ── 复用现有连接池（保留 reqwest 自身 timeout 兜底）
   ├─ breakers: DashMap<String, Arc<CircuitBreaker>>   ── 按 service_name 懒创建
   ├─ cfg: ResilienceConfig (timeout / retry / breaker 参数)
   └─ metrics: ResilienceMetrics (Prometheus handles)
   │
   ▼  call(service_name, method, url, headers, body: Bytes) -> Result<reqwest::Response, ResilientError>
   1. breaker = breakers.entry(service_name).or_insert_with(...)
   2. breaker.is_available()? 否 → 立即返回 Err(CircuitOpen)（不发请求）
   3. 单次尝试：tokio::time::timeout(cfg.timeout, inner.execute(rebuilt_request))
   4. 失败 且 is_retriable(err, method) 且未达 max_attempts
        → RetryPolicy 退避 sleep 后 goto 2（重试同样过 breaker.is_available + timeout）
   5. 成功 → breaker.record_success()；失败/超时 → breaker.record_failure()
   6. 状态变化 / 每次 outcome → 更新 Prometheus
```

**关键：body 处理。** gateway 在 `router.rs:33` 已把请求体读成 `bytes::Bytes`。
`ResilientClient::call` 接收 `Bytes`（而非已构建的 `reqwest::Request`），每次（重试）尝试用
`Bytes`（廉价 `clone`）经 `RequestBuilder` 重建请求，**天然规避 reqwest Request body 流被消费、无法重发的问题**。

## 5. 组件细节

### 5.1 `CircuitBreaker`（`libs/common-http/src/circuit_breaker.rs`，重写）

- 状态：单个 `AtomicU8`（0=Closed,1=Open,2=HalfOpen）。
- 计数：`failure_count` / `success_count`（`AtomicU64`）、`half_open_calls`（`AtomicU64`）。
- **Open 计时修复**：进入 Open 时记录时刻。为避免 `Instant` 不可原子存储，存
  `opened_at_ms: AtomicU64`（`Instant`→进程内单调毫秒，基于一个进程启动锚点 `Instant`）。
  `is_available()` 在 Open 态：`now_ms - opened_at_ms >= open_timeout_ms` → 转 HalfOpen 并放行。
- 配置字段：`failure_threshold: u64`、`open_timeout: Duration`、`half_open_max_calls: u32`。
- 状态机：
  - Closed：`record_failure` 累加，达 `failure_threshold` → Open（记 `opened_at`）。`record_success` 清零失败计数。
  - Open：`is_available` 计时到 → HalfOpen（清 `half_open_calls`/`success_count`）。
  - HalfOpen：放行至多 `half_open_max_calls` 次探测；`record_success` 累计达额 → Closed；任一 `record_failure` → 回 Open。
- 公开 `state() -> CircuitState` 供 metrics 采样。
- 单测覆盖：默认 Closed；达阈值 Open；Open 超时后 HalfOpen；HalfOpen 成功满额 → Closed；HalfOpen 失败 → Open。

### 5.2 `RetryPolicy`（`libs/common-http/src/retry.rs`，重写精简）

- 字段：`max_attempts: u32`、`base_delay: Duration`、`max_delay: Duration`、`jitter: f64`。
- `delay_for(attempt)`：指数退避 `base_delay * 2^(attempt-1)`，`min(max_delay)`，加 `±jitter` 抖动。
- jitter 随机源改用 workspace 已有的 `rand = "0.8"`（替换旧 `RandomState` hasher）。
- **不再做字符串错误匹配**；可重试判定上移到 `ResilientClient::is_retriable`（按 `ResilientError` 枚举 + 方法幂等性）。
- 单测：退避计算（含 cap）、jitter 在界内。

### 5.3 `ResilientClient` + `ResilientError`（`libs/common-http/src/resilient.rs`，新增）

```rust
pub enum ResilientError {
    CircuitOpen,                  // 短路，未发请求
    Timeout,                      // tokio timeout 触发
    Connect(reqwest::Error),      // 连接层错误（请求未送达，幂等可重试）
    Upstream(reqwest::Response),  // 收到响应但 5xx/429
}
```

- `is_idempotent(&Method) -> bool`：GET/HEAD/PUT/DELETE/OPTIONS = true。
- `is_retriable(err, method) -> bool`：
  - `CircuitOpen` → false（不重试，直接短路）。
  - `Timeout` / `Connect` → 仅幂等方法 true。
  - `Upstream(resp)`：status ∈ {502,503,429} 且 幂等方法 → true；否则 false。
- `call(service_name, method, url, headers, body: Bytes)`：实现 §4 流程 1–6。
- breaker 懒创建：`breakers.entry(service_name.to_string()).or_insert_with(|| Arc::new(CircuitBreaker::new(cfg)))`。

### 5.4 配置 `ResilienceConfig`（`libs/common-http/src/config.rs`，重写）

| 字段 | 默认 |
|------|------|
| `timeout_ms` | 3000 |
| `retry_max_attempts` | 3 |
| `retry_base_delay_ms` | 200 |
| `retry_max_delay_ms` | 10000 |
| `retry_jitter` | 0.1 |
| `breaker_failure_threshold` | 10 |
| `breaker_open_timeout_ms` | 30000 |
| `breaker_half_open_max_calls` | 3 |

gateway 配置（`GatewayConfig`）新增可选 `resilience:` 段映射到此；缺省走默认值。

## 6. gateway 接线

- `RouterConfig`（`router.rs:10`）：`http_client: Client` → `resilient: Arc<ResilientClient>`。
- `forward_to_service`（`router.rs:16`，第 38–51 行）：裸 reqwest 调用改为
  `cfg.resilient.call(service_name, parts.method, &target_url, &parts.headers, bytes).await`，
  按 `ResilientError` 映射：
  - `CircuitOpen` → 503 `{"error":"circuit_open","service":<svc>}`
  - `Timeout` → 504
  - `Connect` → 502
  - `Upstream(resp)` → 透传原响应（保持现有 status/headers/body 透传行为）
- `RouterConfig` 构造点（`main.rs` / `lib.rs`）：从 `GatewayConfig.resilience` 构建 `ResilientClient`。

## 7. 可观测性（Prometheus，gateway 现有 `/metrics`）

- `gateway_circuit_state{service}` gauge：0=Closed,1=HalfOpen,2=Open（状态切换时 set）。
- `gateway_upstream_requests_total{service,outcome}` counter：
  outcome ∈ `success` / `timeout` / `connect_error` / `upstream_error` / `circuit_open`。
- `gateway_upstream_retries_total{service}` counter。
- metrics handle 由 gateway 注册并注入 `ResilientClient`（库不强绑 prometheus；通过回调 / trait 或简单 handle 结构注入，保持 `common-http` 对 prometheus 的依赖可选）。

## 8. 测试与验收

### 8.1 单元测试（`common-http`）
- `CircuitBreaker` 状态机全路径（见 §5.1）。
- `RetryPolicy` 退避计算 + jitter 边界。
- `is_retriable` 矩阵：方法（幂等/非幂等）× 错误类型（CircuitOpen/Timeout/Connect/Upstream{502,503,429,500,400}）。

### 8.2 集成测试（gateway 或 common-http）
项目无 `wiremock`；用本地 **axum mock server**（gateway dev-deps 已有 axum/tower/hyper/tokio-test）起假下游，覆盖：
- 慢响应 → 超时触发，返回 504。
- 502 → 幂等方法重试，非幂等（POST）不重试。
- 连续失败达阈值 → breaker Open → 后续请求短路（不打到 mock）→ 503。
- Open 超时后 → HalfOpen 探测成功 → Closed 恢复。

### 8.3 手工验收
新增 `test-case/51-resilience-breaker.md`（沿用 41–44 的逐场景步骤表风格）：
停掉某下游 → 经 gateway 访问触发失败累积 → 观察 503 `circuit_open` + `/metrics`
`gateway_circuit_state{service=...} 2` → 恢复下游 → HalfOpen → Closed。

## 9. 删除 / 重写清单

| 文件 | 处置 |
|------|------|
| `libs/common-http/src/axum.rs` | 删（中间件形态本期不用） |
| `libs/common-http/src/benchmarks.rs` | 删 |
| `libs/common-http/src/rate_limit.rs` | 删（限流另有实现，无人用） |
| `libs/common-http/src/middleware.rs` | 查引用后删（疑似同批孤儿） |
| `libs/common-http/src/service_discovery.rs`（`ConsulResolver`） | 删（grep 确认无外部引用） |
| `libs/common-http/src/circuit_breaker.rs` | 重写（修 bug） |
| `libs/common-http/src/retry.rs` | 重写（rand + 枚举判定） |
| `libs/common-http/src/config.rs` | 重写为 `ResilienceConfig` |
| `libs/common-http/src/resilient.rs` | 新增 |
| `libs/common-http/src/lib.rs` | 更新导出表 |
| `libs/common-http/Cargo.toml` | 加 `dashmap`、`rand`、可选 `prometheus`；移除 `axum`/`tower` 可选 feature（若不再用） |
| `docs/superpowers/specs/phase4-perf-circuit-limit.md` | 顶部标注 **superseded by 本 spec** |

> 删除前对每个文件 `grep` 确认无引用（`middleware.rs`、`service_discovery.rs` 重点核实）。

## 10. 风险与缓解

- **重试放大故障**：仅幂等方法 + 可重试错误 + breaker 串联（Open 时重试也被短路），限定 `max_attempts`。
- **body clone 成本**：用 `Bytes`（引用计数，clone 廉价）；gateway 已读全量 body，无额外内存形态变化。
- **breaker 跨实例不共享**：每 gateway 实例独立 breaker（进程内）。多副本场景下属可接受——熔断是本地保护，非全局一致状态；明确记录为已知限制。
- **`common-http` 对 prometheus 依赖**：通过注入 handle / 可选 feature 避免库强绑监控栈。

## 11. 落地顺序（供 writing-plans 细化）

1. `common-http` 删孤儿 + 重写 `CircuitBreaker` / `RetryPolicy` / `ResilienceConfig` + 单测。
2. 新增 `ResilientClient` + `ResilientError` + `is_retriable` 矩阵单测。
3. metrics handle 结构 + 注入接口。
4. gateway `RouterConfig` / `forward_to_service` 接线 + 错误映射。
5. gateway metrics 注册 + 配置段。
6. 集成测试（axum mock 下游）。
7. `test-case/51-resilience-breaker.md` + 旧 spec 标注 superseded。
