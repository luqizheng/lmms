# KudiCoder .NET SDK

`user-service` / `token-server` 的 .NET 6 / 9 / 10 客户端 SDK。

## 项目

| Project | PackageId | 说明 |
|---|---|---|
| `Common` | `KudiCoder.Sdk.Common` | `HttpClient` 包装、token 缓存、模型、异常基类 |
| `UserService.UserSdk` | `KudiCoder.UserService.UserSdk` | 终端用户 → `user-service`（`username + password`） |
| `UserService.ClientSdk` | `KudiCoder.UserService.ClientSdk` | 服务间 → `user-service`（`api_key + secret`） |
| `TokenServer.UserSdk` | `KudiCoder.TokenServer.UserSdk` | 终端用户登录 / 刷新 / 登出 |
| `TokenServer.ClientSdk` | `KudiCoder.TokenServer.ClientSdk` | 服务间 `client_credentials` |

`Directory.Build.props` 统一 `Version=0.1.0`、`TargetFrameworks=net6.0;net9.0;net10.0`、`TreatWarningsAsErrors=true`、`GeneratePackageOnBuild=true`，并产出 `.snupkg` 符号包。

## 构建

```bash
dotnet restore
dotnet build -c Release
dotnet test
```

## 引入

```xml
<PackageReference Include="KudiCoder.UserService.UserSdk" Version="0.1.0" />
```

或：

```bash
dotnet add package KudiCoder.UserService.UserSdk --version 0.1.0
```

## 网络

所有调用统一走 `api-gateway` 的单一 `BaseUrl`（例：`https://gw.kudi.repos`）。

## 仓库

`http://nuget.kudi.repos/v3/index.json`（apiKey 取自 CI 变量 `NUGET_API_KEY`）。

## 发布

CI 触发 `v*.*.*` tag，本地不入库：

```bash
dotnet pack -c Release
dotnet nuget push "bin/Release/*.nupkg" \
  --source http://nuget.kudi.repos/v3/index.json \
  --api-key $NUGET_API_KEY
```

详细 spec 见 `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md`。
