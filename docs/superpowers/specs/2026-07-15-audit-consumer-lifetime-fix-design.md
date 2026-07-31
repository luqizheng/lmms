# fix(audit-service): consumer.rs lifetime error — design

| 字段 | 值 |
|---|---|
| Spec 日期 | 2026-07-15 |
| 分支 | `fix/audit-service/consumer-lifetime` |
| 类型 | bugfix |
| 影响面 | `services/audit-service` 单服务 |
| 关联 | `feat/v2.1-dist-cleanup` V1 cargo check 暴露的 pre-existing bug |

## 1. 背景

`feat/v2.1-dist-cleanup` V1 `cargo check --workspace` 失败：

```
error[E0597]: `queue_for_span` does not live long enough
  --> services/audit-service/src/consumer.rs:84:66
```

`cargo check -p consul-registrar` 当时通过是因为只查了新 crate；只在全 workspace 时才暴露。

## 2. 根因

`consumer.rs` lines 75-82 的两次 `String` 转换：

```rust
let queue_name_owned = queue_name.to_string();        // line 75
...
move |env: Envelope<serde_json::Value>| {
    let pool = pool_clone.clone();
    let queue_for_span = queue_name_owned.clone();    // line 82
    async move {
        let span = span_from_envelope_trace_id(&env, &queue_for_span); // line 84
        ...
```

`span_from_envelope_trace_id` 签名：
```rust
pub fn span_from_envelope_trace_id<P: Event>(
    env: &Envelope<P>,
    span_name: &'static str,   // ← tracing::info_span! macro 的硬约束
) -> tracing::Span
```

`&queue_for_span` 失败因为 future 由 `Consumer::subscribe` 的 `H: Fn(Envelope<P>) -> Fut + ... + 'static` 约束驱动，borrow checker 拒绝让 `&String` 跨越 closure 调用栈。

### 为什么 audit-service 是唯一出问题的 consumer？

跨服务对比（已在 repo grep 验证）：
- `services/api-gateway/src/perm_consumer.rs:55` — `span_from_envelope_trace_id(&env, "gateway.perm-epoch")` ✓ `'static` 字面量
- `services/notification-service/src/consumer/membership.rs:64,79,94` — `"notification.membership-events"` ✓ `'static`
- `services/notification-service/src/consumer/mod.rs:65,113` — `"notification.iam-events"` / `"notification.announcement-ws"` ✓ `'static`
- `services/audit-service/src/consumer.rs:80-84` — ❌ 试图用 `String` 动态拼

audit-service 作者的意图看起来是"让 queue_name 可配置"，但 `subscribe_loop` 的两个调用点（行 28、36）本来就是字面量 `"audit.iam-events"` / `"audit.pmd-events"`，String 转换是无谓的间接层，引入了 lifetime 错误。

## 3. 修复方案（最小手术）

### 3.1 文件：`services/audit-service/src/consumer.rs`

**改动 1：`try_subscribe` 参数提升为 `'static`**（调用方 `subscribe_loop` 本来就传字面量）

```diff
 async fn try_subscribe(
     pool: &MySqlPool,
     event_bus_url: &str,
-    queue_name: &str,
-    routing_keys: &[&str],
+    queue_name: &'static str,
+    routing_keys: &'static [&'static str],
 ) -> anyhow::Result<RabbitMqConsumer> {
```

**改动 2：删除两次无谓的 String 分配**

```diff
     let pool_clone = pool.clone();
-    let queue_name_owned = queue_name.to_string();
     consumer
         .subscribe::<serde_json::Value, _, _>(
             queue_name,
             routing_keys,
             move |env: Envelope<serde_json::Value>| {
-                let pool = pool_clone.clone();
-                let queue_for_span = queue_name_owned.clone();
+                // queue_name: &'static str captured by move (since &T is Copy),
+                // flows synchronously into span_from_envelope_trace_id without
+                // any allocation.
+                let span = span_from_envelope_trace_id(&env, queue_name);
+                let pool = pool_clone.clone();
                 async move {
-                    let span = span_from_envelope_trace_id(&env, &queue_for_span);
-                    async move { persist(&pool, &env).await }
-                        .instrument(span)
-                        .await
+                    persist(&pool, &env).await.instrument(span).await
                 }
             },
         )
```

净效果：每次事件处理省 2 次 String 堆分配；closure 维度从 4 行砍到 2 行；span 构造移到 async block 之外（同步），直接用闭包捕获的 `'static str`，Borrow checker 满意。

### 3.2 不改动

- `subscribe_loop` 签名（已正确使用 `'static str` literal）
- `persist` 逻辑
- `run` / `run_pmd` 入口
- `libs/digua_common-event/src/span.rs`（API 仍要求 `'static str`，符合 `tracing` macro 约束）
- `Cargo.toml` workspace members / deps

## 4. 验证

```bash
# 1. 编译通过
SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
  CARGO_TARGET_DIR=/tmp/audit-fix-target \
  cargo check -p audit-service --lib

# 2. 全 workspace 编译（验证 V1 也修好）
cargo check --workspace

# 3. audit-service 现有单元测试不退化
cargo test -p audit-service --lib

# 4. lints 干净
cargo clippy -p audit-service --lib -- -D warnings 2>&1 | tail -10
```

### 接受标准

- [ ] V1: `cargo check -p audit-service --lib` 退出 0，零 error
- [ ] V2: `cargo check --workspace` 退出 0
- [ ] V3: 现有 `login_risk_test` + `models_test` 全过
- [ ] V4: audit-service lib 零 warning（pre-existing 之外）

## 5. 回滚

`git revert` 唯一 commit，恢复 6 行原状（`String` 分配 + 原参数签名）。

## 6. 风险

| 风险 | 缓解 |
|---|---|
| 闭包 &T 是 Copy 真的能跨 await 移动 | `&'static str` literal 编译期常驻；闭包 move 只复制指针，不释放原值 |
| 删 `queue_name_owned.to_string()` 后丢失动态配置能力 | `subscribe_loop` 本来就只用字面量；如未来真要动态化，应在 `digua_common-event` 加 `_owned` 变体（独立 PR） |
| `tracing::info_span!` 把 `&'static str` 烘进 field metadata，不 borrow 后续 | 已确认：审计过 `tracing` 0.1.x 行为，Span 是 owned |

