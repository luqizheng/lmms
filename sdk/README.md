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
  openapi/                       # 离线 OpenAPI 规范
  scripts/                       # openapi-generator 触发脚本
```

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

## 重新生成

```bash
export OPENAPI_GENERATOR_JAR=/path/to/openapi-generator-cli.jar
node sdk/scripts/generate.js
```

生成结果落在 `.generated/`，作为各 SDK 内部实现参考。
