# 07. 通知与实时推送

## 7.1 当前能力模型

notification-service 当前主要实现公告（Announcement）及其接收范围、发布状态、已读状态、未读数量和 WebSocket 推送。

不要把公告模型等同于完整的个人消息收件箱。当前能力不应被描述为已具备任意用户私信、消息线程、消息模板发送、多渠道邮件/短信、通用通知偏好和完整投递回执系统。

## 7.2 公告生命周期

OpenAPI 中公告状态包括：

- `draft`：草稿。
- `scheduled`：计划发布。
- `published`：已发布。
- `revoked`：已撤销。

公告字段包括标题、正文、计划发布时间、可选过期时间、置顶、弹窗标记、状态、创建者和创建时间。用户视角的 `read` 字段由服务根据公告阅读记录填充。

## 7.3 接收范围

当前 `ScopeInput` 支持：

| scope_type | 含义 |
|---|---|
| `user` | 指定用户 |
| `role` | 指定角色 |
| `dept` | 指定部门 |

创建公告时接收范围是明确输入。服务端会维护用户成员关系投影，并通过 IAM 事件更新；不要由前端自行把角色/部门展开成用户列表作为权限真相。

## 7.4 常见 API

服务 OpenAPI 路径通常为内部路径，Gateway 对外路径要检查 route config。主要操作包括：

- 公告创建、面向当前用户的列表、详情、租户公告管理列表。
- 发布：`POST /notification/announcements/{id}/publish`
- 撤销：`POST /notification/announcements/{id}/revoke`
- 标记单条已读：`POST /notification/announcements/{id}/read`
- 全部标记已读：`POST /notification/announcements/read-all`
- 未读计数：`GET /notification/unread-count`

当前 OpenAPI 的用户公告列表响应是 `Announcement[]`，查询参数为 `pinned`、`active`；管理列表为租户范围内的 `Announcement[]`。当前 OpenAPI 未定义公告 update API，不要在 UI 中假设存在通用更新接口。以 `services/notification-service/openapi.yaml` 和 `services/notification-service/src/rest.rs` 为准。

## 7.5 WebSocket 实时推送

服务端内部路由为 `/notification/ws`（Axum upgrade）。外部连接路径由 Gateway router 决定，必须按部署配置核对，不能把内部服务端口直接提供给浏览器。

**认证：** WebSocket upgrade 使用 `Principal` extractor，通过 JWKS 验证用户 JWT，并从 JWT 的 `sub` 确定订阅用户。不要使用客户端自报的 `x-user-id` 作为身份。浏览器原生 WebSocket API 不能任意设置 Authorization header；若部署没有安全的 cookie/BFF 或受控 query-token 方案，应通过 BFF 代理 WebSocket，不要把长期 bearer token 放进 URL。

当前服务端帧协议（JSON text frame）：

| type | payload | 方向 | 说明 |
|---|---|---|---|
| `new` | 公告 JSON 对象 | 服务端 → 客户端 | 有新公告 |
| `unread_count` | `{ "count": number }` | 服务端 → 客户端 | 未读数更新 |
| `revoked` | `{ "id": number }` | 服务端 → 客户端 | 公告撤销 |
| `pong` | 无业务 payload | 服务端 → 客户端 | 心跳响应 |

服务端识别文本帧 `{"type":"ping"}` 并回复 `{"type":"pong"}`。其他客户端文本帧当前不作为业务命令处理。

客户端建议：
1. 通过部署配置确定 Gateway WebSocket URL 和认证方式。
2. 使用短生命周期用户会话建立连接，连接失败时按退避策略重连。
3. 收到 `new`、`unread_count`、`revoked` 后调用 REST API 刷新权威数据。
4. 使用公告 ID 去重；断线重连后重新拉取公告列表和未读数。
5. 处理连接关闭、网络切换和 token 过期；不要无限快速重连。

注意：WebSocket 的 `new` payload 是公告对象 JSON；协议实现位于 `services/notification-service/src/ws.rs` 与 `ws_hub.rs`，OpenAPI YAML 本身不描述 WebSocket。

## 7.6 通知 UI 建议

- 公告中心：列表、详情、状态、发布时间、过期时间。
- 未读角标：调用 unread-count 获取，不在前端自行猜测。
- 置顶公告/弹窗公告：根据服务端字段展示。
- 已读操作：调用 read API，成功后刷新状态和未读数。
- 管理端：草稿、发布计划、接收范围、发布、撤销。
- 实时更新：WebSocket 通知后刷新 REST 数据。

## 7.7 未来扩展需明确设计

如 AI-PM 需要任务指派通知、评论回复、个人站内信、审批待办、邮件/短信、通知偏好、投递状态或消息模板，需要先明确独立的消息实体、用户收件箱、幂等键、投递通道、重试、保留策略和权限模型；不要将公告表直接复用成所有消息类型。
