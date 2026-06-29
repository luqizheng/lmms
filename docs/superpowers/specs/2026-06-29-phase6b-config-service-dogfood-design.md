# Phase 6-B — Config-Service 自举（Dogfooding）

> 日期：2026-06-29
> 来源：TODO「Phase 6 — config-service 自举（dogfooding）」。
> 上下文：Phase 6-A 全部完成并合并。config-service 已有完整 CRUD + 导出/导入端点
> 但无任何服务使用——所有服务仍读本地 yaml。本 spec 是「让 config-service 真正被用上」的最小可行方案。

## 1. 背景与问题

**config-service 现状**（Phase 1-3 已建）：

- `services/config-service/src/services/config.rs`：ConfigService 有 namespace CRUD + config item CRUD + version history + export/import 端点
- `services/config-service/src/routes/config.rs`：HTTP 端点全部就位（`POST /config/item`、`GET /config/export/:ns`、`POST /config/import/:ns` 等）
- 已接 OTel + Swagger UI（Phase 6-A T2）

**问题**：

- **没有任何服务调用 config-service**。`grep config-service` 在整个 `services/` 下结果为 0。
- 每个服务自己读本地 `config/config.yaml`（iam-service 用 `serde_yaml::from_str`、token-service 用 `common-config::Loader` 的 file mode、prue_backend 用 env）。
- config-service 是摆设——自己都不 dogfood。

**现有 `libs/common-config`**：

```
Loader::new(prefix)              → 支持 file / env / Consul KV 三种 source
HotConfig<T>                     → RwLock + on_change 回调，用于运行期热更新
```

**缺少**：从 config-service HTTP API 拉取配置的 source（`ConfigServiceProvider`）。

## 2. 目标 / 非目标

### 目标

- **在 `libs/common-config` 加 `ConfigServiceProvider`**：启动时从 config-service HTTP 拉取 namespace 的全量 config（`GET /config/export/:ns`），fallback 到本地 yaml。
- **第一个接入的服务**：token-service（最小、最独立、有最明确的 config schema）。
- **长期轮询 / watch**：HotConfig 已有回调框架，加一个后台 tokio task 每 30s 轮询 `/export/:ns`，与本地 HotConfig 比对，diff 非空时 update + 触发 on_change 回调。
- **服务配置迁移模式**：保留本地 yaml 作为 fallback，逐步把"必须从配置中心读"的 key 标记。
- **敏感配置标记**：config-service 已有 `is_secret` 字段，Provider 拉到的 value 在 debug log 中 mask 掉。

### 非目标（YAGNI）

- **不一键迁移全部 7 个服务**——token-service 作为第一个 dogfood 参考实现，其余服务后续迭代。
- **不做 SSE / long-polling / gRPC stream watch**——30s 轮询够用。
- **不做 Vault / age 加密**（Phase 6-C 或更晚）。
- **不做多环境 / 多 profile 自动路由**——namespace 用 `{service}-{env}` 约定（如 `token-service-dev`），人工建 namespace。
- **不做配置变更审计 / 回滚**——config-service 已有 version history，Provider 不读。
- **不改 config-service 本身的配置源**——config-service 继续读自己的本地 yaml（鸡生蛋问题，先不管）。

## 3. 决策摘要

| 决策点 | 选择 | 理由 |
|---|---|---|
| Provider 位置 | `libs/common-config`，新 feature `config-service` | 与现有 Loader 同级；不新建 crate |
| 拉取方式 | `GET /config/export/:namespace_id` | config-service 已有此端点，一次拉全量 |
| 启动策略 | 1) 先试 config-service HTTP；2) 失败 → 试本地 yaml；3) 两者都失败 → panic（同现有行为） | 渐进迁移，不破坏现有部署 |
| 轮询间隔 | 30 秒（可配） | 比 SSE/长轮询简单；30s 对配置变更够用 |
| 热更新机制 | 复用 `HotConfig<T>` | 已有 `on_change` 回调框架；Provider 只负责拉取 + diff + update |
| HTTP client | `reqwest::Client`（async，复用连接池） | workspace 已有 reqwest；不加新 dep |
| 第一个接入服务 | `token-service` | 最小、config schema 最清晰、独立部署 |
| namespace 约定 | `{service-name}-{env}`（如 `token-service-dev`） | 人工建 namespace，不自动创建 |
| fallback 语义 | 本地 yaml 作为**兜底**，不是并行 source | 配置中心离线时不阻塞启动 |
| diff 策略 | JSON 全量比较（serde_json::Value），有变化就整个 HotConfig 替换 | 简单；不做 field-level diff |

## 4. 架构

```
token-service main()
   │
   ├── common_config::Loader::new("TOKEN")
   │       .with_config_service("http://config-service:6006", "token-service-dev")
   │       .with_file("config/config.yaml")   ← fallback
   │       .load::<TokenServerConfig>()
   │
   ▼
Loader::load()
   ├── 1) ConfigServiceProvider::fetch("token-service-dev")
   │       GET http://config-service:6006/config/export/token-service-dev
   │       → {"host": "0.0.0.0", "port": 4000, ...}
   │       ✅ 成功 → 用返回的 HashMap<String,String> 构建 Config
   │       ❌ 失败 → log warn，fall through
   │
   ├── 2) File::with_name("config/config.yaml").required(false)
   │       ✅ 本地文件存在 → 合并（config-service 的值优先，本地补充缺失 key）
   │       ❌ 文件不存在 → 如果 config-service 已给全量，跳过
   │
   └── 3) Environment::with_prefix("TOKEN")
           ✅ 环境变量覆盖（最高优先级）
   │
   ▼
   Config::try_deserialize::<T>()
   │
   ▼
   T（如 TokenServerConfig）
```

**启动后轮询（可选）**：

```
Loader::watch::<T>(&self, interval: Duration) → tokio::task::JoinHandle<()>
   │
   └── loop every 30s:
         fetch("token-service-dev") → HashMap
         diff against current HotConfig
         if diff:
           update HotConfig → 触发 on_change callbacks
```

## 5. 组件改动

### 5.1 `libs/common-config/Cargo.toml` 加 `config-service` feature

```toml
[features]
default = []
consul = ["dep:reqwest", "dep:base64"]
config-service = ["dep:reqwest"]   # 新增

[dependencies]
reqwest = { workspace = true, optional = true, default-features = false }
```

### 5.2 `libs/common-config/src/provider.rs`（新增，feature-gated）

```rust
//! ConfigServiceProvider — 从 config-service HTTP API 拉取配置。
//!
//! ```text
//! GET /config/export/{namespace_id} → {"key": "value", ...}
//! ```
//! 返回 JSON object，key-value string pairs。Provider 做一次全量拉取，
//! 支持 fallback 到本地 yaml + 后台轮询热更新。

#[cfg(feature = "config-service")]
pub struct ConfigServiceProvider {
    base_url: String,       // http://config-service:6006
    namespace_id: String,   // token-service-dev
    client: reqwest::Client,
}

#[cfg(feature = "config-service")]
impl ConfigServiceProvider {
    pub fn new(base_url: impl Into<String>, namespace_id: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            namespace_id: namespace_id.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Fetch the full config for the namespace. Returns key-value map.
    /// On HTTP error / non-200 / parse failure, returns the error
    /// (caller decides fallback strategy).
    pub async fn fetch(&self) -> AppResult<HashMap<String, String>> {
        let url = format!("{}/config/export/{}", self.base_url, self.namespace_id);
        let resp = self.client.get(&url).send().await
            .map_err(|e| AppError::external("CFG_PROVIDER_HTTP", e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::external(
                "CFG_PROVIDER_STATUS",
                format!("config-service returned {}", resp.status()),
            ));
        }

        let body = resp.text().await
            .map_err(|e| AppError::external("CFG_PROVIDER_BODY", e.to_string()))?;

        let map: HashMap<String, String> = serde_json::from_str(&body)
            .map_err(|e| AppError::external("CFG_PROVIDER_PARSE", e.to_string()))?;

        Ok(map)
    }
}
```

### 5.3 `libs/common-config/src/lib.rs` 集成 Provider 到 Loader

```rust
pub struct Loader {
    prefix: String,
    file: Option<String>,
    consul_url: Option<String>,
    consul_key: Option<String>,
    // 新增
    config_service_url: Option<String>,
    config_service_namespace: Option<String>,
}

impl Loader {
    // 新增
    pub fn with_config_service(mut self, url: impl Into<String>, namespace: impl Into<String>) -> Self {
        self.config_service_url = Some(url.into());
        self.config_service_namespace = Some(namespace.into());
        self
    }

    pub fn load<T: DeserializeOwned>(self) -> AppResult<T> {
        let mut builder = Config::builder();

        // 1) 本地文件（作为 base）
        if let Some(f) = self.file {
            builder = builder.add_source(File::with_name(&f).required(false));
        }

        // 2) config-service（覆盖本地文件中的同 key）
        #[cfg(feature = "config-service")]
        if let (Some(url), Some(ns)) = (self.config_service_url, self.config_service_namespace) {
            match fetch_config_service(&url, &ns) {
                Ok(map) => {
                    // 把 key-value map 作为 Config source 覆盖
                    for (k, v) in &map {
                        builder = builder.set_override(k.clone(), v.clone())
                            .map_err(|e| AppError::internal("CFG_OVERRIDE", e.to_string()))?;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "config-service fetch failed, using local config");
                }
            }
        }

        // 3) Consul KV
        // ...

        // 4) Environment override（最高优先级）
        builder = builder.add_source(Environment::with_prefix(&self.prefix).separator("_"));

        let config = builder.build()?;
        config.try_deserialize::<T>()
    }
}
```

### 5.4 热更新 watch

```rust
impl<T: DeserializeOwned + Clone + Send + Sync + 'static> Loader {
    /// Start a background task that polls config-service every `interval`
    /// and updates the given `HotConfig<T>` when values change.
    pub fn watch(self, hot: Arc<HotConfig<T>>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                // ... fetch + diff + update ...
            }
        })
    }
}
```

### 5.5 第一个接入：token-service

`services/token-service/Cargo.toml`：

```toml
common-config = { path = "../../libs/common-config", features = ["config-service"] }
```

`services/token-service/src/main.rs` 改动：

```rust
// before
let cfg: TokenServerConfig = load_config("config/config.yaml")
    .or_else(|_| load_config("config/token-service.yaml"))
    .expect("failed to load config.yaml");

// after
let cfg: TokenServerConfig = common_config::Loader::new("TOKEN")
    .with_file("config/config.yaml")
    .with_config_service(
        std::env::var("CONFIG_SERVICE_URL").unwrap_or_else(|_| "http://localhost:6006".into()),
        std::env::var("CONFIG_SERVICE_NAMESPACE").unwrap_or_else(|_| "token-service-dev".into()),
    )
    .load()
    .expect("failed to load config");
```

> 注意：token-service 现有的 `TokenServerConfig` 使用嵌套 struct（`server.host` / `database.url` / `consul.addr`），
> config-service 返回的是 flat key-value（`{"host": "0.0.0.0", "port": 4000, ...}`）。
> 需要让 config-service 的 namespace 里存的 key 与 serde field name 对齐。
> 解决方案：在 config-service 里建 namespace 时，key 用 `server.host` / `server.port` 等嵌套路径。

## 6. 测试计划

### 6.1 单元测试 `libs/common-config/tests/provider_test.rs`

```rust
// 用 wiremock 起一个 mock HTTP server 返回 {"key": "value"}
// 验证：
//   1. fetch 成功 → HashMap
//   2. fetch 404 → Err
//   3. fetch 非 JSON → Err
//   4. fetch 超时 → Err（不 panic）
```

### 6.2 集成测试 `services/token-service/tests/config_provider_test.rs`

```rust
// 起 config-service（testcontainers MySQL）+ token-service
// 往 config-service 写入 token-service-dev namespace 的 config items
// 用 token-service 的 Loader 以 config-service feature 加载
// 验证 config 值与预期一致
```

### 6.3 E2E `test-case/65-config-service-dogfood.md`

```text
1. 起 config-service + token-service
2. 在 config-service 建 namespace token-service-dev
3. 写入 token 配置 items（server.host / database.url 等）
4. 重启 token-service（带 CONFIG_SERVICE_URL + CONFIG_SERVICE_NAMESPACE）
5. 验证 token-service 正常启动，配置来自 config-service
6. 改 config-service 里的一个值（如 default_access_ttl_sec）
7. 等 30s 轮询，验证 token-service HotConfig 已更新
```

## 7. 实施步骤（plan 阶段细化）

1. `libs/common-config/Cargo.toml` 加 `config-service` feature + `reqwest` optional dep。
2. 新建 `libs/common-config/src/provider.rs`，实现 `ConfigServiceProvider`。
3. `libs/common-config/src/lib.rs`：`Loader` 加 `with_config_service` builder + `load` 内集成 provider source + `watch` 方法。
4. 加单元测试 `libs/common-config/tests/provider_test.rs`（wiremock 3 case）。
5. 改 `services/token-service/Cargo.toml` + `main.rs`，用 config-service feature。
6. 在 config-service 手工建 `token-service-dev` namespace + 写入 token 配置 items。
7. 加集成测试 `services/token-service/tests/config_provider_test.rs`。
8. 加 e2e 手工验收 `test-case/65-config-service-dogfood.md`。

## 8. 验收

- [ ] `libs/common-config` 编译通过 `--features config-service` 与 `--no-default-features`。
- [ ] wiremock 单元测试 3 case 全绿。
- [ ] token-service 在 config-service 可用时从 HTTP 拉取配置启动。
- [ ] token-service 在 config-service 不可用时回退到本地 yaml 启动。
- [ ] 30s 轮询热更新验证：改 config-service → token-service HotConfig 更新。
- [ ] test-case/65 手工验收全部通过。

## 9. 风险与 mitigation

| 风险 | 缓解 |
|---|---|
| config-service 故障导致全集群不可用 | 本地 yaml 兜底 + fallback 语义 |
| config-service 返回的 key 与 serde field name 不对齐 | namespace 建时 key 用 `server.host` / `database.url` 嵌套路径 |
| HotConfig watch 的 diff 误触发 | 全量 JSON 比较 + 只在有变化时 update |
| 30s 轮询间隔太短/太长 | 留作环境变量 `CONFIG_POLL_INTERVAL_SEC`，默认 30 |
| config-service 自身配置也需要配置中心（鸡生蛋） | config-service 继续读本地 yaml，不做自举（Phase 6-C 再说） |
| wiremock 不在 workspace 依赖里 | 加 `[dev-dependencies] wiremock = "0.5"` |
