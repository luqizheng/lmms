# 03. 用户登录与会话

## 3.1 用户身份流程

用户身份由 iam-service 管理。典型流程：

```text
用户输入凭据
   |
   v
POST /v1/iam/auth/login
   |
   v
access_token + refresh_token
   |
   +--> access_token: Authorization: Bearer ...
   |
   +--> refresh_token: 仅用于续期端点，安全保存
```

用户 JWT 的 audience 与服务级 JWT 不同。用户 access token 不得用作 file-service 等内部服务的服务凭证。

## 3.2 主要认证端点

下表为 Gateway 对外路径形式；仍需核对部署路由：

| 方法 | 路径 | 用途 | 认证 |
|---|---|---|---|
| GET | `/v1/iam/auth/setup-status` | 查看是否需要首次初始化 | 通常公开 |
| POST | `/v1/iam/auth/register` | 首次注册/允许注册时创建用户 | 由服务策略控制 |
| POST | `/v1/iam/auth/login` | 用户名密码登录 | 公开 |
| POST | `/v1/iam/auth/refresh` | 刷新令牌 | refresh token |
| POST | `/v1/iam/auth/logout` | 登出/撤销当前令牌 | 用户 JWT |
| POST | `/v1/iam/auth/password` | 修改密码 | 用户 JWT |
| POST | `/v1/iam/auth/forgot-password` | 发起忘记密码流程 | 公开 |
| POST | `/v1/iam/auth/reset-password` | 使用重置凭据设置密码 | 重置 token |
| GET | `/v1/iam/auth/sessions` | 查询当前用户会话 | 用户 JWT |
| DELETE | `/v1/iam/auth/sessions/{session_id}` | 撤销指定会话 | 用户 JWT |

服务 OpenAPI 中对应的内部路径一般省略 `/v1` 和 Gateway rewrite。请求体字段请直接读取部署版本 `services/iam-service/openapi.yaml`。

## 3.3 登录示例（HTTP）

```bash
curl -sS -X POST "$LMMS_GATEWAY/v1/iam/auth/login" \
  -H 'Content-Type: application/json' \
  -d '{"username":"alice","password":"<password>"}'
```

响应模型以 OpenAPI 的 `LoginResponse` 为准。客户端应从实际响应读取 access token、refresh token 和有效期，不要假设 token 永不过期。

## 3.4 Token 保存与刷新

- Web 应用优先采用安全、HttpOnly、Secure、SameSite Cookie 的 BFF 模式；若使用浏览器持有 bearer token，必须评估 XSS、CSRF、存储和 CSP 风险。
- 不要把 refresh token 放在 URL、日志、埋点或错误报告中。
- access token 过期后，使用 refresh token 调用 refresh 端点；按服务端响应替换 token。
- refresh 失败或会话已撤销时，清理本地会话并要求重新登录。
- 多标签页/多线程客户端应避免并发 refresh 造成 token 竞争；建议单飞刷新并广播会话更新。
- 登出后清理本地凭证。服务端撤销行为按当前实现与 token 类型核验。

## 3.5 会话管理 UI

推荐 UI 功能：

- 当前登录设备/会话列表。
- 当前会话标识。
- 撤销其他会话/指定会话。
- 修改密码、忘记密码和重置密码入口。
- 登录失败、账户禁用、凭证过期等明确错误提示。

会话列表字段以 `SessionInfo` schema 为准。不要自行展示 IP、设备名等并非接口返回的字段。

## 3.6 首次管理员与注册策略

`setup-status` 用于判断系统是否需要初始设置。注册端点并不意味着生产环境永久开放自助注册；当前 OpenAPI 明确注册可能在已有用户时关闭。生产部署应由管理员定义初始账号、邀请/注册策略和密码恢复渠道。

忘记密码端点为防止账号枚举，可能对请求统一返回受理状态；客户端不应根据响应推断账号是否存在。
