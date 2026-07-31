# kudi-http-client 多语言 SDK 设计

日期：2026-06-11
状态：待用户审查
作者：brainstorming 会话

## 1. 目标

在 `sdk/` 下新增三语言通用 HTTP 客户端 `kudi-http-client`，用于**微服务之间**的 HTTP 调用：

- 调用方可以是后端服务、批处理任务、定时作业、CLI 工具
- 既支持以**服务票据**（client_credentials）方式调下游，也支持**显式透传用户 Bearer**
- 三语言 API 形态、错误模型、缓存语义对齐

定位区别于已有 `KudiCoder.Sdk.Common`：

| 维度 | 已有 `KudiCoder.Sdk.Common` | 新 `kudi-http-client` |
|------|-----------------------------|------------------------|
| 调用对象 | 特定服务 SDK（user/token）| 任意外部 URL |
| 调用方 | 终端应用 | 微服务/批处理/CLI |
| 凭证模式 | 仅登录 + token 缓存 | 透传 Bearer **或** 缓存 client_credentials |
| URL 形态 | 由各 client SDK 内部拼路径 | 调用方传完整 URL |

## 2. 范围

包含：

- .NET 包 `KudiCoder.HttpClient`（多目标：`net6.0;net9.0;net10.0`）
- Java 包 `com.kudi.coder:kudi-http-client`（多目标：8/17/21）
- Rust crate `kudi-http-client`（Rust 2021 edition，依赖 `reqwest + tokio + serde`）
- 同步 + 异步双 API（.NET 同步即异步包装，Java 同步 + CompletableFuture，Rust 仅异步）
- `(ClientId, SecretKey)` 维度 token 缓存与自动刷新
- 401 自动失效重取 + 一次重试
- README + 示例

不含：

- 重写或迁移 `KudiCoder.Sdk.Common` / 任何现有 `*.ClientSdk` / `*.UserSdk`
- 服务端（`token-service` / `api-gateway` / 各业务服务）改造
- W3C trace 透传、OpenTelemetry 集成
- 服务发现 / 负载均衡（URL 由调用方传完整 URL）
- 浏览器 SDK、前端 SDK
- Java/.NET/Rust 之外的语言（Node、Go、Python 等）

## 3. 决策摘要

| 项 | 决策 |
|----|------|
| 命名空间 | .NET `KudiCoder.HttpClient` / Java `com.kudi.coder.httpclient` / Rust `digua_http_client` |
| 类名 | `KudiWebClient`（沿用用户原话） |
| 依赖底座 | .NET `HttpClient` / Java `java.net.http.HttpClient`（8 走 `HttpURLConnection`）/ Rust `reqwest` |
| 服务票据获取 | 复用 `token-service` 现有 `POST /api/tokens/access`，`grant_type=client_credentials` |
| Token 缓存 key | `SHA256(ClientId \| SecretKey)` 哈希 |
| 缓存粒度 | 按 `(ClientId, SecretKey)` 维度独立缓存 |
| 提前刷新 | 过期前 60s |
| 401 处理 | 清缓存 → 重 fetch → 重发原请求一次 |
| 重试 | 5xx 与网络异常最多 1 次（可配），4xx 不重试 |
| 错误模型 | 与现有 SDK 同构（`Auth` / `Server` / `Network` / `Validation`） |
| 异步风格 | .NET `Task` / Java `CompletableFuture` / Rust `async/await` |
| 持久化 | 不做，缓存仅进程内 |
| 发布渠道 | 与现有 SDK 一致：私有 Maven / NuGet；Rust crate 入 workspace（暂不发布到 crates.io） |

## 4. 目录结构

```
sdk/
  dotnet/
    KudiCoder.HttpClient/
      KudiCoder.HttpClient.csproj
      KudiWebClient.cs
      KudiHttpOptions.cs
      HttpRequest.cs            # 或放 Internal/
      HttpResponse.cs
      ServiceTokenCache.cs
      ClientCredentialsFetcher.cs
      Exceptions/
        KudiHttpException.cs
        AuthException.cs
        NetworkException.cs
        ServerException.cs
        ValidationException.cs
      Models/
        TokenInfo.cs
      README.md
    KudiCoder.sln               # 加入新项目
    tests/
      KudiCoder.HttpClient.Tests/
        KudiCoder.HttpClient.Tests.csproj
        ServiceTokenCacheTests.cs
        KudiWebClientTests.cs
  java/
    kudi-http-client/
      pom.xml
      src/main/java/com/kudi/coder/httpclient/
        KudiWebClient.java
        KudiHttpOptions.java
        ServiceTokenCache.java
        ClientCredentialsFetcher.java
        HttpTransport.java
        Internal/
          HttpRequest.java
          HttpResponse.java
        Exceptions/
          KudiHttpException.java
          AuthException.java
          NetworkException.java
          ServerException.java
          ValidationException.java
        Models/
          TokenInfo.java
      src/test/java/com/kudi/coder/httpclient/
        ServiceTokenCacheTest.java
        KudiWebClientTest.java
      README.md
    pom.xml                     # 加入新模块
  rust/
    kudi-http-client/
      Cargo.toml
      src/
        lib.rs
        client.rs               # KudiWebClient
        options.rs              # KudiHttpOptions
        cache.rs                # ServiceTokenCache
        fetcher.rs              # ClientCredentialsFetcher
        transport.rs            # HttpTransport（trait + reqwest 实现）
        error.rs                # KudiHttpError + 子类型
        models.rs               # TokenInfo
        request.rs              # HttpRequest
        response.rs             # HttpResponse
      tests/
        cache_test.rs
        client_test.rs
      README.md
```

Rust 端首次进入 `sdk/`，与 `sdk/dotnet`、`sdk/java` 对称；`Cargo.toml` 在仓库根，`kudi-http-client` 作为 workspace member 加入。

## 5. 命名与坐标

| 语言 | 标识 |
|------|------|
| .NET | packageId `KudiCoder.HttpClient`，RootNamespace `KudiCoder.HttpClient`，TargetFrameworks `net6.0;net9.0;net10.0` |
| Java | groupId `com.kudi.coder`，artifactId `kudi-http-client`，`maven.compiler.release` profile 8/17/21 |
| Rust | crate `kudi-http-client`，库名 `digua_http_client`，edition 2021 |

版本号随 monorepo tag（`0.1.0` 起）。

## 6. 核心 API 形态

### .NET

```csharp
using KudiCoder.HttpClient;

// 创建一次，进程内可重用
var http = new KudiWebClient()
    .WithCredential(clientId: "合同系统OrKey", secretKey: "111")
    .WithTokenEndpoint("http://token-svc/api/tokens/access")
    .WithReadTimeoutMs(15_000);

// 方式 A：service-to-service，自动用缓存的 service token
var resp = await http.PostAsync("http://user-svc/api/users/123",
    new { name = "x" });                          // 自动 JSON 序列化
int status = resp.StatusCode;
string body = resp.BodyAsString();
T obj = resp.BodyAs<T>();

// 方式 B：透传用户 token（来自请求上下文）
var userJwt = ctx.UserJwt;                        // 由 axum/actix/web 抽取
await http.GetAsync("http://user-svc/api/users/me",
    bearer: userJwt);

// 读取/释放
var opts = http.GetOptions();
await http.DisposeAsync();
```

### Java

```java
KudiWebClient http = KudiWebClient.builder()
    .credential("合同系统OrKey", "111")
    .tokenEndpoint("http://token-svc/api/tokens/access")
    .readTimeoutMs(15_000)
    .build();

// service-to-service
HttpResponse resp = http.post("http://user-svc/api/users/123",
    Map.of("name", "x"));
CompletableFuture<HttpResponse> fut = http.postAsync(...);

// 透传
http.get("http://user-svc/api/users/me", userJwt);
```

### Rust

```rust
use digua_http_client::{KudiWebClient, KudiHttpOptions, Method, RequestBody};

let http = KudiWebClient::builder()
    .credential("合同系统OrKey", "111")
    .token_endpoint("http://token-svc/api/tokens/access")
    .read_timeout_ms(15_000)
    .build()?;

// service-to-service
let resp = http
    .post("http://user-svc/api/users/123", RequestBody::json(&serde_json::json!({"name":"x"}))?)
    .await?;
println!("{} {}", resp.status, resp.body_as_string()?);
let user: User = resp.body_as()?;

// 透传
let user_jwt = ctx.user_jwt.clone();
http.get_with_bearer("http://user-svc/api/users/me", &user_jwt).await?;
```

## 7. 凭证与 token 管理

- **构造**：`WithCredential(ClientId, SecretKey)` 设置，`WithTokenEndpoint(url)` 指定获取地址（默认 `http://token-svc/api/tokens/access`）
- **首次请求**触发 fetch：`grant_type=client_credentials`，`client_id` / `client_secret` 走 form-urlencoded
- **缓存**：`access_token` + `expires_at` 存内存；提前 60s 主动刷新
- **缓存 key**：`SHA256(ClientId | SecretKey)`，相同凭证无论 `KudiWebClient` 实例化多少次复用同一缓存（static `ConcurrentDictionary` / `ConcurrentHashMap` / `DashMap`）
- **并发刷新**：`SemaphoreSlim`(.NET) / `ReentrantLock`(Java) / `tokio::Mutex`(Rust) 串行化，避免雪崩
- **401 处理**：清缓存 → 重 fetch → 重发原请求一次；再 401 抛 `AuthException`
- **token 永不出 SDK**：仅作为 `Authorization: Bearer ...` 头加入请求
- **不持久化**：进程重启重新拉

## 8. HTTP 客户端

- .NET：`System.Net.Http.HttpClient`，`IHttpClientFactory` 友好；连接池复用由调用方决定（SDK 不在静态字段持有 `HttpClient`）
- Java 11+：`java.net.http.HttpClient`；Java 8：`HttpURLConnection`（按 `maven.compiler.release` profile 分支编译，与现有 `sdk/java/digua_common` 策略一致）
- Rust：`reqwest` 默认 feature（`rustls-tls`），共享 `reqwest::Client`（内部 `Arc`）

## 9. 错误模型

三语言同构：

| 类型 | 触发 |
|------|------|
| `AuthException` / `Auth` | HTTP 401/403，重取后仍失败 |
| `ValidationException` / `Validation` | HTTP 400/422 |
| `RateLimitException` / `RateLimit` | HTTP 429 |
| `ServerException` / `Server` | HTTP 5xx（重试 1 次后仍失败） |
| `NetworkException` / `Network` | 连接失败 / DNS / 超时 |

响应体解析进 `Details` / `details()` 字段，便于日志。

## 10. 配置

| 字段 | 默认 | 说明 |
|------|------|------|
| `ConnectTimeoutMs` | 5000 | 连接超时 |
| `ReadTimeoutMs` | 15000 | 读超时 |
| `RetryCount` | 1 | 5xx/网络异常重试次数 |
| `UserAgent` | `kudi-http-client/0.1.0` | 请求 UA |
| `TokenEndpoint` | `http://token-svc/api/tokens/access` | client_credentials 端点 |
| `RefreshSkewSeconds` | 60 | 提前刷新秒数 |
| `Logger` | none | 可选注入（.NET `ILogger`、Java `org.slf4j.Logger`、Rust `tracing`） |

## 11. 构建

- .NET：`Directory.Build.props` 统一 `Version` / `Authors` / `RepositoryUrl`；`KudiCoder.sln` 加新项目
- Java：父 `sdk/java/pom.xml` 加 module；`digua_common` 的 `HttpClientWrapper` 复用为底座（不破坏性）
- Rust：仓库根 `Cargo.toml` 加 `members = ["services/*", "libs/*", "sdk/rust/*"]`；依赖 `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }`、`tokio = { version = "1", features = ["full"] }`、`serde`、`serde_json`、`thiserror`、`tracing`

## 12. 测试

- 单元（mock HTTP transport）
  - 首次拉 token 后缓存命中
  - 过期前 60s 主动刷新
  - 401 → 清缓存 → 重 fetch → 重发
  - 5xx 重试 1 次
  - 透传 Bearer 时不触发 token 拉取
  - `(ClientId, SecretKey)` 维度缓存隔离
- 集成（`wiremock` / `httpmock` / Rust `wiremock-rs` 模拟 token-service + 下游）
  - 真实 client_credentials 端到端
  - 透传 + 下游重校验
- Smoke：参考现有 `SdkEndToEndSmoke.cs` 模式跑真 `token-service`（docker compose 已就位）
- 覆盖率：核心包 ≥ 80%
- 门槛：`dotnet test` / `mvn verify` / `cargo test` 全过；符合项目 `coder-sdk-publish` 流程

## 13. 文档

- 每个语言包自带 `README.md`：安装、配置、`KudiWebClient` 创建、service-to-service 调用、透传 Bearer、错误处理、示例
- 顶层 `sdk/README.md` 加入"服务间通用 HTTP 客户端"段落，与现有 user/token SDK 区分

## 14. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Rust 端首次进入 `sdk/`，与现有 `services/*`、`libs/*` 模式不同 | 在 `sdk/rust/README.md` 说明定位；CI 显式 cargo build 该 crate |
| `token-service` 端点变更 | SDK 默认 endpoint 可被 `WithTokenEndpoint` 覆盖；CI 端到端测试在端点变更时同步更新 |
| 进程内 token 缓存导致凭据轮换不生效 | 缓存不含凭据明文；轮换时通过 `InvalidateAsync()` / `invalidate()` 强制清缓存；生产侧建议同时滚动所有调用方 |
| `HttpURLConnection` 与 `HttpClient` 行为差异 | 公共 transport 抽象；Java 8 路径独立集成测试 |
| .NET 6/9/10 行为差异 | CI 矩阵构建 + 矩阵测试 |
| Rust `reqwest` 体积 | 默认 `rustls-tls` feature 关闭多余依赖；README 说明 feature 切换 |

## 15. 后续

实施通过 `writing-plans` 拆解为可执行任务清单。三语言同步落地，不分批。
