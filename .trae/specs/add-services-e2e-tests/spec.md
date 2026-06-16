# 微服务 API E2E 测试补全 Spec

## Why
现有 `tests/e2e` 仅覆盖 iam-service 的 auth/users/roles/permissions 与 token-service 部分接口；audit、config、file、notification、token 管理端、iam 的 menus/org-units、api-gateway 健康路由均无 e2e。需补齐以保障微服务联调质量。

## What Changes
- 新增 `audit.e2e.test.ts`：覆盖审计操作日志 / 登录日志 / 导出 / 保留策略
- 新增 `config.e2e.test.ts`：覆盖命名空间 / 配置项 CRUD / watch 长轮询
- 新增 `file.e2e.test.ts`：覆盖文件上传 / 列表 / 下载 / 元数据 / 删除 / 预签名
- 新增 `notification.e2e.test.ts`：覆盖公告创建 / 发布 / 撤销 / 列表 / 详情 / 已读 / 未读数
- 新增 `token-admin.e2e.test.ts`：覆盖 OAuth 客户端 CRUD（/api/v1/admin/oauth-clients）+ JWKS + user-jwt
- 新增 `iam-menus.e2e.test.ts`：覆盖 iam 菜单 CRUD
- 新增 `iam-org-units.e2e.test.ts`：覆盖 iam 组织 CRUD
- 新增 `gateway.e2e.test.ts`：覆盖 api-gateway 健康路由与未授权拦截
- 复用现有 `helpers/http.ts`、`helpers/auth.ts`、`fixtures/payload.ts`，扩展 fixtures
- `package.json` 增加分组脚本

## Impact
- 影响目录：`tests/e2e/`
- 不修改任何服务源码（`services/*/src`）
- 影响 CI：e2e 通过 `pnpm run test:e2e` 触发

## ADDED Requirements

### Requirement: Audit Service E2E
The system SHALL verify audit-service 的 7 类核心 API 在 gateway 上可访问。

#### Scenario: 创建并查询操作日志
- **WHEN** admin POST `/audit/operation` 携带合法 body
- **THEN** 返回 200 + `id`，随后 GET `/audit/operation` 列表中可见

#### Scenario: 查询保留策略
- **WHEN** admin GET `/audit/retention/policy`
- **THEN** 返回 200，body 含 `hot_days`/`warm_days`/`cold_days`

### Requirement: Config Service E2E
The system SHALL verify 命名空间与配置项 CRUD。

#### Scenario: 命名空间创建/查询/删除
- **WHEN** 顺序调用 POST/GET/DELETE `/config/namespace`
- **THEN** 各步均返回 2xx

#### Scenario: 配置项 CRUD
- **WHEN** 在已有 namespace 下创建/读取/更新/删除 ConfigItem
- **THEN** 状态码 2xx，version 单调递增

### Requirement: File Service E2E
The system SHALL verify 文件上传元数据与预签名。

#### Scenario: 上传元数据并查询信息
- **WHEN** POST `/file/upload` body 含 `filename`
- **THEN** 返回 200，body 含 `id`，随后 GET `/file/{id}/info` 返回 200

#### Scenario: 创建预签名 URL
- **WHEN** POST `/file/presign` 携带 `file_id` 与 `operation: get`
- **THEN** 返回 200，body 含 `url`

### Requirement: Notification Service E2E
The system SHALL verify 公告全生命周期。

#### Scenario: 创建发布撤销
- **WHEN** admin 顺序调用 POST 创建 → publish → revoke
- **THEN** 各步返回 200/201

#### Scenario: 查询自己的公告与未读数
- **WHEN** GET `/api/notifications/announcements` 与 `/api/notifications/unread-count`
- **THEN** 返回 200

### Requirement: Token Admin E2E
The system SHALL verify OAuth 客户端管理与 JWKS。

#### Scenario: OAuth 客户端 CRUD
- **WHEN** POST/GET/PUT/DELETE `/api/v1/admin/oauth-clients`
- **THEN** 各步返回 2xx

#### Scenario: JWKS 端点
- **WHEN** GET `/.well-known/jwks.json`
- **THEN** 返回 200，body 含 `keys` 数组

### Requirement: IAM Menus & OrgUnits E2E
The system SHALL verify 菜单与组织 CRUD。

#### Scenario: 菜单 CRUD
- **WHEN** 顺序调用 POST/GET/PUT/DELETE `/iam/menus`
- **THEN** 各步返回 2xx

#### Scenario: 组织 CRUD
- **WHEN** 顺序调用 POST/GET/PUT/DELETE `/iam/org-units`
- **THEN** 各步返回 2xx

### Requirement: API Gateway E2E
The system SHALL verify gateway 健康路由与拦截。

#### Scenario: 健康检查
- **WHEN** GET `/iam/health`、`/audit/health`
- **THEN** 返回 200

#### Scenario: 未授权访问受保护路由
- **WHEN** 不带 token GET `/api/v1/users`
- **THEN** 返回 401

## MODIFIED Requirements
无

## REMOVED Requirements
无
