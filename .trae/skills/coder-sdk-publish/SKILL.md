---
name: coder-sdk-publish
description: 构建并发布多语言 SDK（Java + .NET）到私有 Maven/NuGet 仓库。用于「发 SDK」「sdk:publish」「跑 SDK 测试」等意图。
---

# coder-sdk-publish

## 何时用

- 完成 SDK 包代码后
- 打 `v*.*.*` tag 前
- 用户说「发 SDK」「sdk:publish」时

## 仓库

- Maven：`http://mvn.kudi.repos/#/`（Reposilite）
- NuGet：`http://nuget.kudi.repos/v3/index.json`

账号凭据见 `~/.m2/settings.xml` 与 `~/.nuget/NuGet/NuGet.Config`，本 skill 不打印。

## 步骤

### 1. 跑 SDK 测试

```powershell
cd d:\projects\coder-micro-services
pnpm run sdk:test
```

= `sdk:test:java` + `sdk:test:dotnet`

### 2. 单独跑

```powershell
# Java
pnpm run sdk:test:java
pnpm run sdk:test:java:8
pnpm run sdk:test:java:17
pnpm run sdk:test:java:21

# .NET
pnpm run sdk:test:dotnet
```

### 3. 重新生成（OpenAPI 变更后）

```bash
export OPENAPI_GENERATOR_JAR=/path/to/openapi-generator-cli.jar
cd d:\projects\coder-micro-services
./sdk/scripts/generate.sh
```

生成落 `.generated/`，手工合并到各包。

### 4. 发布（CI 触发，tag `v*.*.*`）

`.gitlab-ci.yml` 的 `sdk:publish` 任务会自动跑：

```bash
mvn deploy -f sdk/java/<pkg>/pom.xml
dotnet pack sdk/dotnet/<pkg> && dotnet nuget push ...
```

本地不要直接 push，除非紧急 hotfix。

## 包名

| Java artifactId | .NET packageId |
|-----------------|----------------|
| `user-service-user-sdk` | `KudiCoder.UserService.UserSdk` |
| `user-service-client-sdk` | `KudiCoder.UserService.ClientSdk` |
| `token-server-user-sdk` | `KudiCoder.TokenServer.UserSdk` |
| `token-server-client-sdk` | `KudiCoder.TokenServer.ClientSdk` |

## 阶段进度

详见 `docs/superpowers/plans/2026-06-08-multi-lang-sdk-plan.md`。
