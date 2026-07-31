# Rust 接入指南

> 用 Rust 应用接入本微服务系统（api-gateway + iam-service + token-service + 其它业务服务）的完整说明。
> 最近更新：2026-07-17

本文档目标读者：

- 在另一个 Rust 应用（CLI / 后端服务 / 集成工具）里**作为客户端**调用本平台 API 的开发者
- 想直接对接 `api-gateway` 暴露的 HTTP 接口（登录、拿 JWT、CRUD 资源）但又不想手写 reqwest + serde + token 缓存样板
- 想理解 Rust SDK 与 Java SDK 的差异、服务内部通信与外部 SDK 的边界的同学

> ℹ️ **关于 `sdk/rust/`**：1.0 起 6 个 service crate 已就位（iam / token / audit / file / notification / config），共享 `kudi-http-client` 底座。直接依赖 `sdk/rust/<svc>-sdk` 即可，不必再按本文示例手写 reqwest。详见文末「§ 8 SDK 现状」。

---

## 1. 平台总览

```
┌────────────────────────────────────────────────────────┐
│ 你的 Rust 应用 (CLI / 后端服务 / 集成工具)             │
└──────────────┬─────────────────────────────────────────┘
               │  HTTPS (统一 baseUrl)
               ▼
        ┌─────────────────┐
        │  api-gateway    │  :3000
        │  JWT 校验 / 限流 │
        │  路由 / Trace 注入│
        └────────┬────────┘
                 │ (Consul 服务发现)
   ┌─────────────┼─────────────────────────────┐
   ▼             ▼             ▼               ▼
┌─────────┐ ┌──────────┐ ┌──────────────┐ ┌──────────┐
│ iam-svc │ │token-svc │ │notification  │ │ audit / │
│ :6002   │ │ :6001    │ │ :6003        │ │ file ... │
└─────────┘ └──────────┘ └──────────────┘ └──────────┘
```

**重要约定**：

- 所有外部调用统一走 `api-gateway` 的**单一 baseUrl**（生产环境一般是 `https://gw.example.com`，本地是 `http://localhost:3000`）。
- 不要直接调下游服务端口（6001–6007）—— 那是内网端口，外部不可达且不带网关的限流、追踪、JWT 校验。
- 所有受保护接口要求 `Authorization: Bearer <jwt>`。JWT 由 iam-service 签发。
- 每次响应都带 `X-Trace-Id` header，用于跨服务排障。

> ℹ️ 想了解网关路由配置 / 限流策略 / TraceContext 串联机制，参见 [`./java-integration`](./java-integration)（通用部分三个语言完全一样，本文档只讲 Rust SDK 本身）。

---

## 2. SDK 套件速查

`sdk/rust/` 共 7 个 crate，**全部是 Rust workspace 成员**：

| Crate | 用途 | 端点数（已包装） |
|---|---|---|
| `kudi-http-client` | 共享底座：HTTP + token 缓存 + Bearer passthrough + `client_credentials` + 错误映射 | — |
| `iam-service-sdk` | `iam-service`：登录、用户 / 角色 / 权限 / 组织单元管理 | 11 |
| `token-service-sdk` | `token-service`：OAuth client / token 端点 / API Key | 5 |
| `audit-service-sdk` | `audit-service`：登录 / 操作日志查询 + 留存策略 | 6 |
| `file-service-sdk` | `file-service`：presigned URL + 元数据查询（multipart upload / download 走 presign URL 由调用方直接 PUT/GET） | 3 |
| `notification-service-sdk` | `notification-service`：公告 CRUD + 发布 / 撤销 + 未读数 | 5 |
| `config-service-sdk` | `config-service`：namespace + item CRUD（SSE watch 不包装） | 4 |

vs Java SDK（`sdk/java/`）的对照：

- **crate 数**：Java 8 个 module（按 service + 场景拆开 user/client 两个变体）；Rust 6 个 service crate（用 Builder 切换 user / client_credentials 场景，因为 `kudi-http-client` 同时支持两种模式）。
- **代码生成方式**：Java 用 `openapi-generator` 骨架 + 手写凭证层；Rust 端点路径与模型**直接从 `services/<svc>/openapi.yaml`（utoipa 自动生成）手写翻译**，不跑 `openapi-generator`（产物质量差，与 Java 实践一致：最终都是手写）。
- **端点路径源**：以 `services/<svc>/openapi.yaml` 为单一事实源。**Java SDK 的 `/v1/audit/...` 等路径已过期（与现网服务不符），Rust 端不照搬**。
- **测试**：Java 用 `MockWebServer`；Rust 用 `wiremock`。
- **错误模型**：两端都按 multi-lang SDK design spec §9 的 5 种 `SdkError` variant（`Auth` / `Validation` / `RateLimit` / `Server` / `Network`）。

---

## 3. 鉴权流程

### 3.1 终端用户（username + password）

```rust
use digua_iam_sdk::IamClient;

let (client, login) = IamClient::login(
    "https://gw.example.com",
    "alice",
    "secret",
).await?;

// login.access_token 已缓存在 client 内部，每次调用自动带 Bearer
let page = client.list_users(
    &digua_iam_sdk::ListUsersQuery::new().page(0).size(20)
).await?;
```

底层 SDK 帮你做了：

1. 第一次 `login()` POST `/iam/auth/login`，拿到 `access_token` + `refresh_token` + 用户信息。
2. 把 `access_token` 闭包存进 `client.bearer()`（每次请求调一次拿当前 token）。
3. 后续业务调用自动带 `Authorization: Bearer <access_token>`。
4. 401 时 SDK 返回 `SdkError::Auth { status: 401, ... }`，**不**自动 refresh（让调用方决定怎么处理 refresh）。
5. token 快过期 → 调用方显式 `client.refresh(refresh_token).await?` 拿新一对。

### 3.2 服务间（client_credentials）

```rust
use digua_http_client::KudiWebClient;
use digua_http_client::error::KudiHttpError;

let http = KudiWebClient::builder()
    .credential("contract-svc", "secret-111")
    .token_endpoint("https://gw.example.com/v1/tokens/access")
    .build()?;

// 第一次会自动调 token 端点换 token，后续请求自动带 Bearer + 提前 60s 主动刷新
let resp = http.get("https://iam-svc.iam/v1/iam/users", None).await?;
```

底层 `KudiWebClient` 内置 DashMap 共享 token 缓存（跨 `KudiWebClient` 实例，按 `SHA256(client_id | secret_key)` 分桶）。收到 401 后调 `client.force_refresh().await?` 让下个请求重取。

### 3.3 Bearer 透传（已有 user JWT）

当你**已经有**用户 JWT（比如从其它模块签发或上一层拿过来），不想让 SDK 自己管 token：

```rust
use std::sync::{Arc, Mutex};
use digua_iam_sdk::IamClient;

let token = Arc::new(Mutex::new(Some("user.jwt".into())));
let client = IamClient::new("https://gw.example.com", move || {
    token.lock().unwrap().clone()
});

// 调用方自己管 token 生命周期（包括过期后换新 + 注入回 Arc<Mutex>)
client.logout().await?;
```

`Fn() -> Option<String> + Send + Sync + 'static` 闭包签名，与 Java `Supplier<String>` 对齐。线程安全由调用方保证（用 `Arc<Mutex<>>` / `ArcSwap` 等）。

---

## 4. 重要边界：服务内部 ≠ 外部 SDK

⚠️ **SDK 是给"你的 Rust 应用"调平台用的，不是给 Rust 服务之间互调用**。

| 场景 | 工具 | 认证 |
|---|---|---|
| 你的 Rust 应用 → 平台（跨网络边界） | `sdk/rust/<svc>-sdk` | user JWT / client_credentials |
| 平台 Rust 服务 A → 服务 B（集群内） | `libs/digua_common-http::ResilientClient` | service-jwt（aud=目标服务） |

详细原因：

- **认证模型不同**：SDK 走 OAuth（`/v1/tokens/access` 换 token + cache）；服务内部走 `digua_common_jwt::ServiceJwtCache`（平台内部签发，不换 token）。
- **网络路径不同**：SDK 走公网 api-gateway；服务内部走 Consul 直连 `/<svc>/internal/...` internal 端点。
- **弹性需求不同**：SDK 自带简单 retry；服务内部 `ResilientClient` 带完整熔断器 + overall deadline + Prometheus observer。
- **架构方向**：项目正在用 RabbitMQ + transactional outbox 替代 service 间同步 HTTP（详见 `docs-site/guides/architecture-notes` 暂无独立条目，参见 [`docs-site/decisions/ADR-002-service-standard-baseline`](../../docs-site/decisions/ADR-002-service-standard-baseline) 与 `TODO.md` Phase 4/5B），引入 SDK 做 service 间调用**南辕北辙**。

如果你的需求是「**我也是平台内的一个 Rust 服务，要调其它 service 的 internal 端点**」，请用 `libs/digua_common-http::ResilientClient` + `digua_common_jwt::ServiceJwtCache`，**不要**用 SDK。

---

## 5. 完整使用示例（iam-service-sdk）

`Cargo.toml`：

```toml
[dependencies]
iam-service-sdk = { path = "sdk/rust/iam-service-sdk" }
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

`src/main.rs`：

```rust
use digua_iam_sdk::{IamClient, ListUsersQuery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 密码登录
    let (client, login) = IamClient::login(
        "https://gw.example.com",
        "alice",
        "secret",
    ).await?;
    println!("logged in as {} ({})", login.user.username, login.user.id);

    // 2. 列用户
    let page = client.list_users(
        &ListUsersQuery::new().keyword("alice").page(0).size(20)
    ).await?;
    for u in &page.items {
        println!("  {} {} ({})", u.id, u.username,
                 u.status.as_deref().unwrap_or("?"));
    }

    // 3. 创建用户（需 admin 权限）
    let new_user = client.create_user(&digua_iam_sdk::CreateUserRequest {
        username: "bob".into(),
        password: "secret".into(),
        nickname: "Bob".into(),
        email: Some("bob@example.com".into()),
        phone: None,
        avatar: None,
        tenant_slug: None,
    }).await?;
    println!("created user {}", new_user.id);

    // 4. 登出
    client.logout().await?;
    Ok(())
}
```

更多示例（错误处理、async 上下文、retry）参见各 crate 的 `tests/client_test.rs`：

- `sdk/rust/iam-service-sdk/tests/client_test.rs`（11 个 wiremock 测试，含 401 → `SdkError::Auth`）
- `sdk/rust/audit-service-sdk/tests/client_test.rs`（6 个）
- `sdk/rust/token-service-sdk/tests/client_test.rs`（6 个）
- `sdk/rust/file-service-sdk/tests/client_test.rs`（4 个）
- `sdk/rust/notification-service-sdk/tests/client_test.rs`（5 个）
- `sdk/rust/config-service-sdk/tests/client_test.rs`（5 个）

---

## 6. 错误模型

所有方法返回 `Result<T, digua_http_client::SdkError>`（在每个 crate 里 re-export 为 `<crate>::SdkError`）。五种 variant 严格按 multi-lang SDK design spec §9：

| HTTP | Error variant | 含义 |
|---|---|---|
| 401 / 403 | `SdkError::Auth { status, message, details }` | token 缺失 / 过期 / 格式错 |
| 429 | `SdkError::RateLimit { message, details }` | 命中网关限流，看 `Retry-After` |
| 400 / 422 | `SdkError::Validation { status, message, details }` | body / param 不合法 |
| 5xx | `SdkError::Server { status, message, details }` | 上游服务故障 |
| 网络 / 超时 / 反序列化失败 | `SdkError::Network(String)` | reqwest 传输错 / serde 反序列化错 |

调用方代码模式：

```rust
match client.get_user("u-1").await {
    Ok(user) => println!("{}", user.username),
    Err(SdkError::Auth { status, .. }) if status == 401 => {
        // 401 → 强制 refresh + 重试一次
        client.force_refresh().await?;
        // 再调一次
    }
    Err(SdkError::RateLimit { .. }) => {
        // 退避后重试
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    Err(e) => return Err(e.into()),
}
```

> 各 SDK 内已通过 wiremock 测试覆盖 401 → `SdkError::Auth` 与典型错误路径，但**不自动 retry / refresh**——这是设计选择：让调用方拿到错误后能根据业务决定（refresh / 退避 / 放弃）。

---

## 7. 端点速查（已包装）

完整方法列表见各 crate 的 README（`sdk/rust/<svc>-sdk/README.md`）。下面是高频入口。

### iam-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `IamClient::login(base, user, pwd)` | POST | `/iam/auth/login` |
| `client.refresh(refresh_token)` | POST | `/iam/auth/refresh` |
| `client.logout()` | POST | `/iam/auth/logout` |
| `client.change_password(old, new)` | POST | `/iam/auth/password` |
| `client.list_users(&ListUsersQuery)` | GET | `/iam/users` |
| `client.get_user(id)` | GET | `/iam/users/{id}` |
| `client.create_user(&CreateUserRequest)` | POST | `/iam/users` |
| `client.update_user(id, &UpdateUserRequest)` | POST | `/iam/users/{id}` |
| `client.list_roles(&ListRolesQuery)` | GET | `/iam/roles` |
| `client.get_role(id)` | GET | `/iam/roles/{id}` |
| `client.create_role(&CreateRoleRequest)` | POST | `/iam/roles` |
| `client.update_role(id, &UpdateRoleRequest)` | POST | `/iam/roles/{id}` |
| `client.list_permissions()` | GET | `/iam/permissions` |

### token-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `client.access(&AccessRequest)` | POST | `/tokens/access` |
| `client.refresh(&RefreshRequest)` | POST | `/tokens/refresh` |
| `client.revoke(&RevokeRequest)` | POST | `/tokens/revoke` |
| `client.list_api_keys(&ListApiKeysQuery)` | GET | `/api-keys` |
| `client.create_api_key(&CreateApiKeyRequest)` | POST | `/api-keys` |

### audit-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `client.query_login_logs(&LoginLogQuery)` | GET | `/audit/login` |
| `client.query_operation_logs(&OperationLogQuery)` | GET | `/audit/operation` |
| `client.trigger_cleanup()` | POST | `/audit/retention/cleanup` |
| `client.get_retention_policy()` | GET | `/audit/retention/policy` |
| `client.update_retention_policy(&RetentionPolicy)` | POST | `/audit/retention/policy` |

### file-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `client.create_presign(&CreatePresignUrl)` | POST | `/file/presign` |
| `client.list_files(&ListFilesQuery)` | GET | `/file/list` |
| `client.get_file_info(id)` | GET | `/file/{id}/info` |

> 故意未包装：multipart upload (`POST /file/upload`) 与 binary download (`GET /file/{id}`) — 这两条路径走字节流，`kudi-http-client` 的 `HttpResponse` 是 text-only，强行包装会丢字节。正确做法：先 `create_presign` 拿到 storage URL，调用方用任何 HTTP client（reqwest / curl / aws-sdk）直接 PUT/GET 到 URL。

### notification-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `client.list_announcements(&ListAnnouncementsQuery)` | GET | `/notification/announcements` |
| `client.get_announcement(id)` | GET | `/notification/announcements/{id}` |
| `client.create_announcement(&CreateAnnouncement)` | POST | `/notification/announcements` |
| `client.publish(id)` | POST | `/notification/announcements/{id}/publish` |
| `client.unread_count()` | GET | `/notification/unread-count` |

> 故意未包装：`POST /notification/announcements/{id}/read`、`/notification/ws` WebSocket。前者按需扩展；后者走 `tokio-tungstenite` 直连 URL，不在 SDK 文本响应模型里。

### config-service-sdk

| 方法 | HTTP | Path |
|---|---|---|
| `client.list_namespaces(&ListNamespacesQuery)` | GET | `/config/namespace` |
| `client.create_namespace(&CreateNamespace)` | POST | `/config/namespace` |
| `client.list_items(&ListItemsQuery)` | GET | `/config/item` |
| `client.create_item(&CreateConfigItem)` | POST | `/config/item` |

> 故意未包装：`PUT/GET /config/namespace/{id}`、`PUT/GET /config/item/{id}`、`GET /config/watch`（SSE）、`/config/export|import/{id}`。SSE 用 `eventsource-client`，update / export / import 按模板扩展。

> ⚠️ **所有路径以 `services/<svc>/openapi.yaml` 为事实源**——utoipa 自动 dump，CI 检查 drift（`git diff --exit-code -- 'services/*/openapi.yaml'`）。Service 改 routes 时 SDK 会**自动失效**，CI 会拒 push，需同步扩展 SDK 或回滚 route。

---

## 8. SDK 现状与扩展指南

### 8.1 当前状态

- 6 个 service crate + 共享底座 `kudi-http-client`，全部在 Cargo workspace 内。
- 端点路径与 `services/<svc>/openapi.yaml` 一一对齐（25/25 OK，2026-07-17 重 dump 验证零 drift）。
- 单元 + doctest 总计 **48 个测试全过**。
- 已合并 main（commit `f26dbf0` + drift fix `3a14444`）。
- 内部 cargo registry 发布：**未做**，当前用 `path = "..."` 依赖。

### 8.2 扩展一个新端点

以「在 audit-service-sdk 加 `export_login_csv`」为例：

1. 读 `services/audit-service/openapi.yaml` 找到 `GET /audit/export/compliance/login/csv/{user_id}` 的 parameters + response。
2. 在 `sdk/rust/audit-service-sdk/src/model.rs` 加 response struct（如 `LoginCsvExport { user_id: String, csv: String }`）— 字段对应 schema 的 required/optional。
3. 在 `sdk/rust/audit-service-sdk/src/client.rs` 加方法：

   ```rust
   pub async fn export_login_csv(&self, user_id: &str) -> Result<String, SdkError> {
       let url = format!("{}/audit/export/compliance/login/csv/{}",
                        self.base_url, url_encode_path(user_id));
       let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
       ensure_success(&resp, "audit export_login_csv")?;
       Ok(resp.body)
   }
   ```

4. 在 `tests/client_test.rs` 加 wiremock 测试覆盖 request shape + 响应反序列化 + 401 → `SdkError::Auth`。
5. 在 `sdk/rust/audit-service-sdk/README.md` 的「Methods」表格追加一行。
6. `cargo test -p audit-service-sdk` + `cargo build --workspace` 验证。
7. commit + push。

### 8.3 共享底座 `kudi-http-client` 的扩展点

当前暴露给 service SDK 用的 API：

| Symbol | 用途 |
|---|---|
| `KudiWebClient` | 共享 reqwest 实例 |
| `KudiWebClient::builder().credential(client_id, secret_key)` | `client_credentials` 自动换 token |
| `KudiWebClient::builder().base_url(url)` | 把 base URL 存进 client（service SDK 用 `client.base_url()` 取） |
| `KudiWebClient::get(url, bearer)` / `post(url, body, bearer)` / `get_with_bearer(url, bearer)` | 三个公开调用方法 |
| `KudiWebClient::force_refresh()` | 401 后主动让 token cache 失效 |
| `KudiWebClient::cache_snapshot()` | 调试用，token cache 当前大小 |
| `SdkError` (= `KudiHttpError` alias) | 错误类型，5 种 variant |

需要时可在不破坏向后兼容的前提下加 `delete` / `post_multipart` 方法，扩展 `HttpTransport` trait 加新 method（所有现有实现 + Mock 同步加）。

---

## 9. 常见问题

| 现象 | 可能原因 | 处理 |
|------|----------|------|
| 登录 `401 Unauthorized` | 用户名 / 密码错，或 `X-Tenant-Id` 与实际 tenant 不符 | 检查 IAM 用户存在 + tenant_slug 正确 |
| 调业务接口 `401` 但 token 刚拿到 | token 解析失败 / aud 不匹配 | 检查 `aud` claim（多租户场景下 iam-service 用 `tid` claim 区分） |
| `429 Too Many Requests` | 命中网关限流 | 看 `Retry-After` header 退避；详见 [`./java-integration`](./java-integration) §6.3 与 [`../user-manual/07-rate-limiting`](../user-manual/07-rate-limiting) |
| 调用偶发 `502 / 504` | 下游服务被 Consul 暂时摘除 | 加重试（指数退避，2–3 次） |
| SDK 编译通过但 `cargo test -p xxx-sdk` 失败 | `CARGO_TARGET_DIR` 设到 tmpfs 满了 | 用独立 target dir：`CARGO_TARGET_DIR=/tmp/sdk-target cargo test -p xxx-sdk` |
| 想看完整调用链路 | — | 拿响应里的 `X-Trace-Id`，到 Jaeger UI（http://localhost:16686）搜 |
| 想本地起完整平台联调 | — | `docker compose -f infrastructure/docker-compose.yml up -d` |
| 连接超时（本地） | 基础设施没启动 | `pnpm run check:env` 或 `docker ps` |

---

## 10. 参考

- [`sdk/rust/README.md`](../../sdk/rust/README.md) — 顶层 SDK 总览（crate 矩阵 + install / build / extend）
- [`sdk/rust/<svc>-sdk/README.md`](../../sdk/rust/) — 每个 service crate 的方法表 + 错误 + bearer supplier 用法
- [`./java-integration`](./java-integration) — 通用平台架构、鉴权、限流、Trace、FAQ（三个语言通用）
- [`./api-gateway-routing`](./api-gateway-routing) — 网关路由配置
- [`./otel-tracing`](./otel-tracing) — 跨服务 trace 机制
- [`./commit-convention`](./commit-convention) — commit message 规范（feat(sdk-rust) / fix(contracts)）
- [`services/<svc>/openapi.yaml`](../../services) — SDK 端点路径的唯一事实源
- `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` — 多语言 SDK 设计 spec
- `TODO.md` Phase 9.5 — 实施记录与显式 out-of-scope 项