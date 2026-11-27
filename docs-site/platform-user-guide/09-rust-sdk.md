# 09. Rust SDK（主要接入方式）

## 9.1 SDK 组成与真实目录

Rust SDK 位于 `sdk/rust/`，workspace package 名称如下：

| Crate | 仓库目录 | 用途 |
|---|---|---|
| `digua_http_client` | `sdk/rust/digua-http-client` | 共享 HTTP transport、client_credentials token cache |
| `digua_iam_sdk` | `sdk/rust/iam-service-sdk` | 用户认证、用户/角色/权限 |
| `digua_token_sdk` | `sdk/rust/token-service-sdk` | Token/OAuth/API key |
| `digua_audit_sdk` | `sdk/rust/audit-service-sdk` | 审计日志与留存策略 |
| `digua_file_sdk` | `sdk/rust/file-service-sdk` | 文件元数据、presign、multipart 上传、二进制下载 |
| `digua_notification_sdk` | `sdk/rust/notification-service-sdk` | 公告、发布/撤销、未读数 |

Crate package 名使用下划线，但仓库目录使用连字符或 `*-service-sdk`。不要根据 crate 名推测源码目录。

## 9.2 构建和测试

在仓库根目录执行：

```bash
cargo build -p digua_http_client -p digua_iam_sdk -p digua_token_sdk -p digua_audit_sdk -p digua_file_sdk -p digua_notification_sdk
cargo test -p digua_http_client -p digua_iam_sdk -p digua_token_sdk -p digua_audit_sdk -p digua_file_sdk -p digua_notification_sdk
```

SDK crate 通过 workspace/path 依赖开发；不要猜测公开 registry 已发布状态和版本。

## 9.3 IAM 登录：基本示例

```rust
use digua_iam_sdk::{IamClient, ListUsersQuery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (client, login) = IamClient::login(
        "https://api.example.com/v1",
        "alice",
        "password-from-secret-input",
    ).await?;

    println!("logged in: {}", login.user.nickname);
    let users = client.list_users(&ListUsersQuery::new().page(0).size(20)).await?;
    println!("users total: {}", users.total);
    Ok(())
}
```

Gateway 对外前缀要以部署路由为准。SDK 会在传入 base URL 后追加 `/iam/...`；应传入与此拼接方式匹配的 Gateway base URL，不能重复追加 `/v1/iam`。

## 9.4 Token refresh：重要的客户端状态语义

`IamClient::login` 返回的 client 会捕获登录时的 access token。调用 `refresh()` 返回新 token **不会自动更新这个 client 内部捕获的 bearer**。应用应管理 token，并通过动态 bearer supplier 构造 client：

```rust
use std::sync::{Arc, Mutex};
use digua_iam_sdk::IamClient;

let token = Arc::new(Mutex::new(Some(login.access_token.clone())));
let token_for_client = Arc::clone(&token);
let client = IamClient::new("https://api.example.com/v1", move || {
    token_for_client.lock().expect("token lock").clone()
});

// refresh 成功后，将新 access_token 写回共享状态：
// *token.lock().expect("token lock") = Some(refreshed.access_token);
```

refresh 请求应使用当前 refresh token；新 access token 与 refresh token 应作为一个会话状态原子更新。生产应用还需处理并发 refresh（single-flight）、refresh 失败登出和 token 安全存储。

## 9.5 服务凭证与 client_credentials

`digua_http_client::KudiWebClient` 支持显式 bearer supplier，也支持通过 `builder().credential(client_id, secret_key)` 配合 token endpoint 自动获取并缓存 client_credentials token。

```rust
use digua_http_client::client::KudiWebClient;

let http = KudiWebClient::builder()
    .base_url("https://api.example.com")
    .token_endpoint("https://api.example.com/<deployed-token-endpoint>")
    .credential("client-id", "client-secret")
    .build()?;
```

token endpoint 的确切 URL、grant 和凭证类型以当前部署 OpenAPI/Gateway 路由为准。client_credentials 不自动等同于内部 `service-jwt` / `aud=file-service`。LMMS 微服务内部调用应使用 `libs/common-http`、`ServiceJwtCache` 等平台内部组件。

## 9.6 文件 SDK

`digua_file_sdk::FileClient` 当前包含 `create_presign`、`list_files`、`get_file_info`、`upload_file`（multipart）和 `download_file`（二进制）。该 SDK 是服务端调用路径，bearer 必须是 `aud=file-service` 的 service-jwt。上传 metadata 的 Rust 模型包含 filename、content_type、user_id、temporary/TTL 字段；tenant 由 service-jwt 的 `tid` claim 决定，不应从浏览器传入 tenant_id。详情见 [06 文件对象服务](./06-file-objects.md)。

## 9.7 通知 SDK 与 WebSocket

`digua_notification_sdk` 包含公告列表/详情/创建/发布/撤销和未读数方法；当前 SDK 未包装单条/全部已读以及 WebSocket。WebSocket 使用独立客户端，连接认证与帧格式见 [07 通知与实时推送](./07-notifications.md)。

## 9.8 错误处理

SDK 返回 `Result<T, SdkError>`。错误分类：`Auth`（401/403）、`Validation`（400/422）、`RateLimit`（429）、`Server`（5xx）、`Network`（连接、解码或序列化错误）。对非幂等写操作不要盲目重试。

## 9.9 SDK 与服务端内部调用的边界

普通业务应用使用 `sdk/rust` 的 typed clients。Rust 微服务内部调用使用 `libs/common-http` 中的 `ResilientClient`、`ServiceJwtCache` 等平台内部组件。不要将内部 crate 当成稳定的外部 SDK，也不要使用用户 bearer supplier 替代服务 audience 鉴权。

## 9.10 源码入口

- `sdk/rust/README.md`
- `sdk/rust/iam-service-sdk/README.md`
- `sdk/rust/file-service-sdk/README.md`
- `sdk/rust/notification-service-sdk/README.md`
- 根目录 `Cargo.toml`（workspace members）
- 对应 crate 的 `src/client.rs`、`src/model.rs` 和 `tests/client_test.rs`
