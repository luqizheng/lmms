# Phase 6-A · T2 — OTel Coverage + tracing_mw Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `tracing_mw.rs` 提取到 `common-telemetry::middleware`（feature-gated），并给 audit / config / file / prue_backend 4 个未接 OTel 的服务接入，端到端 trace 链在 Jaeger 可见。

**Architecture:**
- `libs/common-telemetry` 加 `axum` feature（避免拖 axum 进 CLI binary），middleware 模块整体门控在该 feature 下。
- 4 份逐字相同的 `tracing_mw.rs`（api-gateway / iam / notification / token）合并到 `common-telemetry::middleware::trace_request_middleware`，分别两路实现：`otlp` feature 内走 OTel SDK + `opentelemetry` 提取 trace_id；无 otlp 时 fallback 走随机 128-bit。
- audit / config / file / prue_backend 各自 `Cargo.toml` 加 `features = ["otlp", "axum"]`，main.rs 从 `init_json` 切到 `init_tracing`，router 挂 `trace_request_middleware`。
- audit consumer 用 T1 已注入的 `envelope.trace_id` 建 span parent，让 iam→outbox→audit 跨服务 chain 在 Jaeger 端到端。

**Tech Stack:**
- `axum 0.8` (workspace)、`opentelemetry 0.24`、`opentelemetry-otlp 0.17`、`opentelemetry_sdk 0.24`、`tracing-opentelemetry 0.25`
- `rand 0.8` (workspace)、`hex 0.4`

**Spec:** `docs/superpowers/specs/2026-06-28-phase6a-t2-otel-coverage-design.md`

---

## Global Constraints

- **middleware feature gate**：新增 `axum` feature，门控 middleware 整个模块（`#[cfg(feature = "axum")]`）。`otlp` feature 内 API 路径走 OTel SDK，否则走 fallback。
- **feature 矩阵（CI 必须 4 组合全跑）**：
  - `(none)`：CLI binary only stdout
  - `otlp`：CLI binary + OTel export
  - `axum`：HTTP service，无 OTel（本期无 caller）
  - `axum + otlp`：HTTP service + OTel（本期主路径）
- **4 个现有 HTTP 服务（api-gateway / iam / notification / token）也加 axum feature**：之前 tracing_mw.rs 在各服务本地，提取后 feature 必须 explicit 开。
- **outbox-publisher binary 不加 axum feature**：避免 axum 进 CLI binary。
- **audit consumer span parent 是参考实现**：notification / gateway perm_consumer 留 T2-fu1 follow-up。
- **README OTel 表**：4 个 `—` 改 `✅`。
- **每步 commit**：T2 内拆 5 个 commit。

---

## File Structure

### 新增

| 路径 | 职责 |
|---|---|
| `libs/common-telemetry/src/middleware.rs` | 提取的 trace_request_middleware（两路 otlp/fallback） |
| `test-case/62-otel-cross-service-chain.md` | 端到端 Jaeger trace 验收清单 |

### 修改

| 路径 | 改动 |
|---|---|
| `libs/common-telemetry/Cargo.toml` | 加 `axum` / `rand` / `hex` optional；加 `axum` feature |
| `libs/common-telemetry/src/lib.rs` | 暴露 middleware 模块（feature-gated） |
| `services/api-gateway/Cargo.toml` | `features = ["otlp", "axum"]` |
| `services/api-gateway/src/middleware/mod.rs` | 删 `pub mod tracing_mw;` |
| `services/api-gateway/src/middleware/tracing_mw.rs` | 删 |
| `services/api-gateway/src/main.rs` | import 改 `common_telemetry::trace_request_middleware` |
| `services/iam-service/Cargo.toml` | `features = ["otlp", "axum"]` |
| `services/iam-service/src/lib.rs` | 删 `pub mod tracing_mw;` |
| `services/iam-service/src/tracing_mw.rs` | 删 |
| `services/iam-service/src/main.rs` | import 改 |
| `services/notification-service/Cargo.toml` | `features = ["otlp", "axum"]` |
| `services/notification-service/src/lib.rs` | 删 `pub mod tracing_mw;` |
| `services/notification-service/src/tracing_mw.rs` | 删 |
| `services/notification-service/src/main.rs` | import 改 |
| `services/token-service/Cargo.toml` | `features = ["otlp", "axum"]` |
| `services/token-service/src/lib.rs` | 删 `pub mod tracing_mw;` |
| `services/token-service/src/tracing_mw.rs` | 删 |
| `services/token-service/src/main.rs` | import 改 |
| `services/audit-service/Cargo.toml` | 加 common-telemetry `features = ["otlp", "axum"]` |
| `services/audit-service/src/main.rs` | `init_json` → `init_tracing("audit-service")` + 挂 middleware |
| `services/audit-service/src/consumer.rs` | 加 `span_from_envelope_trace_id` + handler 入口 `instrument` |
| `services/audit-service/src/risk.rs` | 同上 |
| `services/config-service/Cargo.toml` | 同 audit |
| `services/config-service/src/main.rs` | 同 audit（service_name 改 `"config-service"`） |
| `services/file-service/Cargo.toml` | 同 audit |
| `services/file-service/src/main.rs` | 同 audit（service_name 改 `"file-service"`） |
| `services/prue_backend/Cargo.toml` | 同 audit |
| `services/prue_backend/src/main.rs` | 同 audit（service_name 改 `"prue_backend"`） |
| `README.md` | OTel 表 4 个 `—` 改 `✅` |

---

## Task Ordering & Dependencies

```
T2.1 (extract middleware) → T2.2 (delete 4 copies + rewire imports)
                              ↓
T2.3 (audit OTel + span parent ref impl)  → T2.4 (3 other services OTel)
                                              ↓
                                              T2.5 (README + e2e)
```

注意 T2.1 不依赖 T1，但 T2.3 (audit consumer span parent) 依赖 T1 (trace_id 注入)。建议合并到 T1 PR 后做 T2.3，或在 T2.3 PR 注释中说明「期望 T1 已合；T1 未合时 envelope.trace_id 全是 None，不影响代码编译」。

---

### Task 1: 在 common-telemetry 加 `axum` feature 与 middleware 模块

**Files:**
- Modify: `libs/common-telemetry/Cargo.toml`
- Modify: `libs/common-telemetry/src/lib.rs`
- Create: `libs/common-telemetry/src/middleware.rs`

**Interfaces:**
- Produces: `common_telemetry::trace_request_middleware`（仅在 `axum` feature 启用时存在）
- 4 组合 CI 验证：`--no-default-features`、`--features axum`、`--features otlp`、`--features axum,otlp`

- [ ] **Step 1: 修改 `libs/common-telemetry/Cargo.toml`**

完整内容改为：

```toml
[package]
name = "common-telemetry"
version = "0.1.0"
edition = "2021"

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

- [ ] **Step 2: 新建 `libs/common-telemetry/src/middleware.rs`**

```rust
//! Per-request tracing middleware. Extracted from 4 copy-paste copies in
//! services/{api-gateway,iam-service,notification-service,token-service}.
//!
//! Module is gated by the `axum` feature — only HTTP-server consumers need it.
//! CLI binaries (outbox-publisher, etc.) that just want init_tracing leave it off.
//!
//! Two implementations:
//! - `otlp` feature ON  → OTel SDK + tracing-opentelemetry. Trace_id from real
//!   span context; falls back to a random 128-bit id if no OTel subscriber.
//! - `otlp` feature OFF → plain tracing root span + random 128-bit fallback id.

#[cfg(all(feature = "axum", feature = "otlp"))]
pub use otlp_impl::trace_request_middleware;

#[cfg(all(feature = "axum", not(feature = "otlp")))]
pub use fallback_impl::trace_request_middleware;

#[cfg(all(feature = "axum", feature = "otlp"))]
mod otlp_impl {
    //! 与 services/api-gateway/src/middleware/tracing_mw.rs 完全一致的 OTel 实现。
    use axum::{
        body::Body,
        extract::Request,
        http::HeaderValue,
        middleware::Next,
        response::Response,
    };
    use opentelemetry::global;
    use opentelemetry::trace::{TraceContextExt, TraceId};
    use std::time::Instant;
    use tracing::Instrument;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    pub async fn trace_request_middleware(request: Request<Body>, next: Next) -> Response {
        let method = request.method().clone();
        let path = request.uri().path().to_string();

        let span = tracing::info_span!(
            "http_request",
            http.method = %method,
            http.path = %path,
            http.status_code = tracing::field::Empty,
            duration_ms = tracing::field::Empty,
            otel.kind = "server",
        );

        {
            use opentelemetry::propagation::Extractor;
            struct Hd<'a>(&'a axum::http::HeaderMap);
            impl<'a> Extractor for Hd<'a> {
                fn get(&self, key: &str) -> Option<&str> {
                    self.0.get(key).and_then(|v| v.to_str().ok())
                }
                fn keys(&self) -> Vec<&str> {
                    self.0.keys().map(|k| k.as_str()).collect()
                }
            }
            let cx = global::get_text_map_propagator(|prop| prop.extract(&Hd(request.headers())));
            span.set_parent(cx);
        }

        let start = Instant::now();
        let mut response = next.run(request).instrument(span.clone()).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        let trace_id = {
            let cx = span.context();
            let span_ref = cx.span();
            let sc = span_ref.span_context();
            if sc.is_valid() {
                sc.trace_id().to_string()
            } else {
                let id = TraceId::from_bytes(rand::random::<[u8; 16]>());
                id.to_string()
            }
        };

        span.record("http.status_code", response.status().as_u16() as i64);
        span.record("duration_ms", elapsed_ms as i64);

        if !trace_id.is_empty() {
            if let Ok(hv) = HeaderValue::from_str(&trace_id) {
                response.headers_mut().insert("X-Trace-Id", hv);
            }
        }

        response
    }
}

#[cfg(all(feature = "axum", not(feature = "otlp")))]
mod fallback_impl {
    //! 无 OTel：仅日志 root span + 随机 X-Trace-Id（不连 OTel collector）。
    use axum::{
        body::Body,
        extract::Request,
        http::HeaderValue,
        middleware::Next,
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

- [ ] **Step 3: 修改 `libs/common-telemetry/src/lib.rs`**

在 `current_trace_id` 相关代码下方添加：

```rust
#[cfg(feature = "axum")]
pub mod middleware;
#[cfg(feature = "axum")]
pub use middleware::trace_request_middleware;
```

- [ ] **Step 4: CI feature 矩阵验证**

Run: `cargo check -p common-telemetry --no-default-features`
Expected: 编译通过（无 axum、无 otlp）

Run: `cargo check -p common-telemetry --features axum`
Expected: 编译通过（fallback 路径编译）

Run: `cargo check -p common-telemetry --features otlp`
Expected: 编译通过（otlp 路径编译，无 axum）

Run: `cargo check -p common-telemetry --features axum,otlp`
Expected: 编译通过（两 feature 都开）

- [ ] **Step 5: Commit**

```bash
git add libs/common-telemetry/
git commit -m "feat(common-telemetry): extract trace_request_middleware behind axum feature

Consolidates 4 copy-paste copies of tracing_mw.rs (api-gateway / iam /
notification / token) into libs/common-telemetry::middleware.

axum feature gates the module: CLI binaries (outbox-publisher) stay slim
and don't pull axum / rand / hex transitively.

CI must verify all 4 feature combinations: none / axum / otlp / axum+otlp."
```

---

### Task 2: 删除 4 份旧 tracing_mw.rs + 改 import

**Files:**
- Delete: `services/api-gateway/src/middleware/tracing_mw.rs`
- Modify: `services/api-gateway/src/middleware/mod.rs`
- Modify: `services/api-gateway/src/main.rs`
- Delete: `services/iam-service/src/tracing_mw.rs`
- Modify: `services/iam-service/src/lib.rs`
- Modify: `services/iam-service/src/main.rs`
- Delete: `services/notification-service/src/tracing_mw.rs`
- Modify: `services/notification-service/src/lib.rs`
- Modify: `services/notification-service/src/main.rs`
- Delete: `services/token-service/src/tracing_mw.rs`
- Modify: `services/token-service/src/lib.rs`
- Modify: `services/token-service/src/main.rs`
- Modify: `services/{api-gateway,iam-service,notification-service,token-service}/Cargo.toml`

**Interfaces:** 4 份旧 import 路径全部改用 `common_telemetry::trace_request_middleware`

- [ ] **Step 1: 修改 4 个服务的 `Cargo.toml` 加 axum feature**

每个文件的 `common-telemetry` 行改为：

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp", "axum"] }
```

涉及：
- `services/api-gateway/Cargo.toml:18`
- `services/iam-service/Cargo.toml:12`
- `services/notification-service/Cargo.toml:8`
- `services/token-service/Cargo.toml:18`

- [ ] **Step 2: 删除 4 份旧 `tracing_mw.rs`**

```bash
rm services/api-gateway/src/middleware/tracing_mw.rs
rm services/iam-service/src/tracing_mw.rs
rm services/notification-service/src/tracing_mw.rs
rm services/token-service/src/tracing_mw.rs
```

- [ ] **Step 3: api-gateway — 改 `middleware/mod.rs` 与 `main.rs`**

`services/api-gateway/src/middleware/mod.rs`：找到第 14 行（推测）`pub mod tracing_mw;`，删除该行。

`services/api-gateway/src/main.rs`：grep 找 `tracing_mw`，把所有 `...::middleware::tracing_mw::trace_request_middleware` 引用改为 `common_telemetry::trace_request_middleware`。

具体行号以 `rg tracing_mw services/api-gateway/src/main.rs` 实际查询为准。

- [ ] **Step 4: iam-service — 改 `lib.rs` 与 `main.rs`**

`services/iam-service/src/lib.rs`：删除 `pub mod tracing_mw;`（第 11 行附近）。

`services/iam-service/src/main.rs`：grep 找 `tracing_mw`，把 `iam_service::tracing_mw::trace_request_middleware` 改 `common_telemetry::trace_request_middleware`。

- [ ] **Step 5: notification-service — 改 `lib.rs` 与 `main.rs`**

`services/notification-service/src/lib.rs`：删除 `pub mod tracing_mw;`（第 15 行附近）。

`services/notification-service/src/main.rs`：第 87 行（推测）`notification_service::tracing_mw::trace_request_middleware` 改 `common_telemetry::trace_request_middleware`。

- [ ] **Step 6: token-service — 改 `lib.rs` 与 `main.rs`**

`services/token-service/src/lib.rs`：删除 `pub mod tracing_mw;`（第 49 行附近）。

`services/token-service/src/main.rs`：第 137 行（推测）`token_service::tracing_mw::trace_request_middleware` 改 `common_telemetry::trace_request_middleware`。

- [ ] **Step 7: workspace cargo check**

Run: `cargo check --workspace`
Expected: 全部编译通过，4 份旧 import 都已切到 `common_telemetry::trace_request_middleware`

- [ ] **Step 8: 4 个服务的现有单测 + 集成测**

Run: `cargo test -p api-gateway -p iam-service -p notification-service -p token-service`
Expected: 全绿（中间件提取是 mechanical refactor，行为不变）

- [ ] **Step 9: 手工验证 traceparent 透传**

起任一服务（如 iam-service），curl `/health` 带 `traceparent: 00-aaaa...-bbbb-01`，
断言响应 header `X-Trace-Id: aaaa...`。

- [ ] **Step 10: Commit**

```bash
git add services/
git commit -m "refactor: remove 4 tracing_mw.rs copies; rewire to common-telemetry

- delete services/{api-gateway,iam,notification,token}/tracing_mw.rs
- update Cargo.toml: features = ['otlp', 'axum']
- replace import paths in main.rs / lib.rs / middleware/mod.rs

Mechanical refactor; behavior unchanged. Subsequent commit will plug
audit/config/file/prue_backend into the new common module."
```

---

### Task 3: audit-service 接入 OTel + consumer span parent 参考实现

**Files:**
- Modify: `services/audit-service/Cargo.toml`
- Modify: `services/audit-service/src/main.rs`
- Modify: `services/audit-service/src/consumer.rs`
- Modify: `services/audit-service/src/risk.rs`

**Interfaces:** `init_tracing("audit-service")` 替换 `init_json("info")`；handler 入口 `instrument` 带 `span_from_envelope_trace_id(&env)`

- [ ] **Step 1: 修改 `services/audit-service/Cargo.toml` 加 common-telemetry**

找到 `[dependencies]` 段，追加：

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp", "axum"] }
```

- [ ] **Step 2: 修改 `services/audit-service/src/main.rs`**

把：

```rust
use common_log::init_json;
// ...
init_json("info");
```

改为：

```rust
use common_telemetry::init_tracing;
// ...
init_tracing("audit-service");
```

> **注意**：audit-service 可能同时引用 common_log（如旧代码还有用），保留 import 不删；
> 仅把 init 函数切到 telemetry。后续清理可留作 follow-up。

- [ ] **Step 3: 找到 audit 的 router 挂 middleware**

audit-service 的 main.rs（或 routes/mod.rs）构建 Router 处，添加：

```rust
.route_layer(axum::middleware::from_fn(
    common_telemetry::trace_request_middleware,
))
```

> 行为：与 token/iam/notification 现状对齐。

- [ ] **Step 4: 在 audit-service 加 `span_from_envelope_trace_id` helper**

新建 `services/audit-service/src/consumer_span.rs`：

```rust
//! Consumer 侧 OTel span parent 关联。
//!
//! Phase 6-A T2 参考实现：从 envelope.trace_id（T1 已注入）建一个 trace context
//! 作为 consumer span 的 parent，让 producer→consumer 跨服务 chain 在 Jaeger 可见。
//!
//! 仅 audit 一处使用。notification / gateway perm_consumer 留 T2-fu1 follow-up。

use opentelemetry::{
    trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState},
    Context,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use common_event::Envelope;
use common_event::event::Event;

/// Build a tracing span whose OTel parent points to the producer's trace.
/// If envelope.trace_id is None or malformed, the span is parentless
/// (graceful degradation, not error).
pub fn span_from_envelope_trace_id<P: Event>(
    env: &Envelope<P>,
    span_name: &'static str,
) -> tracing::Span {
    let span = tracing::info_span!(
        target: "consumer",
        "consume",
        event_type = %env.event_type,
        otel.kind = "consumer",
        _span_name = span_name,
    );
    if let Some(tid_hex) = env.trace_id.as_deref() {
        if let Ok(tid) = TraceId::from_hex(tid_hex) {
            let sc = SpanContext::new(
                tid,
                SpanId::INVALID, // producer span_id 未知；consumer span 是 trace 下新 span
                TraceFlags::SAMPLED,
                /*is_remote=*/ true,
                TraceState::default(),
            );
            let cx = Context::current().with_remote_span_context(sc);
            span.set_parent(cx);
        }
    }
    span
}
```

`services/audit-service/src/lib.rs` 暴露：

```rust
pub mod consumer_span;
```

- [ ] **Step 5: 在 `services/audit-service/src/consumer.rs` 用 helper**

找到 handler 函数（推测名为 `handle` 或 `process_event`），在 `handler(env).await` 调用前
包一层 `.instrument(span_from_envelope_trace_id(&env, "audit.iam-events"))`：

```rust
use crate::consumer_span::span_from_envelope_trace_id;
// ...
async fn handle(env: Envelope<...>) -> Result<(), HandlerError> {
    let span = span_from_envelope_trace_id(&env, "audit.iam-events");
    async move {
        // 现有 handler body
    }
    .instrument(span)
    .await
}
```

具体函数签名按文件实际形态调整。

- [ ] **Step 6: 在 `services/audit-service/src/risk.rs` 用 helper**

与 Step 5 相同模式，span_name 用 `"audit.login-risk"`。

- [ ] **Step 7: workspace cargo check**

Run: `cargo check --workspace`
Expected: 全部编译通过

- [ ] **Step 8: 跑 audit 现有集成测试**

Run: `cargo test -p audit-service`
Expected: 全绿

- [ ] **Step 9: Commit**

```bash
git add services/audit-service/
git commit -m "feat(audit): OTel init + HTTP middleware + consumer span parent

- init_tracing('audit-service') replaces init_json
- route_layer adds trace_request_middleware
- consumer.rs / risk.rs instrument handler with
  span_from_envelope_trace_id (set parent from envelope.trace_id)

Producer→consumer cross-service trace chain now appears as a single
trace in Jaeger. Reference implementation; notification / gateway
perm_consumer follow-up tracked as T2-fu1."
```

---

### Task 4: config / file / prue_backend 接入 OTel

**Files:**
- Modify: `services/config-service/Cargo.toml`
- Modify: `services/config-service/src/main.rs`
- Modify: `services/file-service/Cargo.toml`
- Modify: `services/file-service/src/main.rs`
- Modify: `services/prue_backend/Cargo.toml`
- Modify: `services/prue_backend/src/main.rs`

**Interfaces:** 与 Task 3 audit 同模式；service_name 各异；本 Task **无** consumer span parent（这 3 个服务没事件消费）

- [ ] **Step 1: 修改 3 个服务的 `Cargo.toml`**

每个文件加：

```toml
common-telemetry = { path = "../../libs/common-telemetry", features = ["otlp", "axum"] }
```

- [ ] **Step 2: `services/config-service/src/main.rs`**

`init_json("info")` → `init_tracing("config-service")`

Router 构建处加：

```rust
.route_layer(axum::middleware::from_fn(
    common_telemetry::trace_request_middleware,
))
```

- [ ] **Step 3: `services/file-service/src/main.rs`**

同上，service_name 改 `"file-service"`。

- [ ] **Step 4: `services/prue_backend/src/main.rs`**

同上，service_name 改 `"prue_backend"`。

> prue_backend 现 main.rs 没用 `axum::middleware::from_fn`，但 `routes::routes()` 已建 Router，
> 挂 middleware 的方式与其它服务一致。

- [ ] **Step 5: workspace cargo check**

Run: `cargo check --workspace`
Expected: 全部编译通过

- [ ] **Step 6: Commit**

```bash
git add services/config-service/ services/file-service/ services/prue_backend/
git commit -m "feat(config|file|prue): OTel init + HTTP middleware

Three more services join the cross-service trace chain. No consumer
side wiring (these services don't subscribe to events)."
```

---

### Task 5: README 更新 + e2e 端到端 trace 验证清单

**Files:**
- Modify: `README.md`
- Create: `test-case/62-otel-cross-service-chain.md`

**Interfaces:** 文档 + 手工验收

- [ ] **Step 1: 修改 `README.md` OTel 表**

找到 README 中 OTel 表（service 与 OTel 列的对应表），把 audit / config / file / prue_backend 4 个 `—` 改 `✅`：

```markdown
| audit-service        | ✅ |
| config-service       | ✅ |
| file-service         | ✅ |
| prue_backend         | ✅ |
```

- [ ] **Step 2: 新建 `test-case/62-otel-cross-service-chain.md`**

```markdown
# T2: OTel 全覆盖 + tracing_mw 提取 — 端到端 trace 链验收

## 前置
docker compose -f infrastructure/docker-compose.yml up -d
scripts/start-observability.sh  # Prometheus :9090, Grafana :3001, Jaeger :16686, OTel collector :4317

## 步骤

### 1. 各服务 OTel init 验证
启动 audit / config / file / prue_backend 各服务，确认日志首行有
"OTel tracing enabled → http://otel-collector:4317"。

如未设 OTEL_EXPORTER_OTLP_ENDPOINT，应回退到 stdout JSON log
（"tracing initialized (stdout only, no OTLP)"）。

### 2. 跨服务 trace 链验证
起 iam-service + outbox-publisher + audit-service（这 3 个有事件 producer/consumer 链）。

执行：
TRACE_ID="aabbccddeeff00112233445566778899"
curl -X POST http://localhost:3000/v1/iam/users \
  -H "Authorization: Bearer ${TOKEN}" \
  -H "traceparent: 00-${TRACE_ID}-1122334455667788-01" \
  -d '{"username":"otel_test","password":"hunter2"}'

等 5s。

打开 Jaeger UI http://localhost:16686：
- Service: iam-service
- 搜 trace_id == aabbccddeeff00112233445566778899
- 应看到：
  1. api-gateway HTTP root
  2. iam-service HTTP child
  3. audit-service consumer span（set parent from envelope.trace_id）
  4. 三者同一 trace_id，组成完整 chain

### 3. traceparent 透传验证
对 audit / config / file / prue_backend 各自 /health：
curl -H "traceparent: 00-${TRACE_ID}-1122334455667788-01" \
     -i http://localhost:6005/health

响应 header X-Trace-Id 应为 aabbccddeeff00112233445566778899。

### 4. feature 矩阵验证（CI 已自动跑，本节为本地 sanity）
cargo check -p common-telemetry --no-default-features        # OK
cargo check -p common-telemetry --features axum              # OK
cargo check -p common-telemetry --features otlp              # OK
cargo check -p common-telemetry --features axum,otlp         # OK
```

- [ ] **Step 3: workspace 全 cargo check 与 feature 矩阵**

Run: `cargo check --workspace`
Expected: 全部通过

Run: `cargo check -p common-telemetry --no-default-features`
Expected: 编译通过

Run: `cargo check -p common-telemetry --features axum,otlp`
Expected: 编译通过

- [ ] **Step 4: 手工 e2e（按 test-case/62）**

按 `test-case/62-otel-cross-service-chain.md` 三步骤执行。

- [ ] **Step 5: Commit**

```bash
git add README.md test-case/62-otel-cross-service-chain.md
git commit -m "docs: T2 README OTel table + test-case/62 cross-service trace verification

- README: 4 '—' → '✅' for audit / config / file / prue_backend
- test-case/62: manual Jaeger trace chain verification steps"
```

---

## Self-Review

### 1. Spec 覆盖检查

| Spec 章节 | 对应 Task |
|---|---|
| §1 背景（4 服务未接 OTel + 4 份 copy）| Task 1 + Task 2 |
| §2 目标（4 服务接 OTel + 提取共用 + audit consumer span parent + README 更新）| Task 1 + Task 2 + Task 3 + Task 4 + Task 5 |
| §3 决策（feature 门控 / Cargo features / 4 服务接入 / consumer 仅 audit 参考实现 / 4 份删除）| Task 1 + Task 2 + Task 3 + Task 4 |
| §3.1 consumer span parent 仅 audit | Task 3 |
| §4 数据流 | Task 3 + Task 5 验收 |
| §5.1 middleware 模块 + Cargo | Task 1 |
| §5.2 删除 4 份 + 改 use | Task 2 |
| §5.3 4 个新服务接入 | Task 4 |
| §5.4 audit consumer span parent | Task 3 |
| §5.5 README OTel 表更新 | Task 5 |
| §6.1 编译期 / feature 矩阵 | Task 1 + Task 5 |
| §6.2 集成 / smoke / traceparent 透传 | Task 2 Step 9 + Task 5 |
| §6.3 E2E Jaeger | Task 5 |
| §6.4 Failure modes | Task 5 README OTel 表更新已含 |

无遗漏。

### 2. 占位符扫描

- 无 "TBD" / "TODO" / "implement later"。
- 无 "Add appropriate error handling"。
- 无 "Write tests for the above"（每个 task 都有具体测试或 cargo check 验证）。
- 无 "Similar to Task N"（Task 2 4 个服务有重复但每步骤 explicit 列出）。

### 3. 类型一致性

- `trace_request_middleware` 签名在 Task 1 定义（两路 otlp/fallback），Task 2 改 import，Task 3 / Task 4 挂 middleware 使用。一致。
- `span_from_envelope_trace_id(env, name) -> tracing::Span` 在 Task 3 定义，Task 3 Step 5/6 使用。一致。
- `common_telemetry::init_tracing` 在 Task 3 / Task 4 替换 `init_json` 使用。一致。

无冲突。

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-28-phase6a-t2-otel-coverage.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
