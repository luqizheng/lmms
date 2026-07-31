# 多语言 SDK 套件实施计划

日期：2026-06-08
依据 spec：`docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md`
状态：执行中

## 阶段一：脚手架

- [ ] 1.1 创建 `sdk/` 顶层目录与 `openapi/`、`scripts/`
- [ ] 1.2 导出 `user-service` OpenAPI 到 `sdk/openapi/user-service.json`
- [ ] 1.3 导出 `token-server` OpenAPI 到 `sdk/openapi/token-server.json`

## 阶段二：Java 公共层

- [ ] 2.1 创建 `sdk/java/pom.xml` 父 POM（管理 groupId、version、profile）
- [ ] 2.2 实现 `sdk/java/digua_common`
  - `HttpClient` 包装（11+ 用 `java.net.http.HttpClient`，8 走 `HttpURLConnection`）
  - `TokenCache`（提前 60s 刷新 + 401 重试一次）
  - `SdkException` + 子类
  - 通用模型：`TokenInfo`、`ApiError`
  - 序列化使用 Jackson

## 阶段三：Java 业务 SDK

- [ ] 3.1 `sdk/java/user-service-user-sdk`：登录、用户 CRUD、角色
- [ ] 3.2 `sdk/java/user-service-client-sdk`：client_credentials、用户/角色管理
- [ ] 3.3 `sdk/java/token-server-user-sdk`：login、refresh、logout
- [ ] 3.4 `sdk/java/token-server-client-sdk`：client_credentials、api_key 管理

## 阶段四：.NET 公共层

- [ ] 4.1 创建 `sdk/dotnet/Directory.Build.props`（统一 Version、TargetFrameworks、RepositoryUrl）
- [ ] 4.2 实现 `sdk/dotnet/Common`
  - `HttpClient` 包装，`IHttpClientFactory` 友好
  - `TokenCache`
  - `SdkException` + 子类
  - 通用模型
  - 序列化使用 `System.Text.Json`

## 阶段五：.NET 业务 SDK

- [ ] 5.1 `sdk/dotnet/UserService.UserSdk`（net6.0;net9.0;net10.0）
- [ ] 5.2 `sdk/dotnet/UserService.ClientSdk`
- [ ] 5.3 `sdk/dotnet/TokenServer.UserSdk`
- [ ] 5.4 `sdk/dotnet/TokenServer.ClientSdk`

## 阶段六：生成与发布

- [ ] 6.1 创建 `sdk/scripts/generate.sh`（openapi-generator 触发脚本）
- [ ] 6.2 创建 `sdk/README.md` 总览
- [ ] 6.3 各 SDK 包自带 `README.md`
- [ ] 6.4 向 `.gitlab-ci.yml` 添加 `sdk:publish` 任务，标签 `v*.*.*` 触发

## 阶段七：测试

- [ ] 7.1 Java 单元测试（MockWebServer）
- [ ] 7.2 .NET 单元测试（WireMock.Net）
- [ ] 7.3 集成测试（连 docker-compose）

## 验收

- 全部阶段勾选完成
- `mvn verify` 通过（Java 8/17/21）
- `dotnet test` 通过（net6.0/9.0/10.0）
- 私有 Maven/NuGet 推送成功
