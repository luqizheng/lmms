# 09. Rust SDK（主要接入方式）

## 9.1 SDK 组成

Rust SDK 位于 `sdk/rust/`，使用 typed client 包装各服务 API，底层共享 `digua_http_client`。

| Crate | 用途 |
|---|---|
| `digua_http_client` | HTTP transport、token supplier/cache 基础 |
| `digua_iam_sdk` | 用户认证及 IAM 管理 |
| `digua_token_sdk` | Token、OAuth、API key |
| `digua_audit_sdk` | 审计日志查询与留存策略 |
| `digua_file_sdk` | 文件 presign 和元数据查询 |
| `digua_notification_sdk` | 公告、通知未读数 |
| `digua_config_sdk` | 配置服务（若部署版本包含该服务） |

具体 crate 清单以 `sdk/rust/Cargo.toml` 和 workspace 为准。

## 9.2 在仓库内构建

```bash
cargo build -p digua_http_client \
  -p digua_iam_sdk \
  -p digua_token_sdk \
  -p digua_audit_sdk \
  -p digua_file_sdk \
  -p digua_notification_sdk

cargo test -p digua_http_client \
  -p digua_iam_sdk \
  -p digua_token_sdk \
  -p digua_audit_sdk \
  -p digua_file_sdk \
  -p digua_notification_sdk
```

当前 Rust SDK 文档说明 SDK crate 通过 workspace path 依赖使用；对外发布渠道和版本应查最新发布说明。不要把内部服务的 `libs/digua_common-http` 当成公共 SDK。

## 9.3 用户 JWT supplier

多数用户向 SDK 接口接收一个 `Fn() -> Option<String>` bearer supplier。由调用应用管理用户登录、refresh 和 logout，SDK 在请求时取得当前 bearer。

示意代码（具体构造函数和方法名以对应 crate 的当前 `src/client.rs` 为准）：

```rust
let token_supplier = || Some(current_access_token());
let client = /* digua_iam_sdk client configured with gateway URL
                and token_supplier */;
```

此处不写死构造函数签名，避免与 SDK 小版本漂移。应查：
- `sdk/rust/digua_iam_sdk/src/client.rs`
- `sdk/rust/digua_token_sdk/src/client.rs`
- 对应 crate 的 `tests/client_test.rs`

## 9.4 错误处理

SDK 返回 `Result<T, SdkError>`。文档定义的错误分类包括：

- `Auth`：401/403。
- `Validation`：400/422。
- `RateLimit`：429。
- `Server`：5xx。
- `Network`：连接、解码或序列化错误。

建议将错误映射到业务层统一错误模型；限流使用有界退避；对非幂等写操作不要盲目重试。

## 9.5 文件与流式协议

当前 Rust `digua_file_sdk` 文档列出 presign、list、info；二进制上传/下载可能需要单独 HTTP 客户端直接访问 presigned URL。共享 SDK 的文本响应抽象并不意味着支持任意二进制流。

WebSocket / SSE 不在通用 HTTP SDK 的范围内，应使用专用 streaming client，并按服务端协议实现认证、心跳、重连和去重。

## 9.6 外部 Rust 项目

如果 Rust SDK 尚未发布到你可访问的 registry，可在本地/同一源码工作区使用 path dependency。不要猜测 crate 版本或 registry 地址；先查看发布配置和当前版本，再选择内部 registry 或公开 registry。

## 9.7 服务端到服务端调用不是用户 SDK

Rust 微服务内部调用使用 `libs/digua_common-http` 中的 `ResilientClient` 和 `ServiceJwtCache` 等平台内部组件。不要将内部 crate 引入普通业务客户端，也不要使用用户 bearer supplier 替代服务 audience 鉴权。
