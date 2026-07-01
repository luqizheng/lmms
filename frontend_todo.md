# frontend_todo.md

> 后端 ↔ 前端 对接缺口盘点（截至 2026-07-01）。
> 盘点范围：`services/` 下全部业务服务 × `frontend/src/{api,views,router}` 全量。
> 路由拓扑：前端 `VITE_API_BASE=/api` → nginx `rewrite ^/api/(.*)$ /$1` → api-gateway `/v1/*` → 业务服务。
> 排序：每节内按 **P0 → P1 → P2** 列。

---

## 0. TL;DR

| 服务 | 后端 | api-gateway 注册 | 前端 API | 前端视图 | 评级 |
| --- | --- | --- | --- | --- | --- |
| iam-service | ✅ | ✅ `iam.router.toml` | ✅ `api/iam.ts` `api/auth.ts` `api/role.ts` | ✅ `views/system/{user,role,permission,menu,dept}` | **部分**（sessions/tenants/register 缺） |
| token-service | ✅ | ✅ `token.router.toml` | ✅ `api/token.ts` | ✅ `views/system/token-client` | **部分**（API key 列表缺） |
| audit-service | ✅ | ✅ `audit.router.toml` | ✅ `api/audit.ts` `api/audit/*` | ✅ `views/monitor/{login-log,operation-log,online,retention,logs}` | **部分**（合规导出按钮缺） |
| notification-service | ✅ | ✅ `notification.router.toml` | ✅ `api/notification.ts` | ✅ `views/notification/{inbox,admin,detail}` | **基本完整** |
| prue_backend | ✅ | ✅ `prue_backend.router.toml` | ✅ `api/menu-config.ts` | ✅ `views/system/menu/` | **已完整**（1430 行） |
| file-service | ✅ | ✅ `file.router.toml` | ❌ | ❌ | **视图/API 待补**（gateway 已通） |
| config-service | ✅ | ✅ `config.router.toml` | ❌ | ❌ | **视图/API 待补**（gateway 已通，SSE 除外） |

**已解锁（2026-07-01）**：file-service 与 config-service 已注册到 api-gateway（`[egress_map]` override + `file.router.toml` + `config.router.toml`，详见 §3「已落地」）。仅剩 SSE `/v1/config/watch` 暂缓（gateway 自身 feature gap）。

---

## 1. 完全无前端的服务（P0）

### 1.1 file-service 【P0】

后端端点（路径前缀 `file.rs` 已挂 `/upload` `/list` `/:id` `/:id/info`；`presign.rs` 挂 `/presign`）：

| 方法 | 路径 | 后端 handler | 前端状态 |
| --- | --- | --- | --- |
| POST | `/file/upload` | `upload_file` | ❌ |
| GET | `/file/list` | `query_files` | ❌ |
| GET | `/file/:id` | `download_file` | ❌ |
| DELETE | `/file/:id` | `delete_file` | ❌ |
| GET | `/file/:id/info` | `get_file_info` | ❌ |
| POST | `/file/presign` | `create_presign` | ❌ |

待补充：

- **新建** `frontend/src/api/file.ts`：6 个函数 + 6 个 DTO 接口。
- **新建** `frontend/src/views/file/index.vue`：列表（按大小/类型/上传时间/上传者筛选、删除确认）。
- **上传抽屉**：合并到 `index.vue` 的 `<el-upload>` 抽屉，或单列 `upload.vue`。
- **可选** `frontend/src/views/file/presign.vue`：外部上传凭证生成页。
- **新增菜单** `frontend/src/router/modules/file.ts`（`/file`），挂到「系统管理」或新建一级「文件中心」。
- **权限点**：在 `contracts/permissions.yaml` 注册 `file:read` / `file:upload` / `file:delete`，按钮挂 `v-perms`。
- **网关前置**（**已解锁 2026-07-01，SSE 除外**）：见 §3「已落地」。`file.router.toml` + `egress_map` override 已就位。

### 1.2 config-service 【P0】

后端端点：

| 文件 | 端点 | handler |
| --- | --- | --- |
| `config.rs` | POST/GET `/config/item` | `create_config_item` / `query_config_items` |
| `config.rs` | GET/PUT/DELETE `/config/item/:id` | `get/update/delete_config_item` |
| `config.rs` | GET `/config/export/:namespace_id` | `export_config` |
| `config.rs` | POST `/config/import/:namespace_id` | `import_config` |
| `namespace.rs` | POST/GET `/config/namespace` | `create_namespace` / `query_namespaces` |
| `namespace.rs` | GET/PUT/DELETE `/config/namespace/:id` | `get/update/delete_namespace` |
| `watch.rs` | GET `/config/watch` | `watch_config`（SSE） |

待补充：

- **新建** `frontend/src/api/config.ts`：13 个函数 + namespace/config item 接口。
- **新建** `frontend/src/views/config/namespace/index.vue`：命名空间 CRUD。
- **新建** `frontend/src/views/config/item/index.vue`：配置项列表（按 namespace tab 切换，支持 key/value/type/分组筛选）。
- **可选** `frontend/src/views/config/item/diff.vue`：导入前 JSON diff 预览。
- **新增菜单** `frontend/src/router/modules/config.ts`（`/config`），挂到「系统管理」。
- **权限点**：`config:namespace:*`、`config:item:*`。
- **SSE `/watch`**：注意 `EventSource` 不支持自定义 header，Bearer 不能塞 header，必须走 `?token=` query；后端 `watch_config` 需配合改 query 取 token（先看实现）。
- **网关前置**（**已解锁 2026-07-01，SSE 除外**）：见 §3「已落地」。`config.router.toml` + `egress_map` override 已就位。`/v1/config/watch` SSE 需独立 plan（gateway `forward_to_service` 全量 buffer 响应 + `RouteConfig.ws` 字段无消费者）。

---

## 2. 已对接服务的端点缺口

### 2.1 iam-service 【P0 / P1 混合】

已对接：用户、角色、权限、菜单、组织单元、登录/登出/改密（`iam.ts` / `auth.ts` / `role.ts` / `iam-scopes.ts`）。

#### 后端有但前端缺的端点

| 端点 | 后端 handler | 优先级 | 说明 |
| --- | --- | --- | --- |
| POST `/v1/iam/tenants` | `register_tenant` | P0 | 多租户注册无前端；超级管理员初始化租户无入口 |
| POST `/v1/iam/auth/register` | `register` | P1 | `auth.ts` 已实现函数但无 UI（取决于是否启用自助注册） |
| GET `/v1/iam/auth/setup-status` | `setup_status` | P1 | 首次部署状态检查无 UI |
| GET `/v1/iam/auth/sessions` | `list_sessions` | P1 | 在线会话列表无 UI（`monitor/online` 是另一数据源） |
| DELETE `/v1/iam/auth/sessions/{id}` | `revoke_session` | P1 | 踢人下线无 UI |
| PUT `/v1/iam/users/{id}/roles` | `assign_user_roles` | P0 | **Phase 4.4-fu1 端点已落**，前端 `form/role.vue` 当前只读未提交，需联通 |
| PUT `/v1/iam/org-units/{id}/users` | `assign_users` | P1 | 组织单元成员指派无 UI |
| GET `/internal/users/{id}/claims` | `get_user_claims` | P2 | 内部端点，前端不直接调用 |
| PUT `/v1/iam/auth/password` | `change_password` | P2 | 已有 `changeIamPassword`，但 `account-settings` 页是否挂载待查 |

待补充：

- **新建** `frontend/src/views/system/tenant/index.vue`：租户列表 + 注册抽屉（P0）。
- **改造** `frontend/src/views/system/user/form/role.vue`：连 `PUT /v1/iam/users/{id}/roles`（P0，承接 4.4-fu1）。
- **新建** `frontend/src/views/system/session/index.vue`：在线会话管理（设备/IP/最后活跃/踢下线，P1）。
- **改造** `frontend/src/views/system/dept/index.vue`：增加部门成员指派面板（P1）。

### 2.2 token-service 【P1】

已对接：OAuth 客户端 CRUD + secret 重置（`token.ts`）；API key 创建（无列表）；token resource 探针。

#### 后端有但前端缺的端点

| 端点 | 后端 handler | 优先级 | 说明 |
| --- | --- | --- | --- |
| GET `/v1/admin/api-keys`（全量列表） | 无独立 handler | P1 | `token.ts:createApiKey` 只创不列；需后端补 GET 或前端本地缓存 |
| POST `/v1/tokens/user_jwt` | `handle_user_jwt` | P2 | gateway 内部用，前端不调 |
| GET `/v1/tokens/resource` | `handle_resource` | ✅ | 已通过 `listResources()` 消费 |

待补充：

- **改造** `frontend/src/views/system/token-client/index.vue`：增加「API Key 管理」tab（依赖后端补 list 端点，或简化为「创建即显示一次 + 本地保存」）。
- **P2** 不动。

### 2.3 audit-service 【P1】

已对接：登录日志、操作日志、保留策略、在线用户（`audit.ts` + `audit/*` 自动生成）。

#### 后端有但前端缺的端点

| 端点 | 后端 handler | 优先级 | 说明 |
| --- | --- | --- | --- |
| GET `/v1/audit/export/compliance/operation/csv` | `export_operation_csv` | P1 | 合规 CSV 导出，前端无下载按钮 |
| GET `/v1/audit/export/compliance/operation/json` | `export_operation_json` | P1 | 同上 JSON |
| GET `/v1/audit/export/compliance/login/csv` | `export_login_csv` | P1 | 登录日志合规导出 |
| GET `/v1/audit/retention/stage/{created_at}` | `get_data_stage` | P2 | 阶段水位查询，前端通常不展示 |

> 网关当前 `audit.router.toml` 有 `/v1/audit/export/{rest}` 单段 `{rest}`，能否匹配 `compliance/operation/csv` 多段路径需实测确认；不匹配则需补 `{rest:.*}` 或拆多段路由。

待补充：

- **改造** `frontend/src/views/monitor/operation-log/index.vue`：增加「合规导出 CSV / JSON」按钮。
- **改造** `frontend/src/views/monitor/login-log/index.vue`：增加「合规导出 CSV」按钮。

### 2.4 notification-service 【基本完整 / P2 微调】

已对接：公告 CRUD、发布、撤销、未读数、标记已读（`notification.ts`）。

- WS `/v1/notification/ws` 走 store 自管，无需补页面。
- inbox/admin 已闭环。
- **P2 可选**：inbox「按已读/未读 tab 切换」的 UI 改进。

---

## 3. 网关拓扑决策点

`api-gateway` 的 `egress_map` 当前只包含 `iam-service` / `token-service` / `notification-service` / `audit-service` / `prue_backend`（`services/api-gateway/src/config.rs:97-101`）。`infrastructure/config/api-gateway/routes/` 也只有这五个 + 旧版 `user/roles/permissions/auth` 的 `.router.toml`。

新增 file / config 对接前需三选一：

| 方案 | 改动 | 优 | 劣 |
| --- | --- | --- | --- |
| A. 走 api-gateway | 加 `file.router.toml` + `config.router.toml` + `egress_map` 两行 + 鉴权策略 | 鉴权/JWT/限流一致；前端代码统一走 `/api/v1/*` | 网关配置改动 + 服务网络可达性要求 |
| B. nginx 直连上游 | 在 `frontend/nginx.conf` 加 `location /api/file/` → `proxy_pass http://file-service:port`（同样 `/api/config/`） | 改动小，不动网关 | 鉴权需自己加中间件；破坏「前端只知 `/api/*`」约定 |
| C. 走网关但用 public 标记 | A 的子集，`public=true` 绕过鉴权 | 适合内部管理 UI | file/config 通常需登录态，public 不适用 |

**已落地（2026-07-01）**：选 A 方案。详见 `docs/superpowers/plans/2026-07-01-gateway-file-config-topology-plan.md`。
- `infrastructure/config/api-gateway/api-gateway.toml` 追加 `[egress_map]` override：`file-service:8082` / `config-service:8081`。
- `routes/file.router.toml` (5 路由) + `routes/config.router.toml` (7 路由) 共 12 条；非 health 路由走 JWT 中间件。
- `rate-limits.toml` 补 4 条 config 限流（file/upload 沿用旧规则）。
- `docker-compose-services.yml` 的 `api-gateway` 服务 volumes 补 `api-gateway.toml` 的 bind-mount（无此 mount override 不会进容器，会触发 `no egress configured` 错误）。
- 端到端 401 测试 8/8 通过：`file.list/upload/presign/{id}` + `config.namespace/item/{id}/export`。
- 端到端 200 测试阻塞：file/config 服务容器因 Docker 构建网络问题（`rsproxy.cn` 解析失败）尚未上线，前端在文件/配置服务起来后可直连 `/api/v1/file/*` `/api/v1/config/*`。

**SSE 暂缓**：`/v1/config/watch` 未加入 `config.router.toml`，原因是当前 api-gateway 对上游响应全量 buffer（`router::forward_to_service:41-57`）+ `RouteConfig.ws` 字段全代码库无消费者（仅 config.rs 单测断言）。SSE 端到端是 gateway 自身 feature gap，待独立 plan。前端 `views/config/item/*` 的实时推送功能需等该 plan 落地。

---

## 4. 横切 & 平台级缺口

### 4.1 Phase 4 后续项

| 项 | 来源 | 优先级 | 前端动作 |
| --- | --- | --- | --- |
| 4.3-fu2 role/dept 收件人解析 | TODO.md | P0 | 公告编辑页 `views/notification/admin/*` 增加「按角色/部门」收件人选择器（依赖 iam 成员查询端点） |
| 4.4-fu1 user-roles assign | TODO.md | P0 | 联通 `form/role.vue`（见 §2.1） |
| 4.4-fu2 actor_id 透传 | TODO.md | P2 | 仅后端 |
| 4.4-fu3 perm_epoch 本地缓存 | TODO.md | P2 | 仅后端 gateway |

### 4.2 平台级补全

- **错误码前端字典**：`libs/common-error/src/codes.rs` 已有常量，需前端 `src/utils/error-codes.ts` 镜像 + i18n 文案；目前各页错误是裸英文/数字。
- **`v-auth` / `v-perms` 全量回归**：确认 `system/user/form/role.vue` 等按钮已挂权限指令；新页面默认引入。
- **菜单入口整理**：`router/modules/monitor.ts` `notification.ts` `system.ts` 完整性需校验（`monitor/` 5 页是否都在 sidebar 注册）。
- **nginx.conf `/api/v1/file` `/api/v1/config` 转发**：选 A 方案则 nginx 不动；选 B 方案需补 location。
- **AGENTS.md 文档漂移**：当前 `frontend/AGENTS.md` 写的 `/api/v1/<service>/<resource>` 与实际 axios `baseURL + path` 行为一致，但路径前缀 `/api/` 在 axios `baseURL` 已含，调用时路径写 `/v1/iam/...`（注意不是 `/api/v1/...`），需要新人 onboarding 时明确。

---

## 5. 落地建议顺序

1. **第一批（P0，修缺位 + 拓扑决策）**
   - §3 网关拓扑决策（先定 A/B/C）
   - §1.1 file-service 全套（依 A 方案含 `file.router.toml`）
   - §1.2 config-service 全套（不含 SSE watch，先做 namespace/item CRUD）
   - §2.1 iam tenant 注册 + user-roles 联通
   - §4.1 4.3-fu2 公告收件人选择器

2. **第二批（P1，闭环）**
   - §2.1 iam sessions 管理 + org-unit 成员指派
   - §2.2 token-service API key 列表（先看后端是否补 list 端点）
   - §2.3 audit 合规导出按钮
   - §4.2 错误码字典 + i18n

3. **第三批（P2，锦上添花）**
   - §1.2 config `/watch` SSE
   - §2.2 user_jwt / resource 探针 UI
   - §2.3 retention stage 可视化
   - inbox 已读/未读 tab 改进

---

## 6. 不在本文档范围

- 后端 only 的 Phase 5/6/7/8 路线图（事件重试、可观测性等基础设施升级）见 `TODO.md`。
- 前端组件库 / 主题 / i18n 文案本身的 polish 不属于「对接缺口」。
- 旧 pure-admin 遗留 mock 端点（`system.ts` 的 `/user` `/list-all-role` 等）需后续清理，但与后端对接无关，不在本文档列项。
- 新增业务服务尚未启动，不在盘点范围。