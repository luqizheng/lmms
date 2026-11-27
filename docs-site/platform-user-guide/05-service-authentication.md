# 05. 服务身份与服务间鉴权

## 5.1 为什么要区分两种 Token

| 项目 | 用户 JWT | service-jwt |
|---|---|---|
| 主体 | 用户 | 调用服务 |
| 代表谁 | 当前登录用户 | 某个后端服务 |
| Audience | 用户 API 的 audience | 目标服务 audience，例如 `file-service` |
| 使用位置 | 用户访问业务 API | 后端服务间调用 |
| 是否可放浏览器 | 按用户 token 安全模型处理 | **不允许** |

file-service 当前是明确的内部服务：`/file/*` 要求 `aud=file-service` 的 service-jwt；用户 JWT 会被拒绝。不要尝试从浏览器直接调用 file-service。

## 5.2 Bootstrap 与服务身份

平台采用受控 bootstrap 凭证建立服务身份，再获取目标 audience 的短期 service-jwt。bootstrap 凭证和 audience grant 是两类不同对象：

- **Bootstrap Token 管理（用户管理面）**：Gateway OpenAPI 提供 `/v1/admin/bootstrap-tokens`、`/{owner}`、`/{owner}/rotate`、`/{owner}/status` 等端点。使用有权限的管理员用户 JWT；权限码包括 `TOKEN_BOOTSTRAP_READ/CREATE/UPDATE/DELETE`。创建/轮换响应只返回一次明文 secret，列表/详情不返回明文。
- **Service JWT Audience Grant 管理（用户管理面）**：`/v1/admin/service-jwt-grants` 及其 owner/audience 子路径使用管理员用户 JWT，并检查 `TOKEN_SERVICE_JWT_GRANT_*` 权限码。grant 定义某个服务 owner 可以申请哪个 audience。
- **内部 grant 管理 API**：另有 `/admin/service-jwt-grants*` 路径，要求 `aud=token-service` 的 service-jwt，并且调用主体必须在 `TOKEN_SERVICE_JWT_GRANT_ADMINS` allowlist 中；该 allowlist 为空时 fail-closed。不要将它与上面的 Web UI 用户 API 混淆。
- **服务凭证交换**：服务使用自己的 bootstrap token，通过平台内部约定的 token-service 交换流程取得短期 service-jwt。准确请求路径和字段以当前 `libs/common-jwt` 的 `ServiceJwtCache` 实现及部署配置为准；不要从旧的 OAuth/client_credentials 示例推断。

所有管理 API 的确切请求 schema 以 `services/token-service/openapi.yaml` 为准。

## 5.3 服务端调用示意

```text
Business API receives user JWT
          |
          v
Validate user + tenant + resource permission
          |
          v
Business service obtains service-jwt (aud=file-service)
          |
          v
POST /file/upload  Authorization: Bearer <service-jwt>
          |
          v
file-service validates audience and caller
```

业务服务不能把用户身份判断委托给 file-service。调用方必须在通过用户鉴权后，按受信业务上下文填写文件的 user_id、tenant_id 等归属字段。

## 5.4 Rust 服务内部调用

仓库 Rust 服务内部调用与面向外部的 SDK 是不同层次：

- `libs/digua_common-http` 的 `ResilientClient`：服务端 HTTP 调用能力。
- `ServiceJwtCache`：面向目标 audience 的 service-jwt 缓存/获取。
- 服务代码通过受控 client 发起内部调用。
- 不应在每个业务 handler 中手写 token 获取、缓存、重试和 URL 拼接。

具体构造参数与 trait API 以当前 crate 源码和各服务示例为准；不要复制旧代码片段后假设版本一致。

## 5.5 服务身份管理 UI

适合平台管理员的页面：

- Bootstrap Token 列表：owner、状态、创建/更新时间、过期/轮换元数据（不展示明文）。
- 创建/轮换：明文仅在成功响应时展示一次，要求复制/下载后关闭，不可再次查询。
- Audience Grants：调用服务 → 目标 audience 的授权关系；支持启用/禁用和审计。
- 凭证状态：过期、禁用、轮换窗口、最近使用情况（仅当 API 真正提供时展示）。
- 安全事件：凭证轮换、授权变更、失败调用。

UI 不应提供“任意 audience 全开”的快捷操作。权限码字典中的 `TOKEN_BOOTSTRAP_*` 和 `TOKEN_SERVICE_JWT_GRANT_*` 用于管理这些操作；服务端仍须做授权校验。

## 5.6 绝对不要做

- 不把 service-jwt 放到 localStorage、浏览器 bundle、移动端安装包或公开日志。
- 不使用用户 JWT 访问 file-service 内部 API。
- 不在服务间调用中伪造 `X-User-Id`、`X-Tenant-Id` 等身份头作为认证依据。
- 不使用单一万能 token 访问所有下游服务。
- 不在没有授权记录的情况下扩大目标服务 audience。
