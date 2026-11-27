# 11. AI-PM 等上层平台集成指南

## 11.1 集成原则

将 LMMS 视为平台基础能力提供方；上层平台保留自己的领域模型、业务授权和 UI 状态。LMMS 负责身份和基础服务，不替代 AI-PM 的项目、需求、任务、Agent、审批或知识库模型。

推荐分层：

```text
AI-PM Web UI
   |
AI-PM Backend / BFF
   |-- user context from LMMS IAM
   |-- AI-PM project/task authorization (AI-PM owns)
   |-- LMMS service identity for internal APIs
   |-- file-service object operations
   |-- notification-service announcement / unread / WS
   |-- audit-service queries where authorized
   |
LMMS API Gateway + internal service network
```

## 11.2 UI 能力映射

| AI-PM UI 区域 | LMMS 能力 | 建议边界 |
|---|---|---|
| 登录/个人设置 | IAM login、refresh、logout、sessions、password | LMMS 管理认证；AI-PM 管理自己的个人偏好 |
| 用户管理 | IAM 用户 API | 用户/租户基础信息由 IAM 管；AI-PM 项目成员关系仍由 AI-PM 管 |
| 角色/权限管理 | IAM RBAC、permission codes | 平台级角色由 IAM 管；项目内角色/资源授权由 AI-PM 管 |
| 租户管理 | IAM tenant | tenant identity 与隔离；项目归属和项目数据由 AI-PM 管 |
| 文件列表/附件 | File Object API | AI-PM 保存 file_id 与业务引用；文件权限通过 AI-PM 后端检查 |
| 通知中心 | Announcement + unread count + WebSocket | 公告可直接集成；任务/审批/Agent 消息需 AI-PM 自己建模 |
| 审计页面 | Audit API | 展示 LMMS 审计事件；AI-PM 自身业务审计由 AI-PM 记录 |
| 服务凭证管理 | Token bootstrap / audience grants | 仅平台运维/安全管理员可见，不能暴露普通项目成员 |

## 11.3 用户与租户映射

建议 AI-PM 在自己的数据库中保存 LMMS user ID 和 tenant ID 的外部引用，不复制密码、refresh token 或 IAM 内部凭证。

- 登录由 LMMS 完成；AI-PM 后端验证用户身份上下文。
- AI-PM 仍要判断用户是否属于某项目、是否能读取某需求或审批某变更。
- tenant_id 必须来自已认证身份/可信上下文；不能由浏览器任意指定。
- 如果 AI-PM 支持项目成员角色，应作为 AI-PM 领域权限管理，不要全部塞入全局 IAM role。

## 11.4 文件与 AI-PM 业务实体关联

推荐 AI-PM 数据库保存独立关联记录，例如：

```text
ai_pm_attachment
  id
  tenant_id
  project_id
  owner_user_id
  lmms_file_id
  purpose
  created_at
```

这只是 AI-PM 侧的建议模型，不是 LMMS 已有表或 API。上传/下载/删除必须经过 AI-PM backend：

1. 校验用户对项目/任务/需求的访问权限。
2. 使用 AI-PM backend 的 service identity 调 file-service。
3. 将 LMMS file ID 关联到 AI-PM 业务实体。
4. 下载时再次校验实体权限，再创建短时 presigned URL 或代理下载。
5. 删除时先处理 AI-PM 引用与业务审计，再调用文件服务删除。

**不要**让浏览器持有 file-service service-jwt，也不要将 storage_path 暴露给 UI。

## 11.5 通知中心的边界

AI-PM 可把 LMMS 公告放入通知中心的“平台公告”区域，显示公告详情、未读数和实时更新。

AI-PM 以下消息应由自身领域实现，除非 LMMS 后续提供明确消息 API：
- 任务分配、任务状态变化。
- 需求评审和审批待办。
- Agent 运行状态、失败升级、人工介入请求。
- 项目评论、提及、文件处理完成。
- 个人消息偏好和通知投递记录。

可以在 AI-PM 自己的通知聚合层把 LMMS 公告和 AI-PM 消息统一呈现，但需要保留来源、权限、已读语义和各自的操作接口；不要把 LMMS 公告 read API 用来标记 AI-PM 任务已读。

## 11.6 权限分层建议

至少分三层：

1. **平台身份**：LMMS IAM 验证用户身份和全局权限。
2. **AI-PM 资源权限**：AI-PM 后端校验项目成员关系、项目角色、任务/需求/文档访问规则。
3. **服务调用权限**：AI-PM 后端持有受限 service-jwt，仅申请必要 audience。

前端只负责显示权限结果，后端每次操作都必须重新检查。Agent/自动化任务不得继承人类用户的无限权限；应由 AI-PM 定义 actor、审批门槛和操作范围。

## 11.7 建议的集成顺序

1. 接入 IAM 登录、refresh、logout 和当前用户上下文。
2. 建立 tenant/user 外部 ID 映射和 AI-PM 项目级授权。
3. 配置 AI-PM backend 服务身份，验证 audience 最小授权。
4. 接入文件对象上传/下载，建立 file_id 关联。
5. 接入公告、未读数和 WebSocket 推送。
6. 接入审计查询；AI-PM 自己的审批/Agent 操作同时写自己的业务审计。
7. 做跨租户、无权限、服务凭证泄露、预签名 URL 过期、通知重连等负向测试。

## 11.8 需单独立项的 LMMS 能力

若 AI-PM 希望 LMMS 原生提供以下能力，需要先定义 API、权限、数据模型和事件契约，再实现：

- 通用个人站内信、任务通知和审批待办。
- 用户偏好、通知渠道、投递状态、邮件/短信。
- 文件夹、文件移动/复制、版本、回收站、预览和扫描。
- AI-PM 专用的项目/任务权限域（若决定由 LMMS 统一承载）。
- 完整的用户级文件授权 API（当前文件服务采用服务间调用模型）。

## 11.9 AI-PM 接入验收清单

- [ ] 用户登录、refresh、logout、session revoke。
- [ ] 不同租户之间用户、项目和文件不可越权访问。
- [ ] 前端无法直接调用内部 file-service。
- [ ] service-jwt 只授予需要的 audience，且不出现在浏览器。
- [ ] 文件下载每次经过 AI-PM 业务授权。
- [ ] 预签名 URL 使用短 TTL，不写日志。
- [ ] 公告已读和 AI-PM 任务已读分别存储。
- [ ] WebSocket 断线后通过 REST 补齐数据。
- [ ] AI-PM 自己的审批/Agent/项目操作有独立审计记录。
