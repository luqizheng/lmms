# Phase 6-A · T1 — Envelope.trace_id Producer 自动注入

> 日期：2026-06-28
> 来源：TODO Phase 4 broad review 遗留 `I4 envelope.trace_id producer 侧未填充`。
> 上下文：plan review（2026-06-28，SCOPE REDUCTION → Phase 6-A 四件套）。
>
> **Plan-review 结论已 bake（推荐方案）：**
> 2A enqueue 内自动从当前 tracing span context 取 trace_id 注入 · `Envelope.trace_id` 已填则尊重 caller ·
> 无 span context（unit-test 场景）写 `None` 不写假值。

## 1. 背景与问题

Phase 4 落地事件总线时，`Envelope` 已预留 `trace_id: Option<String>` 字段
（`libs/digua_common-event/src/envelope.rs:15`），但 **producer 侧从未填充** ——
所有 5 处 `Envelope::new(...)` 调用（iam-service 内）都没接 `.with_trace_id(...)`。

```
Envelope::new() 调用点（grep 验证）：
  services/iam-service/src/service/user_svc.rs:78    (IamUserCreated)
  services/iam-service/src/service/user_svc.rs:306   (IamUserDeleted)
  services/iam-service/src/service/auth_svc.rs:106   (IamLoginSucceeded)
  services/iam-service/src/service/auth_svc.rs:345   (IamLoginFailed)
  services/iam-service/src/service/perm_change.rs:31 (IamPermissionChanged)
```

后果：

- audit / notification / gateway consumer 收到的 envelope `trace_id` 永远是 `None`，
  跨服务 trace 链断裂——只能靠 log + commit id 排障。
- Phase 6-A 的 T2「OTel 全覆盖」即便铺到 4 个新服务，producer→consumer 这一跳仍连不上 Jaeger。
  T1 不做，T2 收益减半。

## 2. 目标 / 非目标

### 目标

- 所有 producer 端（含**未来新增**的 callsite）写 outbox 的 envelope `trace_id` 自动来自
  当前 tracing span 的 trace_id。
- 已被 caller 主动 `.with_trace_id(...)` 设置过的 envelope **尊重 caller 值**，不覆盖。
- 无 span context（单元测试 / 后台任务无 root span）时 `trace_id` 留 `None`，**不写假值**。
- 5 处现有 callsite **零代码改动**。

### 非目标（YAGNI）

- **不**做 consumer 侧 `span.set_parent(env.trace_id)` 关联——属于 T2 范畴（OTel 接入后跟进）。
- **不**做 `tenant_id` 注入（Phase 8 多租户上才做）。
- **不**改 envelope schema、不改 `iam_outbox_events` 表结构（envelope 已是 JSON column，trace_id 字段
  通过 serde 入库就够）。
- **不**做 publisher 侧 span linking（publisher_loop.rs 已经从 envelope_json 透传 trace_id 字段，
  consumer 侧解析就能用；本期不引 OTel SDK 到 publisher，避免与 outbox 解耦原则冲突）。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|---|---|---|
| 注入点 | `MySqlOutboxRepo::enqueue` 实现内（含内部 `fill_trace_id` helper 便于单测） | DRY，零 callsite 改动；envelope 出厂统一管线；helper 提取避免在测试中起 OTel SDK |
| `current_trace_id()` 放置 | `libs/digua_common-telemetry`（otlp feature 内） | 与 OTel SDK 依赖内聚，避免 digua_common-event 拖第二份 SDK |
| 取 trace_id 的方式 | `tracing::Span::current().context()` + OTel `span_context().trace_id()` | 与 `services/api-gateway/src/middleware/tracing_mw.rs:61-72` 已有的 idiomatic 写法对齐 |
| trace_id 编码 | 32-hex W3C TraceId 字符串（`TraceId::to_string()`） | 与 X-Trace-Id header 同款，consumer 端跨语言可读 |
| 已填则尊重 | `if env.trace_id.is_none() { ... }` 守卫 | caller 显式 .with_trace_id(...) 永远优先 |
| 无 OTel 时 | span_context().is_valid() == false → 不写 | 不伪造，单元测试场景仍保持 None |
| 修改 trait 签名 | **不修改** `OutboxRepo::enqueue` 签名 | trait 契约不变；改的是 `MySqlOutboxRepo` 实现内部 |
| `&Envelope<P>` 入参可变性 | **改为 `&mut Envelope<P>`** | 必须能在 enqueue 内回填 trace_id；5 处 callsite 已是 `&mut tx, &envelope`，envelope 本就是 let 变量、改成 `&mut envelope` 是单字符 diff |

### 3.1 关于 `&Envelope<P>` → `&mut Envelope<P>` 的折中

替代方案是「在 enqueue 内 `env.clone()` 后改 clone」——但 `Envelope<P>` 的 payload 可能含
`serde_json::Value`（如 `NotificationAnnouncementPublished.user_ids`、`IamPermissionChanged.user_ids`），
clone 不便宜。**`&mut Envelope<P>` 更 explicit、零拷贝、零 hack**。

trait 签名改动如下：

```rust
// before
async fn enqueue<P: Event>(
    &self,
    tx: &mut Transaction<'_, MySql>,
    env: &Envelope<P>,
) -> Result<(), OutboxError>;

// after
async fn enqueue<P: Event>(
    &self,
    tx: &mut Transaction<'_, MySql>,
    env: &mut Envelope<P>,
) -> Result<(), OutboxError>;
```

5 处 callsite 当前都是

```rust
let envelope = Envelope::new("iam-service", ...);
self.state.outbox_repo.enqueue(&mut tx, &envelope).await?;
```

按 1 字符 diff 改为：

```rust
let mut envelope = Envelope::new("iam-service", ...);
self.state.outbox_repo.enqueue(&mut tx, &mut envelope).await?;
```

这是 **call-site mechanical**，不是 design 改动，编译器全程兜底。

## 4. 数据流

```
business call (HTTP req with traceparent header)
   │
   ▼
axum router → tracing_mw (root span trace_id = T1)
   │
   ▼
iam-service handler (child span inherits trace_id = T1)
   │
   ▼
let mut env = Envelope::new("iam-service", payload)   // env.trace_id = None
   │
   ▼
outbox_repo.enqueue(&mut tx, &mut env)
   │
   ├── enqueue() 内：current_span_trace_id() → Some("T1")
   ├── env.trace_id.is_none() == true → env.trace_id = Some("T1")
   ├── serde_json::to_value(env) ── trace_id 已落 JSON
   ▼
INSERT INTO iam_outbox_events(... envelope_json) VALUES (... '{"trace_id":"T1",...}')
   │
   ▼
outbox-publisher poll → basic_publish (basic_props.message_id = event_id)
   │
   ▼
audit/notification consumer → deserialize Envelope<P> → env.trace_id = Some("T1")
   │
   ▼ (T2 范畴：consumer span.set_parent → Jaeger chain T1 完整可见)
```

## 5. 组件改动

### 5.1 把 `current_trace_id()` 放在 `libs/digua_common-telemetry`

考虑到 OTel SDK 依赖已在 `digua_common-telemetry`（otlp feature），把 `current_trace_id()` 放那里
更内聚——避免 digua_common-event 同时也拖一份 OTel SDK 副本。

`libs/digua_common-telemetry/src/lib.rs` 新增：

```rust
//! 从当前 tracing span 取 W3C TraceId 字符串。
//!
//! 与 trace_request_middleware（middleware.rs）的 trace_id 取法对齐。
//! `is_valid()` 为 false（未接 OTel / 无 root span / 测试场景）时返回 None。

#[cfg(feature = "otlp")]
pub fn current_trace_id() -> Option<String> {
    use opentelemetry::trace::TraceContextExt;
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    let span = tracing::Span::current();
    let cx = span.context();
    let sc = cx.span().span_context().clone();
    if sc.is_valid() {
        Some(sc.trace_id().to_string())
    } else {
        None
    }
}

#[cfg(not(feature = "otlp"))]
pub fn current_trace_id() -> Option<String> {
    None
}
```

`digua_common-event` 通过 `digua_common_telemetry::current_trace_id` 调用。

> **依赖反向问题**：T2 spec 让 digua_common-telemetry 加 `axum` feature。T1 让 digua_common-event 依赖
> digua_common-telemetry。是否环？—— **不**。digua_common-event 不开 axum feature，只用 `otlp` feature
> （也是可选的；当 otlp 关闭时 current_trace_id 返 None，行为退化与 None handler 等价）。

### 5.1.1 ~~（已迁移到 digua_common-telemetry，本节占位）~~

### 5.2 `libs/digua_common-event/src/outbox.rs` 修改

trait + impl 都改 `&mut`，并在 `MySqlOutboxRepo::enqueue` 内提取一个内部 helper 便于测试，
helper 用闭包接受 trace_id source：

```rust
/// Internal helper: fill `env.trace_id` from `source()` only when caller left it None.
/// Extracted so tests can inject a fake source without an OTel runtime.
fn fill_trace_id<P: Event, F: Fn() -> Option<String>>(env: &mut Envelope<P>, source: F) {
    if env.trace_id.is_none() {
        if let Some(tid) = source() {
            env.trace_id = Some(tid);
        }
    }
}

#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &mut Envelope<P>,           // 改 &mut
    ) -> Result<(), OutboxError>;
    // 其它方法不变
}

#[async_trait]
impl OutboxRepo for MySqlOutboxRepo {
    async fn enqueue<P: Event>(
        &self,
        tx: &mut Transaction<'_, MySql>,
        env: &mut Envelope<P>,
    ) -> Result<(), OutboxError> {
        fill_trace_id(env, || digua_common_telemetry::current_trace_id());
        let json = serde_json::to_value(&*env)?;
        // 其余完全不变
        ...
    }
}
```

测试中可以 `fill_trace_id(env, || Some("test".into()))` / `fill_trace_id(env, || None)`
独立验证分支，不用起 OTel SDK。

### 5.3 5 处 callsite mechanical 改动

| 文件 | 行 | 改动 |
|---|---|---|
| `services/iam-service/src/service/user_svc.rs` | 78、87-88 | `let envelope` → `let mut envelope`；`&envelope` → `&mut envelope` |
| `services/iam-service/src/service/user_svc.rs` | 306、313-314 | 同上 |
| `services/iam-service/src/service/auth_svc.rs` | 106、115（enqueue 调用） | 同上 |
| `services/iam-service/src/service/auth_svc.rs` | 345、356 | 同上 |
| `services/iam-service/src/service/perm_change.rs` | 31、39 | 同上 |

### 5.4 现有测试影响

- `services/iam-service/tests/permission_changed_test.rs` —— 已绿，需要确认 callsite 改 mut 后仍编译通过（不需要业务断言改动）。
- `services/audit-service/tests/login_risk_test.rs` —— Phase 4.2 集成测试，直接 publish envelope，应该与 trait 改动无关。
- `libs/digua_common-event/tests/ephemeral_subscribe.rs` —— 调 `publisher.publish(&env)`，不走 outbox，不影响。
- `services/notification-service/tests/welcome_consumer_test.rs` —— consumer 侧，不影响。

## 6. 测试计划

### 6.1 单元测试 — 拆「取 trace_id」与「注入」两件事独立测

**关键决策**：不在单测里跑 testcontainers + 真 OTel SDK provider（会有全局
`global::set_tracer_provider` 测试污染 + 跨测试 flakiness）。改为拆两层独立测：

#### A. 纯函数测 — `libs/digua_common-telemetry/tests/current_trace_id_test.rs`

把 `current_trace_id()` 从 digua_common-event 抽到 digua_common-telemetry（OTel SDK 已在那里依赖），
让 digua_common-event 通过 `digua_common_telemetry::current_trace_id` 调用。这样取 trace_id 的纯函数测试
与 OTel SDK 自然内聚。

```rust
//! 纯函数测：current_trace_id() 在三种 span context 下返回正确结果。
//! 用 `tracing_subscriber::with_default(...)` scope 隔离，避免全局 provider 污染。

use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry::trace::{Tracer, TracerProvider as _};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn with_otel<F: FnOnce() -> R, R>(f: F) -> R {
    let provider = TracerProvider::builder().build();
    let tracer = provider.tracer("test");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let subscriber = tracing_subscriber::registry().with(otel_layer);
    tracing::subscriber::with_default(subscriber, f)
}

#[test]
fn returns_trace_id_inside_active_span() {
    with_otel(|| {
        let _g = tracing::info_span!("test").entered();
        assert!(digua_common_telemetry::current_trace_id().is_some());
    });
}

#[test]
fn returns_none_without_subscriber() {
    // 无 with_default scope，span context invalid
    assert!(digua_common_telemetry::current_trace_id().is_none());
}

#[test]
fn returns_none_with_subscriber_but_no_span() {
    with_otel(|| {
        // 不进入 span
        assert!(digua_common_telemetry::current_trace_id().is_none());
    });
}
```

要把 `current_trace_id` 放在 digua_common-telemetry 后，digua_common-event 在 outbox enqueue 里改为：

```rust
if env.trace_id.is_none() {
    if let Some(tid) = digua_common_telemetry::current_trace_id() {
        env.trace_id = Some(tid);
    }
}
```

`digua_common-event/Cargo.toml` 依赖 `digua_common-telemetry`（feature `otlp` 必带）。

#### B. 注入逻辑测 — `libs/digua_common-event/tests/enqueue_trace_id_test.rs`

注入逻辑用 mock OutboxRepo + 直接构造 envelope，不需要 OTel SDK、不需要 MySQL：

```rust
//! 注入逻辑测：mock OutboxRepo 不实际写库，验证 enqueue 内的 trace_id 注入分支。
//!
//! 三个分支：
//!   - env.trace_id = None，调 enqueue 后通过 fake `current_trace_id` 注入
//!   - env.trace_id = Some("caller-fixed")，enqueue 后仍是 "caller-fixed"
//!   - env.trace_id = None 且 fake `current_trace_id` 返 None，enqueue 后仍 None

// 实现方式：把 enqueue 内的注入逻辑提取为内部辅助函数
//   fn fill_trace_id<P: Event>(env: &mut Envelope<P>, source: impl Fn() -> Option<String>) { ... }
// 把 source 注入便于测试。MySqlOutboxRepo::enqueue 内调用：
//   fill_trace_id(env, || digua_common_telemetry::current_trace_id());

#[test]
fn fills_trace_id_when_none_and_source_some() { ... }

#[test]
fn respects_caller_trace_id_when_already_set() { ... }

#[test]
fn leaves_none_when_source_returns_none() { ... }
```

#### C. 集成测试 — testcontainers MySQL（一个，跑路径而非分支）

`libs/digua_common-event/tests/enqueue_writes_trace_id_to_db.rs`：

```rust
//! 端到端：testcontainers MySQL，构造 envelope.trace_id=Some("test-trace")，
//! 调真 MySqlOutboxRepo::enqueue，读回 envelope_json，断言 trace_id 字段正确入库。
//! 只跑一个用例，验证「注入逻辑 + JSON 序列化 + DB 写入」整条链路。
```

> **测试拆分理由**：纯函数测 A 不依赖 testcontainers / OTel SDK / MySQL，秒过；分支逻辑测 B 同样秒过；
> 集成测 C 只跑一个 happy path，~3 秒。三层加起来比原方案「3 个测试每个起 testcontainers+OTel」
> 快 5 倍，且无全局 provider 污染。映射「too many tests > too few + explicit > clever」。

### 6.2 E2E `scripts/e2e-trace-id-flow.js`（新增）

```text
启动：infra + iam-service + outbox-publisher + audit-service（含 OTel）
步骤：
  1. curl POST /v1/iam/auth/login —— 带 traceparent header
  2. 等待 outbox publish + audit consumer 处理
  3. SELECT envelope_json->>'$.trace_id' FROM audit_events ORDER BY id DESC LIMIT 1
     断言 trace_id == step1 traceparent 中的 T1
  4. 反例：直接 curl（无 traceparent）→ 等到 envelope 后断言 trace_id 是新生成的（is_valid → 非空）
```

放进 `test-case/61-trace-id-injection.md` 手工验收清单。

### 6.3 Failure modes

| 路径 | 失败 | 测试? | 错误处理? | 用户可见? |
|---|---|---|---|---|
| `current_trace_id()` panic | OTel SDK 内部 panic（极罕见） | ❌ | ❌（直接传播） | log 可见 |
| span context 是 valid 但 trace_id 全 0 | OTel 客户端 bug | ❌ | ❌ | 不可见（trace_id="000…0" 入库） |
| `&mut env` 已被持有 → 编译错 | 5 处 callsite 改 mut | 编译期保证 | n/a | 编译期 |

无 critical gap：OTel SDK panic 是 lib bug，超出范围；trace_id 全 0 实践中没见过。

## 7. 实施步骤（plan 阶段细化）

1. `libs/digua_common-telemetry` 加 `current_trace_id()`（`otlp` feature 内，`#[cfg(not)]` 回退 None）。
2. `libs/digua_common-event/Cargo.toml` 依赖 `digua_common-telemetry = { features = ["otlp"] }`。
3. 改 `outbox.rs`：trait 签名 `&mut Envelope` + 提取 `fill_trace_id` helper + 实现内调用。
4. 5 处 callsite 改 `let mut` + `&mut`。
5. 加纯函数测 `libs/digua_common-telemetry/tests/current_trace_id_test.rs`（with_default scope，3 case）。
6. 加注入逻辑测 `libs/digua_common-event/tests/enqueue_trace_id_test.rs`（fake source，3 case）。
7. 加集成测试 `libs/digua_common-event/tests/enqueue_writes_trace_id_to_db.rs`（testcontainers，1 case）。
8. workspace `cargo check` + `cargo test -p digua_common-telemetry -p digua_common-event -p iam-service`。
9. 加 e2e 用例 `scripts/e2e-trace-id-flow.js` + 手工验收 `test-case/61-trace-id-injection.md`。
10. README OTel 表新增「Producer Trace ID Injection: ✅」一行（在 Phase 6-A 后跟 T2 / T3 / T4 一并）。

## 8. 验收

- [ ] 5 处 callsite 改 mut 后编译过、existing iam-service 集成测试全绿。
- [ ] 纯函数测 3 + 注入逻辑测 3 + 集成测 1 全绿。
- [ ] e2e 验证 traceparent 透传 → audit 库中 trace_id 落库。
- [ ] 与 T2 联调：Jaeger UI 看到 iam → outbox → audit 完整 span chain（T2 上线后跟进）。

## 9. 风险与 mitigation

| 风险 | 缓解 |
|---|---|
| `tracing-opentelemetry` 0.25 与项目内其它版本不一致 | 在 workspace Cargo.toml 检查；与 api-gateway 现用版本对齐 |
| 5 处 callsite 漏改 | `cargo check` 会立刻报 `&Envelope` 不匹配 `&mut Envelope` |
| `Span::current()` 在 axum handler 外（如 outbox publisher 启动期）取不到 | 设计就是 `is_valid()==false → None`，不算 bug |
| OTel SDK feature gate 不一致 | digua_common-event 把 opentelemetry/tracing-opentelemetry 作为必选依赖（非 feature），全 producer 路径行为一致 |
