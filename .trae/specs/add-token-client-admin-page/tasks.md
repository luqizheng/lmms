# Tasks

- [ ] Task 1: 后端补全 oauth_client_svc CRUD
  - [ ] 1.1 新增 `list(pool, keyword, status, page, size)` 返回 `(Vec<Client>, total)`
  - [ ] 1.2 新增 `update(pool, client_id, patch)`，仅改 name/permissions/grant_types/TTL/status
  - [ ] 1.3 新增 `delete(pool, client_id)`（硬删除）
  - [ ] 1.4 新增 `reset_secret(pool, client_id) -> 明文 secret`

- [ ] Task 2: 后端管理路由 `routes/admin.rs`
  - [ ] 2.1 定义请求/响应 DTO，移除 `client_secret_hash` 字段
  - [ ] 2.2 实现 GET 列表、POST 创建、PUT 更新、DELETE 删除、POST reset-secret
  - [ ] 2.3 创建时使用 `secret::generate_client_secret()` 与 `hash_client_secret()`
  - [ ] 2.4 在 `routes/mod.rs` 与 `lib.rs` 注册 `/api/v1/admin/oauth-clients` 路由

- [ ] Task 3: api-gateway 路由放行
  - [ ] 3.1 在 `docker/config/api-gateway/routes/token-service.router.toml` 增加 admin 前缀路由（如已存在 token-service 路由文件则追加）
  - [ ] 3.2 跑 `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`

- [ ] Task 4: 前端 API 层
  - [ ] 4.1 在 `frontend/src/api/token.ts` 增加 listOAuthClients/createOAuthClient/updateOAuthClient/deleteOAuthClient/resetOAuthClientSecret 与类型

- [ ] Task 5: 前端页面 `views/system/token-client/`
  - [ ] 5.1 `index.vue` 列表+筛选+分页+操作列（参考 `views/system/role/index.vue`）
  - [ ] 5.2 `form.vue` 新增/编辑表单（name、permissions 多选标签、grant_types 多选、TTL、status）
  - [ ] 5.3 `utils/hook.tsx`、`utils/rule.ts`、`utils/types.ts`
  - [ ] 5.4 创建成功后弹窗一次性展示 client_id + client_secret，带复制按钮

- [ ] Task 6: 路由与菜单
  - [ ] 6.1 `router/modules/system.ts` 新增子路由 `/system/token-client`
  - [ ] 6.2 `locales/zh-CN.yaml`、`en.yaml` 增加 `menus.systemTokenClient` 文案

- [ ] Task 7: 验证
  - [ ] 7.1 启动 token-service，curl 验证 5 个管理接口
  - [ ] 7.2 启动 frontend，进入页面完成一次完整 CRUD + reset-secret 流程

# Task Dependencies
- Task 2 依赖 Task 1
- Task 3 依赖 Task 2
- Task 4 依赖 Task 2
- Task 5 依赖 Task 4
- Task 6 依赖 Task 5
- Task 7 依赖 Task 3、Task 6
