# Phase 6-B — Config-Service 自举 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development.

**Goal:** token-service 成为第一个从 config-service HTTP API 拉配置的服务。

**Architecture:** `libs/common-config` 加 `config-service` feature → `ConfigServiceProvider` HTTP 拉取 `/config/export/:ns` → `Loader::with_config_service(url, ns)` → fallback 本地 yaml → 可选 30s 轮询热更新。

**Spec:** `docs/superpowers/specs/2026-06-29-phase6b-config-service-dogfood-design.md`

---

## Global Constraints

- 新增 `config-service` feature，`reqwest` optional
- fallback: config-service 不可用 → 本地 yaml，不 panic
- 第一个接入: token-service only
- namespace key: 用 `server.host` / `database.url` 等嵌套路径
- 轮询间隔: 30s，环境变量 `CONFIG_POLL_INTERVAL_SEC` 可配
- 3 commits

## File Structure

| 路径 | 改动 |
|---|---|
| `libs/common-config/Cargo.toml` | + feature `config-service`, + `tracing` dep, + `wiremock` dev |
| `libs/common-config/src/provider.rs` | NEW: ConfigServiceProvider (fetch + parse) |
| `libs/common-config/tests/provider_test.rs` | NEW: wiremock 3 case |
| `libs/common-config/src/lib.rs` | + `with_config_service` builder, + `load()` 内集成, + `watch_config()`, + `pub mod provider` |
| `services/token-service/Cargo.toml` | `features = ["config-service"]` |
| `services/token-service/src/main.rs` | `load_config("config.yaml")` → `Loader::new("TOKEN").with_config_service(...).with_file(...).load()` |
| `test-case/65-config-service-dogfood.md` | NEW: 手工验收清单 |

---

### Task 1: provider.rs + wiremock 3 tests

**Files:** `libs/common-config/Cargo.toml`, `libs/common-config/src/provider.rs` (new), `libs/common-config/tests/provider_test.rs` (new)

**Step 1: Cargo.toml** — 加 `config-service = ["dep:reqwest"]` feature, `tracing = { workspace = true }`, `[dev-dependencies] wiremock = "0.6"`

**Step 2: provider.rs** — 实现 `ConfigServiceProvider` struct with `new(base_url, namespace_id)` + `async fn fetch() -> AppResult<HashMap<String, String>>`. 内部用 `reqwest::Client` (5s timeout) GET `/config/export/{namespace_id}`, 解析 JSON map.

**Step 3: provider_test.rs** — 3 wiremock cases: (a) 200-ok returns map, (b) 404-err, (c) invalid-json-err

**Step 4:** `cargo test -p common-config --features config-service --test provider_test` → 3 passed

**Step 5:** `cargo check -p common-config --no-default-features` + `--features config-service` + `--features consul` all OK

**Step 6:** Commit

---

### Task 2: Loader 集成 provider + watch_config

**Files:** `libs/common-config/src/lib.rs`

**Step 1:** `Loader` struct 加 `#[cfg(feature = "config-service")]` 字段: `config_service_url`, `config_service_namespace`

**Step 2:** `impl Loader` 加 `#[cfg(feature = "config-service")] fn with_config_service(mut self, url, namespace) -> Self`

**Step 3:** `load()` 方法在 file source 后插入: 调 `ConfigServiceProvider::fetch()`, 成功则 `builder.set_override(k, v)` 逐 key 覆盖, 失败则 `tracing::warn!` + 跳过

**Step 4:** 加独立函数 `#[cfg(feature = "config-service")] pub fn watch_config<T>(provider, hot: Arc<HotConfig<T>>, interval) -> JoinHandle` — 后台 30s 轮询, 用 `serde_json::to_value` diff, 有变化时 `hot.update()`

**Step 5:** `lib.rs` 顶部加 `#[cfg(feature = "config-service")] pub mod provider;`

**Step 6:** `cargo check --workspace` OK + 跑 provider_test 3 passed

**Step 7:** Commit

---

### Task 3: token-service 接入

**Files:** `services/token-service/Cargo.toml`, `services/token-service/src/main.rs`, `test-case/65-config-service-dogfood.md` (new)

**Step 1:** Cargo.toml: `common-config = { path = "...", features = ["config-service"] }`

**Step 2:** main.rs: 把 `load_config("config/config.yaml").or_else(|_| load_config("config/token-service.yaml"))` 换成 `Loader::new("TOKEN").with_file("config/config.yaml").with_config_service(env("CONFIG_SERVICE_URL"), env("CONFIG_SERVICE_NAMESPACE")).load()`

**Step 3:** test-case/65: 手工验收 4 步 (建 namespace + 写入配置 + 启动验证 + fallback 验证 + 热更新验证)

**Step 4:** `cargo check -p token-service` + `cargo test -p token-service` OK

**Step 5:** Commit

---

## Self-Review

Spec §5.1-5.5 全覆盖。3 task 3 commit 无遗漏。Provider 用 `wiremock` mock HTTP, Loader 用 `set_override` 叠加, watch 用 `serde_json::to_value` diff。Feature gate 用 `#[cfg(feature = "config-service")]` 隔离。
