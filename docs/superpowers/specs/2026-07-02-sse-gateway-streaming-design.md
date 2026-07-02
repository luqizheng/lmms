# api-gateway SSE / WebSocket streaming 支持 — 设计

**日期**：2026-07-02
**目标**：激活 `frontend_todo.md` §3「网关拓扑决策点」中标记暂缓的 `/v1/config/watch` SSE 端到端。

## 背景

- 配置中心 config-service 已实现 `GET /config/watch`（SSE，services/config-service/src/routes/watch.rs）。
- 前端已实现 `EventSource` 占位（frontend/src/api/config/sse.ts，禁用 stub）。
- api-gateway **当前对所有上游响应做全量 buffer**（libs/common-http/src/resilient.rs:124 `body_bytes = resp.bytes().await`，再 `Body::from(resp.body)`），与 SSE 语义不兼容。
- `RouteConfig.ws: bool` 字段已存在（services/api-gateway/src/config.rs:9），但运行时未读。
- EventSource 协议层不支持自定义 header，token 必须走 `?token=<jwt>` query 参数；而 `auth_middleware` 仅看 `Authorization: Bearer`（services/api-gateway/src/middleware/mod.rs:40）。

## 设计

### 1. 库层：`ResilientClient::call_streaming()`

**文件**：`libs/common-http/src/resilient.rs`

新增方法，复用 breaker/timeout/retry，但**不读 body**：

```rust
pub struct UpstreamStream {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: reqwest::Response, // 仍活、未消费 body
}

pub async fn call_streaming(
    &self,
    service: &str,
    method: Method,
    url: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<UpstreamStream, ResilientError>
```

与现有 `call()` 区别：
- 失败路径（breaker / connect / 单次超时）共用同一重试逻辑；
- 成功路径在拿到 `reqwest::Response` 后**立即返回**，调用方负责 body 消费；
- 5xx/429 仍记录到 breaker（用 `status` 即可，不依赖 body）；
- 不再有 retry-on-status（SSE 一旦开始流无法 replay，header-only retry 不语义）。

### 2. 路由层：`forward_to_service` 加 ws 分支

**文件**：`services/api-gateway/src/router.rs`

签名扩展：

```rust
pub async fn forward_to_service(
    cfg: Arc<RouterConfig>,
    request: Request<Body>,
    service_name: &str,
    target_path_query: &str,
    ws: bool,
) -> Response
```

- `ws=false`：现有路径，buffer 后 `Body::from(resp.body)`。
- `ws=true`：调 `call_streaming()`，把 `reqwest::Response::bytes_stream()` 用 `Body::from_stream` 包成 axum streaming body，保留 status / headers（剔除 hop-by-hop：`transfer-encoding`、`connection`）。

`find_and_render` 调用处（services/api-gateway/src/main.rs:421）传 `route.ws` 进入 forward。

### 3. 鉴权：`auth_middleware` 支持 ?token= query

**文件**：`services/api-gateway/src/middleware/mod.rs`

当前 token 提取逻辑：

```rust
let token = request.headers().get("Authorization")...
```

新逻辑：

```rust
let token = request.headers()
    .get("Authorization")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .map(|s| s.to_string())
    .or_else(|| {
        // fallback：query ?token=  用于 ws/sse 路径
        request.uri().query()
            .and_then(|q| parse_token_query(q))
    });
```

辅助函数 `parse_token_query(q: &str) -> Option<String>` 用 `url::form_urlencoded` 解 `q`，匹配 `token=`。

**适用范围**：所有非 public 路由都生效。风险：query 出现在 access log，需在生产层加访问日志过滤；本期不阻断。

### 4. 路由表：加 `/v1/config/watch`

**文件**：`infrastructure/config/api-gateway/routes/config.router.toml`

```toml
[[route]]
upstream   = "/v1/config/watch"
downstream = "/config/watch"
service    = "config-service"
ws         = true
```

`ws=true` 标记路由层 streaming + auth_middleware 走 query token fallback。

### 5. 前端：激活 EventSource stub

**文件**：`frontend/src/api/config/sse.ts`

取消注释块：

```ts
const token = storageLocal().getItem<string>(TokenKey);
const url = `${wsBaseURL()}/v1/config/watch?token=${encodeURIComponent(formatToken(token ?? "") ?? "")}`;
const es = new EventSource(url);
es.addEventListener("config.item.changed", ev => {
  try { opts.onEvent(JSON.parse((ev as MessageEvent).data)); } catch (e) { opts.onError?.(e); }
});
es.onerror = ev => { opts.onError?.(ev); };
return () => { es.close(); };
```

`isSseReady()` 同步返回 `true`。

## 风险 & 约束

| 项 | 说明 | 缓解 |
|---|---|---|
| Body 不读 → breaker 5xx 判别仍准确 | `reqwest::Response.status()` 在 `bytes_stream()` 消费前可用 | 维持现状 breaker 计 502/503/429 |
| SSE 一旦开始无法 retry | 流式不可 replay | 在 `call_streaming()` 中**禁用** status-based retry（仅保留 connect/timeout 重试） |
| Token 出现在 URL query | 浏览器历史、proxy access log 可能落盘 | access log 过滤留作后续 ticket；本期仅 SSE/watch 一条路由，范围可控 |
| EventSource 自动重连 | 服务端断流时浏览器自动重连 + Last-Event-ID 协议 | 配置服务端支持 `Last-Event-ID`（config-service `watch.rs` 现状） |
| Auth path 双重来源（header + query） | CSRF 风险 | ws 路由仅暴露给同源前端；本期仅 `/v1/config/watch` 一条 |
| ResilientClient 测试覆盖 | 新方法需单测 | `libs/common-http/tests/resilient_streaming_test.rs`：mock 服务器返回分块 body，验证 chunk 流式回到 caller |

## 实施步骤

1. 开新分支 `feat/sse-gateway-streaming`（从 `main` 基线）
2. `libs/common-http/src/resilient.rs` 加 `UpstreamStream` + `call_streaming()` + 单测
3. `services/api-gateway/src/router.rs` 加 `ws` 分支
4. `services/api-gateway/src/main.rs` `find_and_render` 调用处传 `route.ws`
5. `services/api-gateway/src/middleware/mod.rs` query-token fallback
6. `infrastructure/config/api-gateway/routes/config.router.toml` 加 `/v1/config/watch` 行
7. `frontend/src/api/config/sse.ts` 取消注释
8. `cd services/api-gateway && cargo build` / `cargo test`
9. `cd libs/common-http && cargo test`
10. 提交 2 个 commit（lib + gateway+route+frontend），push，开 MR

## 验收

- `cargo test` 全过（lib + gateway）
- `scripts/check-spec.js` ✓
- 端到端（在容器栈 + browser）：打开 `/config/item`，点击 namespace，server 端推 `config.item.changed` 事件 → 前端 store 收到 → 列表自动刷新（可选手工验证 — 本期不在 e2e 范围）