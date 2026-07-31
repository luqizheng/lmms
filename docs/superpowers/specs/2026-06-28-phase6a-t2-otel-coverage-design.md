# Phase 6-A · T2 — OTel 全覆盖 + tracing_mw 提取共用

> 日期：2026-06-28
> 来源：TODO「Phase 6 — 配置 & 可观测性补齐」中的 `OTel 全覆盖`。
> 上下文：plan review（2026-06-28，SCOPE REDUCTION → Phase 6-A）。
>
> **Plan-review 结论已 bake（推荐方案）：**
> 0-Q2 A 提取 `tracing_mw.rs` 到 `digua_common-telemetry`（1 处定义 + 8 处 use）·
> 给 audit / config / file / prue_backend 4 个服务接 `init_tracing` + `tracing_mw` ·
> 与 T1 联动让 producer→consumer 跨服务 span chain 在 Jaeger 可见。

## 1. 背景与问题

README OTel 表当前状态：

| Service | OTel |
|---|---|
| api-gateway | ✅ |
| token-service | ✅ |
| iam-service | ✅ |
| notification-service | ✅ |
| audit-service | — |
| config-service | — |
| file-service | — |
| prue_backend | — |

四个未接入服务的 `main.rs` 仍用 `digua_common_log::init_json`（见
`services/audit-service/src/main.rs:3,49`、`services/config-service/src/main.rs:3,8`、
`services/file-service/src/main.rs:3,9`、`services/prue_backend/src/main.rs:2,9`），
**没有 OTel 导出 + 没有 HTTP root span**——T1 producer 注入了 trace_id，但这 4 个服务
作为 producer / consumer 时都看不到链。

同时已有 4 份 **逐字相同** 的 `tracing_mw.rs`（73-85 行）：

```
services/api-gateway/src/middleware/tracing_mw.rs
services/iam-service/src/tracing_mw.rs
services/notification-service/src/tracing_mw.rs
services/token-service/src/tracing_mw.rs
```

如果不提取，T2 落地后会变 8 份。DRY 违规。

## 2. 目标 / 非目标

### 目标

- 给 audit / config / file / prue_backend 4 个服务接 `digua_common_telemetry::init_tracing` 并启用 `otlp` feature。
- 给上述 4 个服务接 HTTP 根 span 中间件（W3C traceparent extract + X-Trace-Id 注入 + status/duration 字段）。
- 把 `tracing_mw.rs` 提取到 `libs/digua_common-telemetry::middleware::trace_request_middleware`，
  4 处旧 copy 删除并改 use；4 处新接入直接用。
- audit-service consumer 端从 envelope.trace_id（T1 已注入）建 span 父子关系，
  跨服务 trace chain 在 Jaeger 可见。
- README OTel 表 4 个 `—` 改 `✅`。

### 非目标（YAGNI）

- **不**做 metrics export 到 OTLP（保留现有 Prometheus 端点，不动）。
- **不**给 outbox-publisher（独立 binary）接 OTel——已经从 envelope_json 透传 trace_id，
  publisher 端不引 OTel SDK，避免与「outbox 与业务进程解耦」原则冲突。
- **不**改 `digua_common_log::init_json` 的现有 caller（`digua_common-log` 与 `digua_common-telemetry` 共存，
  init_tracing 已包含日志层）。新接入的 4 服务从 `init_json` 切到 `init_tracing`。
- **不**做 sampling 配置（保持 OTel SDK 默认 ParentBased + AlwaysOn）。
- **不**给消费者侧的「envelope→span」builder 提取到 digua_common-event；audit/notification 各自实现，
  本期只统一 HTTP root span。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|---|---|---|
| 提取位置 | `libs/digua_common-telemetry/src/middleware.rs` | 与 init_tracing 同 crate，调用方一次 `use digua_common_telemetry::trace_request_middleware` 解决 |
| 中间件 feature 门控 | 跟随 `digua_common-telemetry` 的 `otlp` feature 启用 | 无 OTel 时仍能用 fallback random trace_id（4 份现有 copy 的行为本就如此） |
| **middleware 模块本身的 feature 门控** | **新增 `axum` feature**；module `#[cfg(feature = "axum")]` 整体门控 | 避免 axum + tower + hyper ~1MB 进 outbox-publisher 等 CLI binary；与 otlp feature 风格一致 |
| 4 服务 OTel 接入 | 各 `Cargo.toml` 加 `digua_common-telemetry = { features = ["otlp", "axum"] }` | 与 token/iam/notification/gateway 现状对齐 |
| outbox-publisher 不开 axum | binary 端 `digua_common-telemetry` 默认不带 axum feature | binary 不需要 HTTP middleware，避免膨胀 |
| 4 服务 HTTP middleware 接入 | 在各自 `routes::routes(state)` 构造的 Router 上挂 `.route_layer(from_fn(trace_request_middleware))` | 与 token-service 现状对齐 |
| consumer 侧 span parent 关联 | audit-service 的 `consumer.rs` / `risk.rs`、notification-service 的 `consumer/mod.rs` 在收到 envelope 后**用 envelope.trace_id 起一个 span** | 让 Jaeger 看到完整链；本期仅做 audit-service 一处作为参考实现，notification/gateway perm_consumer 留 follow-up |
| 旧 4 份 tracing_mw 删除 | 全删，改 `use digua_common_telemetry::trace_request_middleware` | DRY |

### 3.1 关于 consumer span parent 仅本期做 audit-service 的折中

完整端到端链 = producer span → outbox JSON → publisher → broker → **consumer span 设父**。
notification-service 与 api-gateway perm_consumer 也是 consumer，但本期 spec 范围内只做
**audit-service 一处** 作为参考实现，理由：

- audit-service 是 Phase 4 主验收路径（iam → audit 闭环），跨服务 trace 在 audit 落库后看
  得到才算端到端。
- notification / gateway 的 consumer 改造是同样 pattern 但代码位置 6+ 处，本期不全铺，
  避免 spec 蔓延。
- audit 一处成功后留 follow-up `T2-fu1` 扩面到其余 consumer。

## 4. 数据流（与 T1 联动）

```
HTTP req (traceparent: 00-T1-S1-01)
   │
   ▼
api-gateway tracing_mw → extract T1 → root span
   │ forward (traceparent passed downstream)
   ▼
iam-service tracing_mw → child span, trace_id=T1
   │ business handler
   ▼
let mut env = Envelope::new(...);
outbox_repo.enqueue(&mut tx, &mut env)    // T1 内 current_trace_id() → env.trace_id = T1
   │
   ▼
iam_outbox_events row {envelope_json: {trace_id: "T1", ...}}
   │
   ▼
outbox-publisher (无 OTel SDK；basic_publish 透传 envelope_json) → RabbitMQ
   │
   ▼
audit-service consumer.rs
   ├── deserialize Envelope<P> → env.trace_id = Some("T1")
   ├── ★ 新增：build_consumer_span(&env) → span with parent (T1)
   ├── handler(env) instrument with that span
   ▼
Jaeger UI: 一条 trace T1
   ┌─ gateway HTTP root
   ├─── iam HTTP child
   ├───── enqueue / DB span
   └─── audit consumer span（同 trace_id）
```

## 5. 组件改动

### 5.1 `libs/digua_common-telemetry` 加 middleware 模块

新建 `libs/digua_common-telemetry/src/middleware.rs`（内容是 4 份 copy 的逐字合并，无逻辑改动）。
**整个模块由 `axum` feature 门控**，避免拖 axum 进 CLI binary（outbox-publisher 等）。

```rust
//! Per-request tracing middleware. Extracted from 4 copy-paste copies in
//! services/{api-gateway,iam-service,notification-service,token-service}.
//!
//! Module is gated by the `axum` feature — only HTTP-server consumers need it.
//! CLI binaries (outbox-publisher, etc.) that just want init_tracing leave it off.

#[cfg(all(feature = "axum", feature = "otlp"))]
pub use otlp_impl::trace_request_middleware;

#[cfg(all(feature = "axum", not(feature = "otlp")))]
pub use fallback_impl::trace_request_middleware;

#[cfg(all(feature = "axum", feature = "otlp"))]
mod otlp_impl {
    use axum::{
        body::Body, extract::Request, http::HeaderValue, middleware::Next,
        response::Response,
    };
    use opentelemetry::{global, trace::{TraceContextExt, TraceId}};
    use std::time::Instant;
    use tracing::Instrument;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        // (与 services/api-gateway/src/middleware/tracing_mw.rs 完全一致)
        // ...
    }
}

#[cfg(all(feature = "axum", not(feature = "otlp")))]
mod fallback_impl {
    // 仅日志 root span + 随机 X-Trace-Id，不需要 OTel SDK
    use axum::{
        body::Body, extract::Request, http::HeaderValue, middleware::Next,
        response::Response,
    };
    use std::time::Instant;
    use tracing::Instrument;

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        let span = tracing::info_span!(
            "http_request",
            http.method = %method,
            http.path = %path,
            http.status_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
        );
        let start = Instant::now();
        let mut response = next.run(request).instrument(span.clone()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let trace_id = hex::encode(rand::random::<[u8; 16]>());
        span.record("http.status_code", response.status().as_u16() as i64);
        span.record("duration_ms", elapsed_ms as i64);
        if let Ok(hv) = HeaderValue::from_str(&trace_id) {
            response.headers_mut().insert("X-Trace-Id", hv);
        }
        response
    }
}
```

`libs/digua_common-telemetry/Cargo.toml` 更新：

```toml
[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true

# axum middleware deps — opt-in via `axum` feature
axum = { workspace = true, optional = true }
rand = { version = "0.8", optional = true }
hex = { version = "0.4", optional = true }

# OTel SDK deps — opt-in via `otlp` feature
opentelemetry = { version = "0.24", optional = true }
opentelemetry-otlp = { version = "0.17", optional = true, default-features = false, features = ["grpc-tonic", "trace"] }
opentelemetry_sdk = { version = "0.24", optional = true, features = ["rt-tokio"] }
tracing-opentelemetry = { version = "0.25", optional = true }

[features]
default = []
axum = ["dep:axum", "dep:rand", "dep:hex"]
otlp = ["dep:opentelemetry", "dep:opentelemetry-otlp", "dep:opentelemetry_sdk", "dep:tracing-opentelemetry"]
```

> **feature 矩阵**（4 个组合，都需 CI 验证）：
>
> | features | 用途 | 暴露符号 |
> |---|---|---|
> | (none) | CLI binary 仅 stdout 日志 | `init_tracing` (stdout only) |
> | otlp | CLI binary 带 OTel 导出（如 outbox-publisher 将来想接） | `init_tracing` (with OTLP) |
> | axum | HTTP 服务无 OTel（罕见，本期无 caller） | `init_tracing` + middleware (fallback) |
> | axum + otlp | HTTP 服务带 OTel（本期主路径） | `init_tracing` + middleware (otlp) |

`libs/digua_common-telemetry/src/lib.rs` 加：

```rust
#[cfg(feature = "axum")]
pub mod middleware;
#[cfg(feature = "axum")]
pub use middleware::trace_request_middleware;
```

### 5.2 删除 4 份旧 copy + 改 use

| 文件 | 操作 |
|---|---|
| `services/api-gateway/src/middleware/tracing_mw.rs` | 删 |
| `services/iam-service/src/tracing_mw.rs` | 删 |
| `services/notification-service/src/tracing_mw.rs` | 删 |
| `services/token-service/src/tracing_mw.rs` | 删 |

各服务的 `main.rs` / `lib.rs` / `mod.rs`：

```rust
// before
mod tracing_mw;  // or middleware::tracing_mw
use crate::tracing_mw::trace_request_middleware;

// after
use digua_common_telemetry::trace_request_middleware;
```

api-gateway 的 `services/api-gateway/src/middleware/mod.rs` 需删除 `pub mod tracing_mw;`
并修正 `super::middleware::tracing_mw::trace_request_middleware` 的 import。

### 5.3 4 个新服务接入

每个服务做以下三件事：

**1. `Cargo.toml`**

```toml
digua_common-telemetry = { path = "../../libs/digua_common-telemetry", features = ["otlp", "axum"] }
```

> 这 4 个新服务都是 HTTP 服务，axum feature 必带。
> 现有 4 服务（api-gateway / iam-service / notification-service / token-service）的 `Cargo.toml`
> 在本期一并加上 `axum` feature flag——之前没启用是因为 tracing_mw.rs 在各服务本地引 axum，
> 提取到 digua_common-telemetry 后 feature 必须 explicit 开启。

**2. `main.rs` 把 `init_json(...)` 换成 `init_tracing(...)`**

```rust
// audit-service main.rs:3,49
- use digua_common_log::init_json;
+ // 用 digua_common_telemetry，OTel feature 已启用
...
- init_json("info");
+ digua_common_telemetry::init_tracing("audit-service");
```

同样模式应用到 config-service / file-service / prue_backend（service_name 各异）。

**3. Router 挂 middleware**

各服务的 `routes.rs` / `routes/mod.rs` / 等 build router 处统一在最后挂：

```rust
.route_layer(axum::middleware::from_fn(digua_common_telemetry::trace_request_middleware))
```

具体位置按服务现有 `routes()` 函数风格。

### 5.4 audit-service consumer 侧 span parent（参考实现）

`services/audit-service/src/consumer.rs`（durable 队列处理 iam.user.created 等 audit 写入路径）
和 `services/audit-service/src/risk.rs` 的 handler 入口，加：

```rust
use opentelemetry::trace::{SpanContext, TraceFlags, TraceId, TraceState, SpanId};
use tracing_opentelemetry::OpenTelemetrySpanExt;

fn span_from_envelope_trace_id<P>(env: &Envelope<P>, name: &'static str) -> tracing::Span {
    let span = tracing::info_span!(
        target: "consumer",
        "consume",
        event_type = %env.event_type,
        otel.kind = "consumer",
    );
    if let Some(tid_hex) = env.trace_id.as_deref() {
        if let Ok(tid) = TraceId::from_hex(tid_hex) {
            // span_id 给 invalid，会让 OTel 把当前 span 作为 trace=T1 下的新 span
            let sc = SpanContext::new(
                tid,
                SpanId::INVALID,                   // parent span_id 未知（producer 不存）
                TraceFlags::SAMPLED,
                /*is_remote=*/ true,
                TraceState::default(),
            );
            let cx = opentelemetry::Context::current()
                .with_remote_span_context(sc);
            span.set_parent(cx);
        }
    }
    span
}
```

在 handler 入口：

```rust
let span = span_from_envelope_trace_id(&env, "audit.iam-events");
async move {
    // 现有 handler 逻辑
}.instrument(span).await
```

notification-service / gateway perm_consumer 留 follow-up `T2-fu1`。

### 5.5 README OTel 表更新

```
| audit-service        | ✅ |
| config-service       | ✅ |
| file-service         | ✅ |
| prue_backend         | ✅ |
```

并在「Phase 6-A」section 加一行说明：T1+T2+T3+T4 联动。

## 6. 测试计划

### 6.1 编译期 / 单元

- workspace `cargo check --all-features`：4 份旧 tracing_mw.rs 删除 + 提取后所有 caller 用 digua_common-telemetry 的版本，应一次通过。
- **feature 矩阵 CI**：分别跑 `cargo check -p digua_common-telemetry --no-default-features`、`--features axum`、`--features otlp`、`--features axum,otlp` 四组，确保每个组合都干净编译。
- 4 服务的现有单测全绿（中间件提取是行为不变的 mechanical refactor）。

### 6.2 集成 / smoke

- `pnpm run check:env`：4 服务起进程后探活 `/health` 200。
- 手工：curl 4 服务 `/health` 带 `traceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01`，
  断言响应 `X-Trace-Id: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`（W3C trace_id 部分透传）。
- 反例：不带 traceparent，响应 X-Trace-Id 非空（32 hex）。

### 6.3 E2E `test-case/62-otel-cross-service-chain.md`（手工验收）

```text
1. 起 infra + observability overlay (Jaeger :16686)
2. 起 iam + outbox-publisher + audit-service（全带 OTEL_EXPORTER_OTLP_ENDPOINT）
3. curl POST gateway /v1/iam/users 创建用户（带 traceparent T1）
4. 等 5 秒（outbox poll + audit consume）
5. Jaeger UI: search trace_id=T1
   断言：
   - gateway HTTP root span
   - iam HTTP child span
   - iam DB enqueue span
   - audit consumer span（设父到 T1）
   四节点同 trace_id，组成完整链
```

### 6.4 Failure modes

| 路径 | 失败 | 测试? | 错误处理? | 用户可见? |
|---|---|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` 未设 | 退化 stdout-only | 已被 init_tracing 现有分支处理 | ✅ | log 可见 |
| Collector 不可达 / 慢 | batch 导出阻塞 | OTel SDK 内置 timeout / queue | ✅（drop on overflow） | metric 可见 |
| envelope.trace_id 是 valid hex 但全 0 | OTel 拒绝 invalid TraceId | from_hex 返 Err 走 None 分支 | ✅ | log 可见 |
| fallback 路径 random trace_id 与上游真 T1 不一致 | feature 配错（otlp 未启用） | 编译期 features 列表保证 | ✅（fallback 是 best-effort） | X-Trace-Id 可见 |

无 critical gap。

## 7. 实施步骤

1. 提取 `libs/digua_common-telemetry/src/middleware.rs` + Cargo.toml 加 axum/rand/hex 依赖。
2. 4 处旧 `tracing_mw.rs` 删除，相应 caller 改 `use digua_common_telemetry::trace_request_middleware`。
3. workspace `cargo check`：4 个已接入服务回归（无行为改动）。
4. 4 个新服务 `Cargo.toml` 加 `digua_common-telemetry = { features = ["otlp"] }`，main.rs 从 `init_json` 切到 `init_tracing`，router 挂 middleware。
5. audit-service consumer / risk handler 加 `span_from_envelope_trace_id` 父子关联。
6. workspace `cargo build` 全过 → infra + observability overlay 起来。
7. 手工 cross-service trace 验收（test-case/62）。
8. 更新 README OTel 表 + Service 表。

## 8. 验收

- [ ] 4 份旧 tracing_mw.rs 全删除，无 `cargo check` warning。
- [ ] audit/config/file/prue_backend 4 服务 `/health` + traceparent 透传 X-Trace-Id 一致。
- [ ] Jaeger UI 看到 iam → audit 完整跨服务 trace 链。
- [ ] README 4 个 `—` 改 `✅`。
- [ ] notification / gateway perm_consumer 加 follow-up T2-fu1（不阻塞本期合并）。

## 9. 风险与 mitigation

| 风险 | 缓解 |
|---|---|
| middleware 提取后 4 处 use 漏改 → 编译错 | 编译器全程兜底，无运行时风险 |
| digua_common-telemetry 加 axum 依赖污染纯 lib | axum 由 `axum` feature 门控，CLI binary（outbox-publisher 等）不开 feature → 不拖 axum 进 binary |
| audit consumer parent 关联依赖 envelope.trace_id 非空 → T1 必须先合 | spec 内已声明 T1 优先；T2 PR 在 T1 合后 |
| OTel version 漂移（0.24 vs 0.25 跨 crate） | 与 api-gateway 现用版本对齐；workspace Cargo.toml 锁版本 |
| feature gate 配错 | CI 矩阵 4 组合（none / axum / otlp / axum+otlp）全跑（见 §6.1） |
