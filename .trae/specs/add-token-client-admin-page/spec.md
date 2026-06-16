# Token Service 客户端管理前端页面 Spec

## Why
当前 token-service 的 OAuth 客户端（`oauth_clients` 表 / `docker/config/token-service/clients.json`）只能通过 SQL 或 JSON 配置维护，缺少可视化管理。需提供前端页面对客户端进行 CRUD。

## What Changes
- 前端新增"系统管理 → 令牌客户端"页面，支持列表、新增、编辑、启用/禁用、删除。
- token-service 新增管理后台 REST API：`GET/POST/PUT/DELETE /api/v1/admin/oauth-clients`。
- 复用现有 `oauth_clients` 表，无 schema 变更。
- 新建 client 时由后端生成 `client_secret`，仅在创建响应中明文返回一次。

## Impact
- 受影响 specs: token-service 管理能力、frontend 系统菜单
- 受影响代码:
  - `services/token-service/src/routes/admin.rs`（新增）
  - `services/token-service/src/routes/mod.rs`、`lib.rs`（路由注册）
  - `services/token-service/src/service/oauth_client_svc.rs`（list/update/delete）
  - `frontend/src/api/token.ts`（新增管理 API）
  - `frontend/src/views/system/token-client/`（新增页面）
  - `frontend/src/router/modules/system.ts`、`router/enums.ts`
  - `frontend/locales/zh-CN.yaml`、`en.yaml`
  - `docker/config/api-gateway/routes/*.router.toml`（如需暴露管理路由）

## ADDED Requirements

### Requirement: OAuth 客户端管理 API
token-service SHALL 提供管理端 REST 接口用于 CRUD `oauth_clients`。

#### Scenario: 列出客户端
- **WHEN** 调用 `GET /api/v1/admin/oauth-clients?keyword=&status=&page=&size=`
- **THEN** 返回分页客户端列表，字段不含 `client_secret_hash`

#### Scenario: 创建客户端
- **WHEN** 提交 `POST /api/v1/admin/oauth-clients` 含 name、permissions、grant_types、TTL
- **THEN** 后端生成 `client_id` 与明文 `client_secret`，存储 hash，仅本次返回明文 secret

#### Scenario: 更新客户端
- **WHEN** 调用 `PUT /api/v1/admin/oauth-clients/{client_id}`
- **THEN** 更新名称、权限、grant_types、TTL、status；不修改 secret

#### Scenario: 重置 secret
- **WHEN** 调用 `POST /api/v1/admin/oauth-clients/{client_id}/reset-secret`
- **THEN** 生成新 secret 并返回明文一次

#### Scenario: 删除客户端
- **WHEN** 调用 `DELETE /api/v1/admin/oauth-clients/{client_id}`
- **THEN** 软删除（status=deleted）或硬删除该客户端

### Requirement: 前端令牌客户端管理页面
frontend SHALL 在系统菜单下提供"令牌客户端"页面。

#### Scenario: 浏览列表
- **WHEN** 进入 `/system/token-client`
- **THEN** 展示客户端表格，含 client_id、名称、grant_types、状态、创建时间，支持关键字+状态筛选+分页

#### Scenario: 新增客户端
- **WHEN** 点击"新增"提交表单
- **THEN** 弹窗显示一次性 client_secret，提示用户复制保存

#### Scenario: 编辑/启用禁用/删除/重置 secret
- **WHEN** 点击行操作
- **THEN** 调用对应 API 并刷新列表

## MODIFIED Requirements
无

## REMOVED Requirements
无
