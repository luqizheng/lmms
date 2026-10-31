# KudiCoder SDK 套件

`iam-service`、`token-service` 的多语言访问 SDK。

## 结构

```
sdk/
  java/                          # Java 8 / 17 / 21（v0.2.0，已对齐 v1 路由）
    digua_common/                      # HttpClient、TokenCache、异常、模型
    user-service-user-sdk/       # user -> iam-service
    user-service-client-sdk/     # client -> iam-service
    token-server-user-sdk/       # user -> token-service
    token-server-client-sdk/     # client -> token-service
    digua_http_client/            # 通用服务间 HTTP 客户端（client_credentials + Bearer 透传）
  rust/                          # Rust（workspace 成员）
    digua_http_client/            # 通用服务间 HTTP 客户端
  iam-services/                  # npm TS 包：登录 / 用户管理 / token 管理
  audit-services/                # npm TS 包：日志查询 / 合规导出 / 保留策略
  config-services/               # npm TS 包：配置中心（命名空间/配置项/导入导出）
  file-services/                 # npm TS 包：multipart 上传 / 二进制下载 / 预签名
  notification-services/         # npm TS 包：公告 / 未读数（不含 WS）
  prue-services/                 # npm TS 包：动态路由 / 菜单配置版本（ETag 乐观锁）
  openapi/                       # 离线 OpenAPI 规范
  scripts/                       # openapi-generator 触发脚本
```

> **npm TS 包**：浏览器 / Node 侧不放在 `sdk/`，而是仓库顶层 `packages/`（`@digua/contracts` + `@digua/http`，发布到私有 registry `http://192.168.2.12:14873/`，域名别名 `http://npm.kudi.repos/`；发布用根目录 `pnpm run packages:publish`，发布前校验用 `pnpm run packages:check`；CI 在 `v*.*.*` tag 触发时由 `publish_packages` job 自动按 tag 版本发布）
>
> **例外**：`sdk/iam-services`（npm 包名 `iam-services`）是 npm TS 包放 `sdk/` 的唯一例外——经用户裁决（2026-10-31）落位于此，同样接入上述发布管线（已列入 `packages/cli/lib/npm-packages.js` 清单）。
>
> **C# SDK（`sdk/dotnet`）规划中**：目录尚未创建。

> Java SDK 的 artifact ID 仍沿用历史命名（`user-service-*` / `token-server-*`），但 `0.2.0` 起内部已经对齐到合并后的 `iam-service`（`/v1/iam/...`）和 `token-service`（`/v1/tokens/...`、`/v1/api-keys`），请求体一律为 JSON。

## 选型矩阵

| 场景 | 用户凭证 | 包名（Java） |
|------|----------|--------------|
| 终端用户调用 iam-service | username + password | `user-service-user-sdk` |
| 终端用户调用 token-service | username + password | `token-server-user-sdk` |
| 服务间调用 iam-service | api_key + secret | `user-service-client-sdk` |
| 服务间调用 token-service | api_key + secret | `token-server-client-sdk` |

## 网络路径

所有调用统一走 `api-gateway` 的单一 baseUrl，路径前缀为 `/v1/...`。

## 仓库

- Maven：`http://mvn.kudi.repos/#/`（Reposilite）
- npm：`http://192.168.2.12:14873/`（别名 `npm.kudi.repos`；`@digua/contracts` / `@digua/http` 位于仓库顶层 `packages/`，`iam-services` 位于 `sdk/iam-services`）
- kellnr（Rust crates）：见 `sdk/rust` 发布脚本（`pnpm run sdk:publish:rust`）

## 重新生成

```bash
export OPENAPI_GENERATOR_JAR=/path/to/openapi-generator-cli.jar
node sdk/scripts/generate.js
```

生成结果落在 `.generated/`，作为各 SDK 内部实现参考。
