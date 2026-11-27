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

- 公告创建、列表、详情、更新。
- 发布：`POST /notification/announcements/{id}/publish`
- 撤销：`POST /notification/announcements/{id}/revoke`
- 标记已读：`POST /notification/announcements/{id}/read`
- 未读计数：`GET /notification/unread-count`

请以 `services/notification-service/openapi.yaml` 为准，特别核对列表查询参数和创建/更新请求体。

## 7.5 WebSocket 实时推送

公告发布事件会触发 WebSocket 推送扇出。客户端需：

1. 按部署配置连接 Gateway 暴露的 WebSocket 路由。
2. 使用当前实现规定的认证方式建立连接。
3. 处理连接关闭、网络切换和重连。
4. 将推送视为“有新数据”的提示，随后调用 REST API 拉取公告/未读数；不要将 WebSocket 帧当作唯一持久化记录。
5. 避免因重连重复展示；以公告 ID 和服务端已读状态去重。

WebSocket 路径、消息 envelope、鉴权时机和心跳协议必须从 notification-service 当前代码与 Gateway router 中确认。OpenAPI YAML 不完整描述 WebSocket 协议，本文不虚构其字段。

## 7.6 通知 UI 建议

- 公告中心：列表、详情、状态、发布时间、过期时间。
- 未读角标：调用 unread-count 获取，不在前端自行猜测。
- 置顶公告/弹窗公告：根据服务端字段展示。
- 已读操作：调用 read API，成功后刷新状态和未读数。
- 管理端：草稿、发布计划、接收范围、发布、撤销。
- 实时更新：WebSocket 通知后刷新 REST 数据。

## 7.7 未来扩展需明确设计

如 AI-PM 需要任务指派通知、评论回复、个人站内信、审批待办、邮件/短信、通知偏好、投递状态或消息模板，需要先明确独立的消息实体、用户收件箱、幂等键、投递通道、重试、保留策略和权限模型；不要将公告表直接复用成所有消息类型。
