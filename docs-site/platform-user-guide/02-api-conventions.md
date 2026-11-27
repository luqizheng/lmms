# 02. API 入口与通用约定

## 2.1 统一入口

外部客户端统一使用 API Gateway。常见 base URL：

- 本地：`http://localhost:3000`
- 生产：由部署方提供 HTTPS 域名

外部 API 路径通常带版本前缀，例如 `/v1/iam/...`、`/v1/tokens/...`。服务自身 OpenAPI 中的路径通常是内部服务路径（如 `/iam/auth/login`），Gateway 会按路由配置匹配并转发。不能简单把 OpenAPI path 与 Gateway path 直接拼接而忽略前缀。

## 2.2 请求头

受保护的用户 API：

```http
Authorization: Bearer <user-access-token>
Content-Type: application/json
```

跨服务请求使用目标服务认可的 service-jwt，详见 [05 服务身份与服务间鉴权](./05-service-authentication.md)。不要把服务凭证发给浏览器。

建议所有请求携带调用方可识别的 request/trace context（遵循部署的 W3C Trace Context 传播策略）；不要自行覆盖平台生成的安全身份字段。

## 2.3 响应格式

仓库 `contracts/http-conventions.md` 定义了标准 JSON envelope 约定：

```json
{
  "code": "OK",
  "message": "success",
  "data": {},
  "request_id": "..."
}
```

- 成功时 `code` 为 `OK`。
- 业务数据位于 `data`。
- `request_id` 用于定位请求。
- 当前服务并非所有 handler 都统一包装 envelope：部分端点直接返回 OpenAPI 定义的 JSON 对象/数组，部分返回 204，文件下载返回二进制流。
- **客户端必须按每个端点的 OpenAPI response schema 解析**，不得对所有响应统一假定存在 `data` 包装层。OpenAPI 与 handler 不一致时应作为契约缺陷处理。

## 2.4 分页、排序、时间和 ID

| 项目 | 约定 |
|---|---|
| 分页 | `items`、`total`、`page`、`size`（具体端点可能使用 0-based 或 1-based，按 OpenAPI） |
| 排序 | `sort_by` + `sort_order=asc|desc`；默认通常为 `created_at desc` |
| 时间 | ISO-8601 UTC，例如 `2026-11-27T10:30:00Z` |
| ID | API 以字符串表达 ID，避免 JavaScript 大整数精度损失；具体模型也可能包含整数型租户/公告 ID |

分页参数的起始值和上限必须按各端点 OpenAPI 核验；不要在通用客户端中假定所有服务完全相同。

## 2.5 HTTP 状态与错误处理

常见处理原则：

| 状态 | 含义 | 客户端建议 |
|---|---|---|
| 400 / 422 | 参数或业务校验失败 | 展示可理解错误；不要无限重试 |
| 401 | 缺失、过期或无效身份 | 用户会话尝试 refresh；服务身份检查凭证/受众 |
| 403 | 已认证但权限不足 | 展示无权限；不应反复刷新 token |
| 404 | 资源不存在或不可见 | 按业务语义处理 |
| 409 | 状态冲突/重复/并发冲突 | 提示刷新状态或重新提交 |
| 413 | 上传体积超限 | 提示文件限制 |
| 429 | 网关或服务限流 | 尊重 `Retry-After`，退避重试 |
| 5xx | 服务端故障 | 有界重试；避免重复执行非幂等操作 |

SDK 错误类型可将认证、校验、限流、服务端和网络错误区分处理。任何重试都应考虑操作幂等性。

## 2.6 OpenAPI 与路由

主要来源：

- `services/<service>/openapi.yaml`：服务 API schema，由 utoipa 生成。
- `infrastructure/config/api-gateway/routes/*.router.toml`：外部路径、目标服务、public 标志和 rewrite。
- `contracts/http-conventions.md`：HTTP 通用规则。
- `contracts/permissions.yaml`：权限码字典。
- `contracts/events.yaml`：领域事件 schema。

Swagger UI 仅在服务配置 `SWAGGER_ENABLED=true` 时启用。生产环境是否公开 Swagger 由部署安全策略决定。

## 2.7 兼容性原则

- 以实际部署版本为准；不要将 main 分支文档当成所有已部署环境的 API 版本。
- 兼容性变更应检查 OpenAPI、Rust SDK、Java SDK、Gateway 路由和 E2E 测试。
- Java SDK 历史模块曾发生端点漂移，当前可用模块范围见 [10 Java SDK](./10-java-sdk.md)。
