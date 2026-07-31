# Phase 5（子项目 B）— 韧性三件套：超时 / 重试 / 熔断

> 日期：2026-06-28
> 来源：TODO「Phase 5 — 服务间安全 & 韧性（P1）」中的「韧性三件套」条目。
> Phase 5 拆为两个独立子项目：**A = mTLS / 服务网格**、**B = 韧性三件套**。本 spec 仅覆盖 **B**，A 后续单独 spec。
>
> **Plan-review 结论已 bake（2026-06-28，BIG CHANGE 模式）：**
> 1A 进程内 breaker（不复用 Redis）· 2A HalfOpen 原子占额 · 3A `ResilienceObserver` trait（含 no-op 默认）·
> 4A 单次 timeout + overall deadline · 5A `call` 返回 owned `UpstreamResponse`，错误枚举仅传输层 ·
> 6A 重试策略归 `RetryPolicy` · 8C deadline 用栈上 `Instant`、breaker 用毫秒锚点 · 9A 服务发现在韧性边界外 ·
> 15A breaker 查找读锁优先 · 16A headers/body 入口准备一次。
> 测试缺口闭合：10A 并发占额单测 · 11A overall deadline 集成测试 · 12A 四态错误映射集成测试 ·
> 13A observer 回调单测 · 14B 服务发现边界靠结构保证（不加防御性测试）。

## 1. 背景与问题

当前 api-gateway 的 `forward_to_service`（`services/api-gateway/src/router.rs:16`）直接用裸
`reqwest::Client` 转发到 Consul 发现的下游服务，**没有任何韧性保护**：

- 下游慢 → gateway 请求按 reqwest 默认行为挂着，无统一可配超时。
- 下游瞬时抖动（连接重置 / 502 / 503）→ 无重试。
- 下游持续故障 → gateway 仍不断打过去，无熔断短路，故障扩散、资源浪费。

同时 `libs/digua_common-http` 里残留一批 **2026-06-14 留下的孤儿脚手架**
（`circuit_breaker.rs` / `retry.rs` / `rate_limit.rs` / `axum.rs` / `benchmarks.rs`，
对应旧 spec `phase4-perf-circuit-limit.md`）：

- **没有任何 service import 使用**（`grep` 确认仅 `digua_common-http` 内部互引）。
- `CircuitBreaker::is_available()` 的 Open 态计时逻辑有 bug：
  `Instant::now().checked_duration_since(Instant::now() - Duration::from_secs(last))`
  —— `last` 存的是 `Instant::now().elapsed()`（≈0），整个计时不成立。
- `RetryPolicy` 用字符串 `error.contains("timeout")` 判可重试，脆弱；随机源用
  `RandomState` hasher 充当 jitter，劣质且不可控。
- 形态是独立 struct + `from_fn` 中间件，与 TODO 期望的「outbound 韧性」用途不符。

## 2. 目标 / 非目标

### 目标
- 在 `libs/digua_common-http` 提供一个通用 **`ResilientClient`**（包 `reqwest::Client`），统一封装
  超时 / 重试 / 熔断三件套。
- 本期把 **api-gateway 的 `forward_to_service`** 接到 `ResilientClient`，**每个下游 service
  一个独立熔断器**，状态通过 Prometheus 暴露到 gateway 现有 `/metrics`。
- 删除并重写 `digua_common-http` 里未被使用的韧性脚手架，消除双实现与死代码。

### 非目标（YAGNI）
- **不**接入其他服务间直调（token→user 等），后续迭代再扩面。
- **不**做限流（gateway 已有 `governor` 实现，notification 用 `digua_common_ratelimit`；本期 `rate_limit.rs` 删除不替换）。
- **不**动 mTLS / `require_service_token`（属 Phase 5 子项目 A）。
- **不**改 reqwest 为 tower::Service 栈（见 §6 决策记录）。

## 3. 决策摘要（brainstorm 结论）

| 决策点 | 选择 |
|--------|------|
| 注入点 | 通用库 `digua_common-http`，但**本期仅接 gateway**；其他服务后续迭代 |
| 技术形态 | `ResilientClient` 包 `reqwest`，韧性作为显式调用逻辑（不硬碰 tower::Layer） |
| 熔断粒度 | 按 `service_name` 一个 breaker，懒创建于 `DashMap` |
| 状态暴露 | Prometheus gauge/counter，写入 gateway 现有 `/metrics` |
| 重试幂等性 | **仅幂等方法**（GET/HEAD/PUT/DELETE/OPTIONS）+ 可重试错误重试；POST 不重试 |
| 旧脚手架 | **全删重写** |

## 4. 架构

```
gateway forward_to_service(svc, req)
   │  服务发现(Consul discover) 已在此之前完成 ── 韧性边界之外，失败不喂 breaker（决策 9A）
   │  (parts.method, target_url, &parts.headers, body: Bytes)
   ▼
ResilientClient (libs/digua_common-http)            ── 每 gateway 实例一个（进程内，决策 1A）
   ├─ inner: reqwest::Client                  ── 复用现有连接池（保留 reqwest 自身 timeout 兜底）
   ├─ breakers: DashMap<String, Arc<CircuitBreaker>>   ── 按 service_name 懒创建（读锁优先，决策 15A）
   ├─ cfg: ResilienceConfig (timeout / overall_timeout / retry / breaker 参数)
   └─ observer: Arc<dyn ResilienceObserver>   ── 库定义 trait，gateway 实现写 Prometheus（决策 3A）
   │
   ▼  call(service_name, method, url, &headers, body: Bytes)
   │     -> Result<UpstreamResponse{status, headers, body: Bytes}, ResilientError>   (决策 5A)
   0. deadline = Instant::now()（栈上局部，决策 8C/4A）；headers/body 入口准备一次复用（决策 16A）
   1. breaker = breakers.get(svc)（读锁命中直接 clone Arc）否则 entry 写锁创建（决策 15A）
   2. deadline 已超 overall_timeout? → 立即返回 Err(Timeout)→504，不再尝试（决策 4A）
   3. breaker.is_available()? 否 → Err(CircuitOpen)（不发请求）；HalfOpen 态用 fetch_add 原子占额（决策 2A）
   4. 单次尝试：tokio::time::timeout(cfg.timeout, inner.execute(rebuild(&headers, body.clone())))
        → Ok(resp) 立即读成 owned UpstreamResponse{status,headers,body:Bytes}（决策 5A）
        → Elapsed → Err(Timeout)；reqwest 传输错误 → Err(Connect)
   5. RetryPolicy::should_retry(attempt, &err-or-status, &method)?（策略归 RetryPolicy，决策 6A）
        是 且未达 max_attempts 且 deadline 未超 → delay_for(attempt) sleep 后 goto 2
   6. 成功 → breaker.record_success()；传输失败/超时 → breaker.record_failure()
        （收到 5xx 响应视为「拿到响应」，是否计失败见 §5.3）
   7. observer.on_outcome / on_state_change / on_retry 回调（决策 3A）
```

**关键 1：body / headers 处理（决策 16A）。** gateway 在 `router.rs:33` 已把请求体读成 `bytes::Bytes`。
`ResilientClient::call` 接收 `&HeaderMap` + `Bytes`（而非已构建的 `reqwest::Request`），在入口准备一次；
每次（重试）尝试用 `Bytes`（O(1) 引用计数 clone）+ `RequestBuilder::headers(&headers)`（接受引用，无需每轮全量 clone）
重建请求，**天然规避 reqwest Request body 流被消费、无法重发的问题**。

**关键 2：owned 响应（决策 5A）。** 收到任何 HTTP 响应（含 5xx）即在 `call` 内读成 owned
`UpstreamResponse{status, headers, body: Bytes}`，彻底消除「Response body 流读一次即消费」与重试/透传的所有权冲突。
`ResilientError` 仅表达**没拿到响应**的传输层失败（见 §5.3）。

**关键 3：服务发现边界（决策 9A）。** Consul `discover` 在 `forward_to_service` 内、`call` 之前完成，
**不进入 `ResilientClient`**。breaker 信号语义纯净——只反映「下游实例健康」，不被 Consul 故障污染。
该边界靠代码结构保证（discover 不在 call 内），不加防御性测试（决策 14B），在 `forward_to_service` 加一行注释固化。

## 5. 组件细节

### 5.1 `CircuitBreaker`（`libs/digua_common-http/src/circuit_breaker.rs`，重写）

- 状态：单个 `AtomicU8`（0=Closed,1=Open,2=HalfOpen）。
- 计数：`failure_count` / `success_count`（`AtomicU64`）、`half_open_calls`（`AtomicU64`）。
- **Open 计时修复**：进入 Open 时记录时刻。为避免 `Instant` 不可原子存储，存
  `opened_at_ms: AtomicU64`（`Instant`→进程内单调毫秒，基于 `CircuitBreaker` 自身的进程级 `OnceLock<Instant>` 锚点；
  与 §4 的 overall deadline 互不耦合——deadline 用 call 栈上 `Instant`，决策 8C）。
  `is_available()` 在 Open 态：`now_ms - opened_at_ms >= open_timeout_ms` → 转 HalfOpen 并放行。
- 配置字段：`failure_threshold: u64`、`open_timeout: Duration`、`half_open_max_calls: u32`。
- 状态机：
  - Closed：`record_failure` 累加，达 `failure_threshold` → Open（记 `opened_at`）。`record_success` 清零失败计数。
  - Open：`is_available` 计时到 → HalfOpen（清 `half_open_calls`/`success_count`）。
  - HalfOpen：放行至多 `half_open_max_calls` 次探测；`record_success` 累计达额 → Closed；任一 `record_failure` → 回 Open。
- **HalfOpen 并发占额（决策 2A）**：`is_available()` 进入 HalfOpen 分支时用 `half_open_calls.fetch_add(1)`
  原子占一个探测名额，占后若超过 `half_open_max_calls` 则不放行——消除多并发 worker 同时读计数导致的
  TOCTOU 超额探测竞态（旧实现 `load` 后比较有此 bug）。
- 公开 `state() -> CircuitState` 供 metrics 采样。
- 单测覆盖：默认 Closed；达阈值 Open；Open 超时后 HalfOpen；HalfOpen 成功满额 → Closed；HalfOpen 失败 → Open；
  **并发占额（决策 10A）：spawn ~100 个 task 在 HalfOpen 态同时 `is_available`，断言放行数 == `half_open_max_calls`**。

### 5.2 `RetryPolicy`（`libs/digua_common-http/src/retry.rs`，重写精简）

- 字段：`max_attempts: u32`、`base_delay: Duration`、`max_delay: Duration`、`jitter: f64`。
- `delay_for(attempt)`：指数退避 `base_delay * 2^(attempt-1)`，`min(max_delay)`，加 `±jitter` 抖动。
- jitter 随机源改用 workspace 已有的 `rand = "0.8"`（替换旧 `RandomState` hasher）。
- **重试策略归属（决策 6A）**：`RetryPolicy::should_retry(attempt, &ResilientError-or-status, &Method) -> bool`
  持有完整重试决策（次数 + 幂等 + 错误类型）；`is_idempotent(&Method)` 作为无状态 pure fn 供其内部调用。
  `ResilientClient::call` 只驱动「问策略→执行→记录」循环，不散落判定逻辑。
- **不再做字符串错误匹配**（旧 `error.contains("timeout")` 删除），改按错误枚举 / status 判定。
- 单测：退避计算（含 cap）、jitter 在界内、`should_retry` 矩阵（方法 × 错误类型 × status）。

### 5.3 `ResilientClient` + `ResilientError`（`libs/digua_common-http/src/resilient.rs`，新增）

```rust
// 决策 5A：收到任何 HTTP 响应（含 5xx）都读成 owned 结构，错误枚举只表达「没拿到响应」
pub struct UpstreamResponse {
    pub status: reqwest::StatusCode,
    pub headers: reqwest::header::HeaderMap,
    pub body: bytes::Bytes,
}

pub enum ResilientError {
    CircuitOpen,             // 短路，未发请求
    Timeout,                 // tokio timeout 触发 / overall deadline 超限
    Connect(reqwest::Error), // 连接层错误（请求未送达）
}
// 注意：5xx/429 不再是 Err —— 它们是成功拿到的响应，作为 Ok(UpstreamResponse) 返回，
// 由 RetryPolicy::should_retry 按 status 决定是否重试；重试耗尽后原样透传（决策 5A/6A）。
```

- `call(service_name, method, url, &headers, body: Bytes) -> Result<UpstreamResponse, ResilientError>`：实现 §4 流程 0–7。
- 重试与幂等判定全部由 `RetryPolicy::should_retry(attempt, &ResilientError-or-status, &Method)` 承担（决策 6A）：
  - `CircuitOpen` → 不重试。
  - `Timeout` / `Connect` → 仅幂等方法重试。
  - 收到响应 status ∈ {502,503,429} 且幂等方法 → 重试；其余 status（含 2xx/4xx/500）→ 不重试。
- **breaker 计失败口径**：`Timeout`/`Connect` 计 `record_failure`；status ∈ {502,503} 计失败（下游不健康）；
  2xx/4xx 计 `record_success`（4xx 是请求问题，非下游故障）。429 计失败（下游过载）。
- breaker 查找（决策 15A）：先 `breakers.get(svc)`（读锁）命中即 `clone` Arc；未命中才 `entry().or_insert_with(...)`（写锁）。

### 5.4 配置 `ResilienceConfig`（`libs/digua_common-http/src/config.rs`，重写）

| 字段 | 默认 | 说明 |
|------|------|------|
| `timeout_ms` | 3000 | 单次尝试超时 |
| `overall_timeout_ms` | 8000 | 整个 call（含重试+退避）预算上限，留 ~2s 余量给 gateway+前端 10s 死线（决策 4A） |
| `retry_max_attempts` | 3 | |
| `retry_base_delay_ms` | 200 | |
| `retry_max_delay_ms` | 10000 | |
| `retry_jitter` | 0.1 | |
| `breaker_failure_threshold` | 10 | |
| `breaker_open_timeout_ms` | 30000 | |
| `breaker_half_open_max_calls` | 3 | |

gateway 配置（`GatewayConfig`）新增可选 `resilience:` 段映射到此；缺省走默认值。

## 6. gateway 接线

- `RouterConfig`（`router.rs:10`）：`http_client: Client` → `resilient: Arc<ResilientClient>`。
- `forward_to_service`（`router.rs:16`，第 38–51 行）：裸 reqwest 调用改为
  `cfg.resilient.call(service_name, parts.method, &target_url, &parts.headers, bytes).await`，
  按返回值映射（决策 5A）：
  - `Ok(UpstreamResponse)` → 透传 status/headers/body（含重试耗尽的 5xx——保持现有透传行为）
  - `Err(CircuitOpen)` → 503 `{"error":"circuit_open","service":<svc>}`
  - `Err(Timeout)` → 504
  - `Err(Connect)` → 502
- **服务发现边界（决策 9A/14B）**：`discover` 仍在 `forward_to_service` 内、`call` 之前；其失败走 gateway 既有
  `status.into_response()`，**不进 `ResilientClient`、不喂 breaker**。在此处加一行注释固化该边界。
- `RouterConfig` 构造点（`main.rs` / `lib.rs`）：从 `GatewayConfig.resilience` 构建 `ResilientClient`，
  注入 gateway 实现的 `ResilienceObserver`。

## 7. 可观测性（决策 3A：`ResilienceObserver` trait + Prometheus）

库定义 trait，gateway 实现写 Prometheus，`digua_common-http` 不强绑监控栈：

```rust
pub trait ResilienceObserver: Send + Sync {
    fn on_state_change(&self, service: &str, state: CircuitState);
    fn on_outcome(&self, service: &str, outcome: Outcome);  // success/timeout/connect_error/upstream_error/circuit_open
    fn on_retry(&self, service: &str);
}
// 库提供 no-op 默认实现 NoopObserver，供不关心 metrics 的调用方零成本接入。
```

gateway 侧 impl 写入现有 `/metrics`：
- `gateway_circuit_state{service}` gauge：0=Closed,1=HalfOpen,2=Open（`on_state_change` 触发 set）。
- `gateway_upstream_requests_total{service,outcome}` counter（`on_outcome`）。
- `gateway_upstream_retries_total{service}` counter（`on_retry`）。

## 8. 测试与验收

### 8.1 单元测试（`digua_common-http`）
- `CircuitBreaker` 状态机全路径（见 §5.1）：默认 Closed、达阈值 Open、Open 超时→HalfOpen、HalfOpen 满额→Closed、HalfOpen 失败→Open。
- **并发占额（决策 10A）**：spawn ~100 task 在 HalfOpen 态并发 `is_available`，断言放行数 == `half_open_max_calls`。
- `RetryPolicy` 退避计算（含 cap）+ jitter 边界。
- `should_retry` 矩阵：方法（幂等/非幂等）× 错误类型（CircuitOpen/Timeout/Connect）× status（502/503/429/500/400/200）。
- **observer 回调（决策 13A）**：库内 mock `ResilienceObserver`（记录调用到 Vec），断言成功→`on_outcome(success)`、
  失败累积→`on_state_change(Open)`、重试→`on_retry` 在对应路径被触发。

### 8.2 集成测试（gateway 或 digua_common-http，用本地 axum mock 下游）
项目无 `wiremock`；用本地 **axum mock server**（gateway dev-deps 已有 axum/tower/hyper/tokio-test）起假下游，覆盖：
- 慢响应 → 单次超时触发，返回 504。
- **overall deadline（决策 11A）**：持续慢下游（每次都超单次 timeout）→ 断言总耗时 ≤ `overall_timeout`+容差、
  尝试次数因 deadline 提前截断（< max_attempts）、返回 504。
- 502 → 幂等方法重试，非幂等（POST）不重试。
- 连续失败达阈值 → breaker Open → 后续请求短路（不打到 mock）→ 503。
- Open 超时后 → HalfOpen 探测成功 → Closed 恢复。
- **四态错误映射（决策 12A，含 5xx 透传 5A）**：持续 503（重试耗尽）→ 透传原 status+body；连接拒绝 → 502；
  持续超时 → 504；breaker Open → 503 circuit_open JSON。
- 服务发现失败不喂 breaker（决策 14B）：**不加专门测试**，靠 `discover` 在 `call` 外的代码结构保证。

### 8.3 手工验收
新增 `test-case/51-resilience-breaker.md`（沿用 41–44 的逐场景步骤表风格）：
停掉某下游 → 经 gateway 访问触发失败累积 → 观察 503 `circuit_open` + `/metrics`
`gateway_circuit_state{service=...} 2` → 恢复下游 → HalfOpen → Closed。

## 9. 删除 / 重写清单

| 文件 | 处置 |
|------|------|
| `libs/digua_common-http/src/axum.rs` | 删（中间件形态本期不用） |
| `libs/digua_common-http/src/benchmarks.rs` | 删 |
| `libs/digua_common-http/src/rate_limit.rs` | 删（限流另有实现，无人用；删时同步移除 `lib.rs` 的 `pub use`，决策 Issue 7） |
| `libs/digua_common-http/src/middleware.rs` | 查引用后删（疑似同批孤儿） |
| `libs/digua_common-http/src/service_discovery.rs`（`ConsulResolver`） | 删（grep 确认无外部引用） |
| `libs/digua_common-http/src/circuit_breaker.rs` | 重写（修 Open 计时 bug + HalfOpen 原子占额 2A） |
| `libs/digua_common-http/src/retry.rs` | 重写（rand + `should_retry` 策略归属 6A） |
| `libs/digua_common-http/src/config.rs` | 重写为 `ResilienceConfig`（含 `overall_timeout_ms`） |
| `libs/digua_common-http/src/resilient.rs` | 新增（`ResilientClient` / `ResilientError` / `UpstreamResponse` / `ResilienceObserver`） |
| `libs/digua_common-http/src/lib.rs` | 更新导出表 |
| `libs/digua_common-http/Cargo.toml` | 加 `dashmap`、`rand`、`bytes`；`prometheus` 不进库（observer trait 解耦）；移除 `axum`/`tower` 可选 feature（若不再用） |
| `docs/superpowers/specs/phase4-perf-circuit-limit.md` | 顶部标注 **superseded by 本 spec** |

> 删除前对每个文件 `grep` 确认无引用（`middleware.rs`、`service_discovery.rs` 重点核实）；删后 `cargo build -p digua_common-http` 验证无残留引用。

## 10. 风险与缓解

- **重试放大故障**：仅幂等方法 + 可重试错误 + breaker 串联（Open 时重试也被短路）+ overall deadline 封顶，限定 `max_attempts`。
- **body clone 成本**：用 `Bytes`（引用计数，clone O(1)）；headers 入口准备一次复用（决策 16A）；gateway 已读全量 body，无额外内存形态变化。
- **breaker 跨实例不共享（决策 1A）**：每 gateway 实例独立 breaker（进程内）。**已评估复用 gateway 现有 Redis（Phase 4.4 perm_epoch 已接入），
  结论：不值得**——熔断是本地资源保护语义（业界默认进程内），把 Redis 拖进每个 outbound 热路径的延迟成本 > 全局一致收益，且引入新 SPOF。明确记录为已知限制。
- **`digua_common-http` 不绑 prometheus（决策 3A）**：通过 `ResilienceObserver` trait 注入，库零监控栈依赖，提供 `NoopObserver` 默认实现。

## 11. 落地顺序（供 writing-plans 细化）

1. `digua_common-http` 删孤儿（axum/benchmarks/rate_limit/middleware/service_discovery，grep+build 验证）。
2. 重写 `CircuitBreaker`（修计时 bug + HalfOpen 原子占额 2A）+ 单测（含并发占额 10A）。
3. 重写 `RetryPolicy`（rand + `should_retry` 策略归属 6A）+ `ResilienceConfig`（含 `overall_timeout_ms`）+ 单测。
4. 定义 `ResilienceObserver` trait + `NoopObserver`（决策 3A）。
5. 新增 `ResilientClient` + `ResilientError` + `UpstreamResponse`（owned 响应 5A，deadline 4A/8C，读锁优先 15A）+ observer 回调单测（13A）。
6. gateway `RouterConfig` / `forward_to_service` 接线 + 四态映射（5A/12A）+ 服务发现边界注释（9A/14B）。
7. gateway 实现 `ResilienceObserver` 写 Prometheus + 配置段（`resilience:`）。
8. 集成测试（axum mock 下游：超时/overall deadline 11A/重试/短路/恢复/四态映射 12A）。
9. `test-case/51-resilience-breaker.md` + 旧 spec `phase4-perf-circuit-limit.md` 标注 superseded。

## 12. NOT in scope（明确延后）

- **韧性接入其他服务间直调**（token→user、各服务 `digua_common-http::HttpClient` 调用点）——本期仅 gateway outbound；已加入 TODO.md。
- **限流（rate limiting）**——gateway 已有 `governor`、notification 用 `digua_common_ratelimit`；本期删 `rate_limit.rs` 不替换。
- **mTLS / 服务网格 / 淘汰 `require_service_token`**——Phase 5 子项目 A，独立 spec。
- **breaker 状态跨副本共享**——已评估并否决（§10，决策 1A）。
- **OTel metrics**——本期用 Prometheus（gateway 现有），observer trait 已为未来换 OTel 解耦。
- **Phase 4 遗留 I1**（RabbitMQ consumer transient/permanent 区分 + DLQ TTL retry）——主题相邻但机制不同（消息中间件，非 HTTP），保持原位记录。

## 13. What already exists（复用情况）

| 子问题 | 已有代码 | 处置 |
|--------|---------|------|
| 熔断状态机 | `circuit_breaker.rs`（Open 计时 bug，无人用） | 重写复用，修 bug |
| 重试退避 | `retry.rs`（字符串判定 + 劣质 jitter） | 重写复用，换 rand + 枚举 |
| 配置 | `config.rs`（`HttpDefaultsConfig`） | 重写为 `ResilienceConfig` |
| gateway 转发 | `router.rs:16 forward_to_service`（裸 reqwest） | 接线改造 |
| Prometheus + /metrics | gateway 现有 | 复用（observer 写入） |
| body 已读 Bytes | `router.rs:33` | 复用，规避 body 流消费 |
| Redis 连接 | gateway Phase 4.4 perm_epoch 已接 | **评估后不复用**（§10 决策 1A） |

无重复造轮子；孤儿脚手架（axum/benchmarks/rate_limit/middleware/service_discovery）删除不重建。

## 14. 失败模式分析（全景图每条新路径）

| 新路径 | 真实失败方式 | 测试 | 错误处理 | 用户可见 | 评级 |
|--------|------------|------|---------|---------|------|
| 单次 timeout | 下游慢 | ✅ 8.2 | →504 | ✅ 504 | OK |
| overall deadline | 重试拖过死线 | ✅ 11A | →504 提前截断 | ✅ 504 | OK |
| Connect 失败 | 下游拒连/宕机 | ✅ 12A | →502 | ✅ 502 | OK |
| breaker Open 短路 | 下游持续故障 | ✅ 10A/12A | →503 JSON | ✅ 503 circuit_open | OK |
| HalfOpen 并发探测 | 恢复瞬间打穿 | ✅ 10A | 原子占额 | N/A | OK |
| 5xx 重试耗尽透传 | 下游持续 5xx | ✅ 12A | Ok 透传原 body | ✅ 原始 5xx | OK |
| observer 回调 | metrics 静默无数据 | ✅ 13A | NoopObserver 默认 | ⚠️ 仅监控 | OK（13A 覆盖） |
| 非幂等不重试 | POST 失败 | ✅ 12A | 不重试直接返回 | ✅ 原始错误 | OK |
| 服务发现失败 | Consul 故障 | 🚫 结构排除(14B) | gateway 既有处理 | ✅ 既有状态码 | OK |

**critical gap：0 个。** review 前 §8 存在 5 个「无测试且会静默失败」缺口（尤其 observer/G5），已由 10A–13A 全部闭合。
