# 04. 用户、角色、权限与租户

## 4.1 IAM 的职责

iam-service 负责身份主体和授权基础数据，包括用户、角色、权限、组织单元和租户。上层业务系统应使用 IAM 的身份与权限作为认证上下文，但仍需在自己的业务域判断资源级访问权限。

## 4.2 RBAC 概念

- **User**：登录主体。
- **Role**：一组权限的集合。
- **Permission**：可执行的动作标识。
- **Organization Unit / Department**：组织层级及其用户归属。
- **Tenant**：多租户隔离边界。

角色是权限的聚合，不等于业务资源的所有权。拥有 `PROJECT_READ` 类权限不自动意味着可以读取任意项目；业务服务仍要校验当前用户是否属于目标项目/资源范围。

## 4.3 权限码

权限码单一真相源位于 `contracts/permissions.yaml`。格式约定为 `SERVICE_RESOURCE_ACTION`，例如：

- `IAM_USER_READ`
- `IAM_ROLE_ASSIGN`（具体是否存在应以字典为准）
- `AUD_LOG_READ`
- `FILE_READ`、`FILE_UPLOAD`、`FILE_DOWNLOAD`、`FILE_DELETE`
- `NTF_MESSAGE_READ`
- `TOKEN_BOOTSTRAP_CREATE`
- `TOKEN_SERVICE_JWT_GRANT_READ`

上例仅作命名解释；**实际可用权限码必须查当前 YAML**。不要在前端、后端或 AI-PM 中自行创造看似合理但不存在的权限码。

## 4.4 UI 权限呈现建议

- 登录后获取当前用户/角色/权限数据（具体端点以 IAM OpenAPI 为准）。
- 前端可用权限码隐藏无权菜单、按钮和操作入口。
- 前端权限判断只是体验层；后端每次写操作都必须再次鉴权。
- 对资源级操作，后端同时校验用户权限、租户边界、资源归属和业务状态。
- 403 应显示无权限，不要仅靠隐藏按钮保证安全。
- 权限变化可能触发令牌权限版本更新/失效；客户端应正确处理后续 401/403 并重新获取会话上下文。

## 4.5 多租户

当前平台采用共享数据库、业务数据按 tenant_id 隔离的架构。用户 JWT 带有租户上下文；缺少必要租户声明时请求可能被拒绝。跨租户能力是受控的 platform-admin 能力，不应由客户端自行设置布尔值绕过隔离。

集成要求：

1. 业务记录保存 tenant_id。
2. tenant_id 从已认证身份和业务上下文确定，不能信任任意客户端传入值。
3. 所有 list/get/update/delete 查询都必须限定租户范围。
4. 后端检查资源是否属于当前租户。
5. 管理员跨租户操作必须有明确授权、审计和操作理由。
6. 不要用数据库自增 ID 或 UUID 难猜来替代租户隔离。

## 4.6 管理 UI 的模块建议

| UI 模块 | 主要数据来源 | 注意事项 |
|---|---|---|
| 用户管理 | IAM 用户 API | 分页、禁用状态、租户过滤 |
| 角色管理 | IAM 角色 API | 角色权限变更影响用户有效权限 |
| 权限字典 | contracts/permissions.yaml / IAM 查询 API | 字典展示与实际授权操作分离 |
| 组织架构 | IAM 组织单元 API | 部门树和用户归属需要一致性 |
| 租户管理 | IAM 租户 API | 平台级高权限区域 |
| 会话管理 | IAM sessions API | 只允许用户管理自身会话，除非有管理端点授权 |

菜单配置服务 prue_backend 与 IAM 权限是不同关注点。菜单是否可见不能替代 API 授权。

## 4.7 相关依据

- `contracts/permissions.yaml`
- `services/iam-service/openapi.yaml`
- `services/iam-service/README.md`
- `docs-site/ai-context.md` 中多租户和 JWT 事实
