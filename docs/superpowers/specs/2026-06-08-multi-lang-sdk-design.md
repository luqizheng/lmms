# 多语言 SDK 套件设计

日期：2026-06-08
状态：待用户审查
作者：brainstorming 会话

## 1. 目标

在 `sdk/` 下为 `user-service`、`token-server` 交付一套多语言访问 SDK，涵盖
Java 8/17/21、.NET Core 6/9/10 三套运行时。

清晰区分两种调用场景：

- **user-to-service**：终端用户访问后端微服务，凭证为 `username + password`。
- **client-to-client**：服务之间或受信任后台访问后端微服务，凭证为 `api_key + secret`（client_credentials 流程）。

## 2. 范围

包含：

- `user-service` SDK
- `token-server` SDK
- Java 8 / 17 / 21 三套目标
- .NET 6 / 9 / 10 三套目标
- 同步 + 异步双 API
- 内部 token 缓存与自动刷新
- 发布到私有 Maven / NuGet 仓库

不含：

- 任何后端服务改造
- 前端浏览器 SDK
- Java/.NET 之外的运行时（Node、Go、Python 等）

## 3. 决策摘要

| 项 | 决策 |
|----|------|
| 网络路径 | 统一走 `api-gateway` |
| 用户凭证 | `username + password`（user-to-service） |
| 客户端凭证 | `api_key + secret` / `client_credentials` |
| 生成方式 | openapi-generator 骨架 + 手工补充凭证层 |
| 拆分结构 | monorepo 双分类（每服务 + 每场景一个包） |
| API 风格 | 同步 + 异步双 API |
| 登录职责 | SDK 负责登录 + token 缓存 |
| HTTP 客户端 | Java 内置 `HttpClient`（8 走 `HttpURLConnection`）/ .NET `HttpClient` |
| 发布渠道 | 私有 Maven `http://mvn.kudi.repos`（Reposilite, admin/`<pwd>`） + 私有 NuGet `http://nuget.kudi.repos/v3/index.json`（apiKey `<key>`） |

## 4. 目录结构

```
sdk/
  java/
    digua_common/                            # 公共：HttpClient 包装、TokenCache、模型、异常
    user-service-user-sdk/             # user → user-service
    user-service-client-sdk/           # client → user-service
    token-server-user-sdk/             # user → token-server（登录、刷新）
    token-server-client-sdk/           # client → token-server
  dotnet/
    Common/                            # 公共
    UserService.UserSdk/               # 多目标：net6.0;net9.0;net10.0
    UserService.ClientSdk/
    TokenServer.UserSdk/
    TokenServer.ClientSdk/
  openapi/                             # 离线保存的 OpenAPI 规范
    user-service.json
    token-server.json
  scripts/
    generate.sh                        # openapi-generator 触发脚本
```

## 5. 命名与坐标

Java：

- `groupId`：`com.kudi.coder`
- `artifactId`：`<service>-<scenario>-sdk`（例：`user-service-user-sdk`）
- `version`：`0.1.0` 起，跟随 monorepo tag

.NET：

- `packageId`：`KudiCoder.<Service>.<Scenario>Sdk`
- `TargetFrameworks`：`net6.0;net9.0;net10.0`
- `Version`：与 monorepo tag 同步

## 6. 核心 API 形态

### Java

```java
// user → user-service
UserServiceUserClient c = UserServiceUserClient.login(
    "https://gw.kudi.repos", "alice", "secret");
User u = c.getById("u-1");
CompletableFuture<User> fu = c.getByIdAsync("u-1");

// client → user-service
UserServiceClientClient cc = UserServiceClientClient.create(
    "https://gw.kudi.repos", "api_key_xxx", "secret_yyy");
List<User> all = cc.listAll();
```

### .NET

```csharp
var c = await UserServiceUserClient.LoginAsync(
    "https://gw.kudi.repos", "alice", "secret");
var u = await c.GetByIdAsync("u-1");

var cc = await UserServiceClientClient.CreateAsync(
    "https://gw.kudi.repos", "api_key_xxx", "secret_yyy");
var all = await cc.ListAllAsync();
```

## 7. 凭证与 token 管理

- 构造时换 `access_token`，内部 `TokenCache` 缓存
- 提前 60s 主动刷新
- 收到 401 时强制刷新并重试一次
- token 永不出 SDK
- 登录端点：
  - user：`POST /api/v1/auth/login`（api-gateway → auth-service）
  - client：`POST /oauth/token`，`grant_type=client_credentials`
- 刷新：`POST /oauth/token`，`grant_type=refresh_token`
- 登出：`POST /api/v1/auth/logout`

## 8. HTTP 客户端

Java：

- 11+：`java.net.http.HttpClient`
- 8：`HttpURLConnection`（`digua_common` 包按 `maven.compiler.release` 编译分支）

.NET：

- `System.Net.Http.HttpClient`，`IHttpClientFactory` 友好

## 9. 错误模型

统一异常 `SdkException`，子类型：

- `AuthException`：401/403
- `RateLimitException`：429
- `ValidationException`：400/422
- `ServerException`：5xx
- `NetworkException`：连接/DNS/超时

响应体解析进 `getDetails()`，便于日志。

## 10. 配置

Java：`SdkConfig(baseUrl, connectTimeoutMs, readTimeoutMs, retryCount, proxy, logger)`

.NET：`SdkOptions { BaseUrl, ConnectTimeout, ReadTimeout, RetryCount, Proxy, Logger }`

## 11. 构建

Java：

- 多模块 Maven 父 POM 统一管理 `groupId`、`version`、插件
- 8/17/21 通过 `<maven.compiler.release>` 与 `profile` 切换公共模块源码分支
- 业务模块对版本不敏感，复用同一 source

.NET：

- `Directory.Build.props` 统一 `Version`、`Authors`、`RepositoryUrl`
- `<TargetFrameworks>net6.0;net9.0;net10.0</TargetFrameworks>`

## 12. 发布

- CI：`.gitlab-ci.yml` 新增 `sdk:publish` 任务
- 凭据来自 GitLab CI 变量（`MAVEN_REPO_USER`、`MAVEN_REPO_PASS`、`NUGET_API_KEY`），不入库
- 触发条件：标签 `v*.*.*` 推送
- 流程：构建 → 测试 → 打包 → 推送

Maven 仓库：`http://mvn.kudi.repos/#/`（Reposilite，admin/`<pwd>`）
NuGet 仓库：`http://nuget.kudi.repos/v3/index.json`（apiKey `<key>`）

## 13. 测试

- 单元：Java `MockWebServer` / .NET `WireMock.Net` 模拟 HTTP
- 集成：`docker-compose.test.yaml` 拉起 token-server + user-service + api-gateway，跑 e2e
- 契约：`openapi-generator validate` 校验生成的客户端与服务端规范一致
- 覆盖率：核心包 `digua_common` ≥ 80%

## 14. 文档

- 每个 SDK 包自带 `README.md`：安装、配置、登录、API 列表、错误处理
- 顶层 `sdk/README.md`：套件总览 + 选型矩阵
- API 文档自动生成：JavaDoc / XML doc → 推送到内部 GitLab Pages

## 15. 风险与缓解

| 风险 | 缓解 |
|------|------|
| Java 8 HTTP 实现差异 | 公共模块按 `release` 编译分支，关键路径 8 单独跑集成测试 |
| .NET 6/9/10 行为差异 | CI 矩阵构建 + 矩阵测试 |
| 私有仓库凭据泄露 | 仅入 GitLab CI 变量，仓库侧定期轮换 |
| 后端 OpenAPI 变更 | CI 校验步骤失败时阻断 SDK 发布 |
| SDK 版本与后端不匹配 | 文档强制声明最低后端版本号 |

## 16. 后续

实施通过 `writing-plans` 拆解为可执行任务清单。

