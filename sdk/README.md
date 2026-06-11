# KudiCoder SDK 套件

`user-service`、`token-server` 的多语言访问 SDK。

## 结构

```
sdk/
  java/                          # Java 8 / 17 / 21
    common/                      # HttpClient、TokenCache、异常、模型
    user-service-user-sdk/       # user → user-service
    user-service-client-sdk/     # client → user-service
    token-server-user-sdk/       # user → token-server
    token-server-client-sdk/     # client → token-server
    kudi-http-client/            # 通用服务间 HTTP 客户端（client_credentials + Bearer 透传）
  dotnet/                        # .NET 6 / 9 / 10
    Common/                      # 公共基础
    UserService.UserSdk/
    UserService.ClientSdk/
    TokenServer.UserSdk/
    TokenServer.ClientSdk/
    KudiCoder.HttpClient/        # 通用服务间 HTTP 客户端
    tests/KudiCoder.HttpClient.Tests/
  rust/                          # Rust（workspace 成员）
    kudi-http-client/            # 通用服务间 HTTP 客户端
  openapi/                       # 离线 OpenAPI 规范
  scripts/                       # openapi-generator 触发脚本
```

## 选型矩阵

| 场景 | 用户凭证 | 包名（Java / .NET） |
|------|----------|---------------------|
| 终端用户调用 user-service | username + password | `user-service-user-sdk` / `KudiCoder.UserService.UserSdk` |
| 终端用户调用 token-server | username + password | `token-server-user-sdk` / `KudiCoder.TokenServer.UserSdk` |
| 服务间调用 user-service | api_key + secret | `user-service-client-sdk` / `KudiCoder.UserService.ClientSdk` |
| 服务间调用 token-server | api_key + secret | `token-server-client-sdk` / `KudiCoder.TokenServer.ClientSdk` |

## 网络路径

所有调用统一走 `api-gateway` 的单一 baseUrl。

## 仓库

- Maven：`http://mvn.kudi.repos/#/`（Reposilite）
- NuGet：`http://nuget.kudi.repos/v3/index.json`

## 重新生成

```bash
export OPENAPI_GENERATOR_JAR=/path/to/openapi-generator-cli.jar
./scripts/generate.sh
```

生成结果落在 `.generated/`，作为各 SDK 内部实现参考。
