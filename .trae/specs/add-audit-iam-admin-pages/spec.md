# Audit & IAM 管理前端页面 Spec

## Why
当前 frontend 仅 token-client 页面对接真实后端，audit-service 与 iam-service 的能力没有可视化入口；user/role/menu/dept 页面仍使用本地 mock。需补齐审计相关与 IAM 真实数据管理界面。

## What Changes
- 前端新增"系统监控"一级菜单，包含：
  - 操作日志（对接 audit-service `/audit/operation`）
  - 登录日志（对接 audit-service `/audit/login`）
  - 数据保留（对接 audit-service `/audit/retention/*`）
- 前端将"系统管理"下既有 user/role/menu/dept 接入 iam-service 真实接口（`/iam/users`、`/iam/roles`、`/iam/permissions`、`/iam/menus`、`/iam/org-units`），替换 mock。
- 新增独立菜单"权限管理"（对接 `/iam/permissions`）。
- 新增前端 API 模块 `frontend/src/api/audit.ts`、`frontend/src/api/iam.ts`。
- 新增/更新 i18n 文案与路由 enums。
- 不修改 audit-service / iam-service 后端代码（接口已就绪）。

## Impact
- 受影响 specs: 前端系统管理菜单、监控菜单
- 受影响代码:
  - `frontend/src/api/audit.ts`（新增）
  - `frontend/src/api/iam.ts`（新增）
  - `frontend/src/views/monitor/operation-log/`（新增）
  - `frontend/src/views/monitor/login-log/`（新增）
  - `frontend/src/views/monitor/retention/`（新增）
  - `frontend/src/views/system/permission/`（新增）
  - `frontend/src/views/system/user/`（改造对接 iam）
  - `frontend/src/views/system/role/`（改造对接 iam）
  - `frontend/src/views/system/menu/`（改造对接 iam）
  - `frontend/src/views/system/dept/`（改造对接 iam org-units）
  - `frontend/src/router/modules/system.ts`、`router/modules/monitor.ts`（新增）、`router/enums.ts`
  - `frontend/locales/zh-CN.yaml`、`en.yaml`
  - `docker/config/api-gateway/routes/*.router.toml`（如未暴露 audit/iam 管理路由需补充）

## ADDED Requirements

### Requirement: 操作日志查询页面
frontend SHALL 在监控菜单下提供"操作日志"页面。

#### Scenario: 查询列表
- **WHEN** 进入 `/monitor/operation-log`
- **THEN** 调用 `GET /audit/operation`，按 user_id / action / resource / 分页筛选展示

#### Scenario: 查看详情
- **WHEN** 点击行
- **THEN** 抽屉展示 before_value / after_value / request_id / ip / user-agent

### Requirement: 登录日志查询页面
frontend SHALL 在监控菜单下提供"登录日志"页面。

#### Scenario: 查询列表
- **WHEN** 进入 `/monitor/login-log`
- **THEN** 调用 `GET /audit/login`，分页展示 username / success / fail_reason / ip / location / created_at

### Requirement: 数据保留策略页面
frontend SHALL 在监控菜单下提供"数据保留"页面。

#### Scenario: 查看统计与策略
- **WHEN** 进入 `/monitor/retention`
- **THEN** 调用 `GET /audit/retention/stats` 与 `GET /audit/retention/policy` 展示热/温/冷数据计数与策略表单

#### Scenario: 更新策略
- **WHEN** 编辑表单后提交
- **THEN** 调用 `PUT /audit/retention/policy` 保存

#### Scenario: 手动清理
- **WHEN** 点击"立即清理"并确认
- **THEN** 调用 `POST /audit/retention/cleanup` 并提示删除条数

### Requirement: 权限管理页面
frontend SHALL 在系统管理菜单下提供"权限管理"页面，支持 CRUD。

#### Scenario: 列出权限
- **WHEN** 进入 `/system/permission`
- **THEN** 调用 `GET /iam/permissions` 展示权限码、名称、描述

#### Scenario: 新增 / 编辑 / 删除
- **WHEN** 操作对应按钮
- **THEN** 调用 `POST/PUT/DELETE /iam/permissions[/:id]`

## MODIFIED Requirements

### Requirement: 用户管理页面
frontend `/system/user` SHALL 通过 iam-service 真实接口加载与维护数据，不再使用 mock。

#### Scenario: 列出用户
- **WHEN** 进入页面
- **THEN** 调用 `GET /iam/users` 分页展示

#### Scenario: 创建 / 更新 / 删除用户
- **WHEN** 操作弹窗
- **THEN** 调用 `POST /iam/users`、`PUT /iam/users/:id`、`DELETE /iam/users/:id`

### Requirement: 角色管理页面
frontend `/system/role` SHALL 对接 iam-service。

#### Scenario: CRUD 与绑定权限
- **WHEN** 操作页面
- **THEN** 调用 `GET/POST/PUT/DELETE /iam/roles[/:id]`，并通过 `PUT /iam/roles/:id/permissions` 绑定权限

### Requirement: 菜单管理页面
frontend `/system/menu` SHALL 通过 `GET/POST/PUT/DELETE /iam/menus[/:id]` 维护树形菜单。

### Requirement: 部门管理页面
frontend `/system/dept` SHALL 改对接 `/iam/org-units`，维护组织树。

## REMOVED Requirements
无
