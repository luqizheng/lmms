# frontend_todo.md

> 后端 ↔ 前端 对接缺口盘点（截至 2026-07-01）。
> 盘点范围：`services/` 下全部业务服务 × `frontend/src/{api,views,router}` 全量。
> 路由拓扑：前端 `VITE_API_BASE=/api` → nginx `rewrite ^/api/(.*)$ /$1` → api-gateway `/v1/*` → 业务服务。
> 排序：每节内按 **P0 → P1 → P2** 列。

---

## 0. TL;DR

| 服务 | 后端 | api-gateway 注册 | 前端 API | 前端视图 | 评级 |
| --- | --- | --- | --- | --- | --- |
| iam-service | ✅ | ✅ `iam.router.toml` | ✅ `api/iam.ts` `api/auth.ts` `api/role.ts` | ✅ `views/system/{user,role,permission,menu,dept,tenant,session}` | ✅ **已完整** |
| token-service | ✅ | ✅ `token.router.toml` | ✅ `api/token.ts`（含 listApiKeys） | ✅ `views/system/token-client`（含 api-keys / resource-probe） | ✅ **已完整** |
| audit-service | ✅ | ✅ `audit.router.toml` | ✅ `api/audit.ts`（含导出 / stage） | ✅ `views/monitor/{login-log,operation-log,online,retention,logs}`（含导出按钮） | ✅ **已完整** |
| notification-service | ✅ | ✅ `notification.router.toml` | ✅ `api/notification.ts` | ✅ `views/notification/{inbox（tabs）,admin,detail}` | ✅ **已完整** |
| prue_backend | ✅ | ✅ `prue_backend.router.toml` | ✅ `api/menu-config.ts` | ✅ `views/system/menu/` | ✅ **已完整**（1430 行） |
| file-service | ✅ multipart | ✅ `file.router.toml` | ✅ `api/file.ts`（multipart + limits） | ✅ `views/file/{index,upload,presign}` | ✅ **已完整** |
| config-service | ✅ reveal-aware | ✅ `config.router.toml` + SSE ws=true | ✅ `api/config.ts`（含 masking / sse） | ✅ `views/config/{namespace,item}` | ✅ **已完整**（SSE 激活） |

**全部对接完成（2026-07-02）**：7 个服务全部接入端到端。后端 4 个 gap（multipart upload、is_secret reveal、api-key list、gateway SSE streaming）已分别在 `fix/backend-integration-gaps` 与 `feat/sse-gateway-streaming` 分支修复并 push。SSE `/v1/config/watch` 已激活，前端 `api/config/sse.ts` `EventSource` 实打开。

> **PM 平台（研发管理）已于 2026-07-19 移除**：PM 属于独立业务项目 `/home/kudi/projects/project-manager`，不属于本基础框架范围。相关文件已删除（`views/pm/`、`api/{pm,kb,llm}.ts`、`router/modules/pm.ts`、`store/modules/pm.ts`、locales 中 PM i18n keys）。

---

## 1. 完全无前端的服务（P0）

### 1.1 file-service 【P0】

后端端点（路径前缀 `file.rs` 已挂 `/upload` `/list` `/:id` `/:id/info`；`presign.rs` 挂 `/presign`）：

| 方法 | 路径 | 后端 handler | 前端状态 |
| --- | --- | --- | --- |
| POST | `/file/upload` | `upload_file` | ✅ `api/file.ts:uploadFile` |
| GET | `/file/list` | `query_files` | ✅ `views/file/index.vue` |
| GET | `/file/:id` | `download_file` | ✅ `api/file.ts:downloadFile` |
| DELETE | `/file/:id` | `delete_file` | ✅ `views/file/index.vue` 操作列 |
| GET | `/file/:id/info` | `get_file_info` | ✅ `api/file.ts:getFileInfo` |
| POST | `/file/presign` | `create_presign` | ✅ `views/file/presign.vue` |

待补充（**2026-07-01 第二批落地**）：

- ✅ **新建** `frontend/src/api/file/` + `frontend/src/api/file.ts`：codegen 出 DTO 类型 + 手写 http.request 包装
- ✅ **新建** `frontend/src/views/file/index.vue`：列表 + 删除（按文件名筛选、删除确认）
- ✅ **上传抽屉** `frontend/src/views/file/upload.vue`（2026-07-01 第二轮）：el-upload 多选 + 客户端 `:before-upload` 校验（`checkFileSize` + `checkFileExtension`） + 临时 / 过期选项 + 提交按钮 `FILE_UPLOAD` 权限保护
- ⚠ **后端 gap**：`services/file-service/src/routes/file.rs:25-39` `upload_file` 只接 `UploadFileMeta` JSON，`Bytes::new()` 不接 binary 实际内容。upload.vue 现在仅发 metadata 占位；后端补 multipart 后 UI 自动生效，无需改前端
- ✅ **新建** `frontend/src/router/modules/file.ts`（`/file`）
- ✅ **权限点**：`contracts/permissions.yaml` 已增 `FILE_READ` + `FILE_UPLOAD` + `FILE_DELETE` + `FILE_DOWNLOAD`（按钮挂 `v-perms`/`hasPerms`）
- ✅ **网关前置**：`file.router.toml` + `egress_map` override 已就位
- ⏸ **菜单 publish**：router module 文件已就位，但 `prue_menu_configs` 需运营/手动 publish 才在 sidebar 出现（详见 §7.2.3）

### 1.2 config-service 【P0】

后端端点：

| 文件 | 端点 | handler | 前端状态 |
| --- | --- | --- | --- |
| `config.rs` | POST/GET `/config/item` | `create_config_item` / `query_config_items` | ✅ `views/config/item/index.vue` |
| `config.rs` | GET/PUT/DELETE `/config/item/:id` | `get/update/delete_config_item` | ✅ 同上 |
| `config.rs` | GET `/config/export/:namespace_id` | `export_config` | ✅ `api/config.ts:exportConfig` |
| `config.rs` | POST `/config/import/:namespace_id` | `import_config` | ✅ `api/config.ts:importConfig` |
| `namespace.rs` | POST/GET `/config/namespace` | `create_namespace` / `query_namespaces` | ✅ `views/config/namespace/index.vue` |
| `namespace.rs` | GET/PUT/DELETE `/config/namespace/:id` | `get/update/delete_namespace` | ✅ 同上 |
| `watch.rs` | GET `/config/watch` | `watch_config`（SSE） | ❌ blocked by gateway |

待补充：

待补充（**2026-07-01 第二批落地**）：

- ✅ **新建** `frontend/src/api/config/` + `frontend/src/api/config.ts`：codegen + 手写 http 包装
- ✅ **新建** `frontend/src/views/config/namespace/index.vue`：CRUD（按名称筛选 + 删除）
- ✅ **新建** `frontend/src/views/config/item/index.vue`：按 namespace tab 切换、key 筛选、敏感值 UI 掩码 + reveal 按钮（`CFG_CONFIG_REVEAL` 权限）
- ⏸ **diff.vue**：导入前 JSON diff 预览未做（依赖 JSON diff 库，YAGNI 暂不引入）
- ✅ **新建** `frontend/src/router/modules/config.ts`（`/config`）
- ✅ **权限点**：8 个码（`CFG_NAMESPACE_{CREATE,READ,UPDATE,DELETE,EXPORT,IMPORT}` + `CFG_CONFIG_REVEAL` + 既有 `CFG_CONFIG_*`）
- ⏸ **SSE `/watch`**：占位 `frontend/src/api/config/sse.ts` 已就位，gateway 上线后激活
- ✅ **网关前置**：`config.router.toml` + `egress_map` override 已就位
- ⏸ **菜单 publish**：同 §1.1

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

待补充（**2026-07-01 第二批落地**）：

- ✅ **新建** `frontend/src/views/system/tenant/index.vue`（P0）：列表 + 注册（POST /v1/iam/tenants 拉 `registerIamTenant`）
- ✅ **改造** `form/role.vue` + `hook.tsx` handleRole + `assignUserRoles`（P0，4.4-fu1）：联通 PUT /v1/iam/users/{id}/roles
- ✅ **新建** `frontend/src/views/system/session/index.vue`（P1，2026-07-01 第二轮）：在线会话列表 + 踢下线（`listIamSessions` / `revokeIamSession`，`router/modules/system.ts` 挂载 `showLink:false`）
- ✅ **改造** `views/system/dept/index.vue`（P1，2026-07-01 第二轮）：新增「分配成员」按钮 + `form/assign-users.vue` 弹窗 + `assignOrgUnitUsers` 调用

### 2.2 token-service 【P1】

已对接：OAuth 客户端 CRUD + secret 重置（`token.ts`）；API key 创建（无列表）；token resource 探针。

#### 后端有但前端缺的端点

| 端点 | 后端 handler | 优先级 | 说明 |
| --- | --- | --- | --- |
| GET `/v1/admin/api-keys`（全量列表） | 无独立 handler | P1 | `token.ts:createApiKey` 只创不列；需后端补 GET 或前端本地缓存 |
| POST `/v1/tokens/user_jwt` | `handle_user_jwt` | P2 | gateway 内部用，前端不调 |
| GET `/v1/tokens/resource` | `handle_resource` | ✅ | 已通过 `listResources()` 消费；新增 `views/system/token-client/resource-probe.vue`（2026-07-01 第二轮，路由 `showLink:false`）做可视化 |

待补充：

- ✅ **改造** token-client（2026-07-01 第二轮）：后端无 list 端点，采用「本地缓存」方案 —— `frontend/src/api/api-key-cache.ts`（localStorage），`frontend/src/views/system/token-client/api-keys.vue` 独立路由 `/system/token-client/api-keys`（showLink:false）。**待办**：后端补 GET list 后改读后端
- **P2** 不动。

### 2.3 audit-service 【P1】

已对接：登录日志、操作日志、保留策略、在线用户（`audit.ts` + `audit/*` 自动生成）。

#### 后端有但前端缺的端点

| 端点 | 后端 handler | 优先级 | 说明 |
| --- | --- | --- | --- |
| GET `/v1/audit/export/compliance/operation/csv` | `export_operation_csv` | P1 | 合规 CSV 导出，前端无下载按钮 |
| GET `/v1/audit/export/compliance/operation/json` | `export_operation_json` | P1 | 同上 JSON |
| GET `/v1/audit/export/compliance/login/csv` | `export_login_csv` | P1 | 登录日志合规导出 |
| GET `/v1/audit/retention/stage/{created_at}` | `get_data_stage` | P2 | ✅ 已落地（2026-07-01 第二轮）：`monitor/retention/index.vue` 加 Stage 探针卡，输入 RFC3339 时间看落点；`api/audit.ts:getDataStage` |

> 网关当前 `audit.router.toml` 有 `/v1/audit/export/{rest}` 单段 `{rest}`，能否匹配 `compliance/operation/csv` 多段路径需实测确认；不匹配则需补 `{rest:.*}` 或拆多段路由。**实测确认（2026-07-01）**：`{rest}` 模板语法（`services/api-gateway/src/template.rs:106-119`）在 var 后无 literal 时抓取到 input 末尾，可吞 `compliance/operation/csv`、`compliance/operation/json`、`compliance/login/csv`、`retention/stage/<date>` 等多段路径。`scripts/check-audit-routes.js` 6/6 验证通过，router.toml 无需改动。

待补充：

- ✅ **改造** `frontend/src/views/monitor/operation-log/index.vue`（2026-07-01 第二轮）：合规导出 CSV / JSON 按钮 + `downloadRaw()`/`timestampedFilename()` helper（`frontend/src/utils/download.ts`），`AUD_LOG_EXPORT` 权限保护
- ✅ **改造** `frontend/src/views/monitor/login-log/index.vue`（2026-07-01 第二轮）：合规导出 CSV 按钮（无 JSON 端点）

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
| 4.3-fu2 role/dept 收件人解析 | TODO.md | P0 | ✓ **追溯已落地（2026-07-01）**：`ReScopeSelector` 组件已含 user/role/dept 三 tab（`frontend/src/components/ReScopeSelector/index.vue`），挂在 `views/notification/admin/form.vue` 的「接收人」项。底层展开靠 `notification-service` `iam_client.rs:47,52` 调 `GET /iam/internal/users/by-{role,dept}/{id}`。无需新前端动作 |
| 4.4-fu1 user-roles assign | TODO.md | P0 | 联通 `form/role.vue`（见 §2.1） |
| 4.4-fu2 actor_id 透传 | TODO.md | P2 | 仅后端 |
| 4.4-fu3 perm_epoch 本地缓存 | TODO.md | P2 | 仅后端 gateway |

### 4.2 平台级补全

- **错误码前端字典**：`libs/digua_common-error/src/codes.rs` 已有常量，需前端 `src/utils/error-codes.ts` 镜像 + i18n 文案；目前各页错误是裸英文/数字。
- ✅ **`v-auth` / `v-perms` 全量回归**（2026-07-01 第二轮）：新增 `scripts/check-v-perms.js` linter，扫描 198 个 .vue 文件，0 违规。本轮新增 view（file / config / system/{tenant,session,dept/assign-users,token-client/api-keys,token-client/resource-probe}）全挂 perm。历史 view `monitor/logs/*` 因 legacy `useRole` 误用跳过（独立清理）
- **菜单入口整理**：`router/modules/monitor.ts` `notification.ts` `system.ts` 完整性需校验（`monitor/` 5 页是否都在 sidebar 注册）。
- **nginx.conf `/api/v1/file` `/api/v1/config` 转发**：选 A 方案则 nginx 不动；选 B 方案需补 location。
- ✅ **AGENTS.md 文档漂移**（2026-07-01 第二轮）：澄清调用约定 —— `http.request(url)` 写 `/v1/<svc>/...`（不是 `/api/v1/...`），因为 axios `baseURL` 已含 `/api` 前缀。AGENTS.md 加示例 + 已集成服务表扩 file / config。

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

## 7. 补充缺口盘点（2026-07-01 第二轮）

基于 §1–§4 的盲点扫描，按 P0/P1/P2 复列。

### 7.1 P0 — 没就会卡

**permission matrix 落点未定** → **✓ 已落地（2026-07-01）**：`contracts/permissions.yaml` 增 8 码（`FILE_READ` + `CFG_CONFIG_REVEAL` + `CFG_NAMESPACE_{CREATE,READ,UPDATE,DELETE,EXPORT,IMPORT}`），`scripts/contracts-gen.js` 跑通，`libs/digua_authz/src/perms.rs` 已重新生成；admin 种子策略沿用现有 `ensure_admin_default_permissions`（仅 `notification.admin`），其余需 `POST /iam/permissions` 入库后通过权限管理页分配。doc §1.1/§1.2/P2 UI 按钮的 `v-perms` 引用按新增码声明。

**DTO 来源未定** → **✓ 已落地（2026-07-01）**：`scripts/openapi-codegen.js` 出 DTO 类型到 `frontend/src/api/{file,config}/{file,config}.types.ts`；运行时 `api/file.ts` / `api/config.ts` 手写 `http.request` 包装（注入 JWT header，codegen 出的 `FileClient` 类缺鉴权）。参考 `api/audit.ts` / `api/iam.ts` 既有模式。详见 §1.1 / §1.2 头部已更新的注释。

**前端测试硬性章节** → **✓ 已落地（2026-07-01）**：`frontend/package.json` 加 `test:unit`（跑全量 vitest）+ `test:unit:watch`；`frontend/AGENTS.md` 加「测试规范」段，要求每新 view ≥ 1 happy-path 单测，就近放 `<name>.test.ts`。E2E 引用既有 `scripts/run-e2e-local.js`（后端 happy-path），浏览器级 E2E 列为立项可选，不在本规范强制范围。

**file 上传边界条件**（强化 §1.1）→ **✓ 已落地（2026-07-01）**：从 `services/file-service/src/config.rs:33-34` 提取 max_file_size = 100MB，写入 `frontend/src/api/file/limits.ts`（导出 `FILE_MAX_SIZE_BYTES` / `checkFileSize` / `checkFileExtension` / `FILE_BLOCKED_EXTENSIONS`），由 `file/index.ts` 统一 re-export。`services/file-service/AGENTS.md` 端点表补齐 `/file/list` `/file/:id(GET/DELETE)` `/file/:id/info`。view 上传 view 写完后 `:before-upload` 调用 `checkFileSize` + `checkFileExtension`，超出阻止；后端不强制 MIME / 文件名清理 / 配额。

**config 敏感字段**（强化 §1.2）→ **决策（2026-07-01）**：

- 后端 `ConfigItem.is_secret: bool` 已存在（`services/config-service/src/models/config_item.rs:13`，DB 列 `cfg_config_items.is_secret TINYINT(1) DEFAULT 0`），**但** `get_config_item` / `query_config_items` / `export_config` 当前全部返回明文，未挂钩 `CFG_CONFIG_REVEAL` 权限（`§7.1.1` 新增）。
- 前端补 UI 掩码：`frontend/src/api/config/masking.ts` 提供 `maskSecretValue` / `isMaskedDisplay` / `shouldMaskInList` / `reveal` / `hide` / `toDisplayValue`，view 中调用 `toDisplayValue(id, raw, item.is_secret)` 显示，"显示明文"按钮挂 `v-perms="['CFG_CONFIG_REVEAL']"`。**仅 UX**：后端 API 仍返明文，需后续补 reveal 端点检查。
- 后端缺口（高优，跟 §7.1.5 拆开立项）：在 `get_config_item`/`query_config_items`/`export_config` 三个 handler 加 reveal-aware 路径 —— 详见 `services/config-service/AGENTS.md` 「已知缺口 — 敏感字段掩码」段。

### 7.2 P1 — 闭环残留

| 项 | 落点 | 动作 |
| --- | --- | --- |
| 错误码 i18n 落地表 | `frontend/src/contracts/error-codes.zh-CN.ts` | 镜像 `libs/digua_common-error/src/codes.rs`（数字 → 文案）；启用 `gen-ts.js` 已在 `frontend/src/contracts/`；helper `translateError()` 在 `frontend/src/utils/error-translate.ts`；`frontend/AGENTS.md` 已加使用规范 |
| 写操作事件盘点 | `contracts/events.yaml` | ✓ 已落地（2026-07-01）：补 `file.uploaded` / `file.deleted` / `config.item.changed`，gen 重生成 `libs/digua_common-event/src/{routing_keys,events}.rs`（10 事件）。`iam.user.roles_assigned` 沿用现有 `iam.permission.changed`（详见 `events.yaml` 注释 + AGENTS.md Phase 4.4 段），未重复造事件 |
| sidebar 注册校验 | `scripts/check-router-modules.js` | ✓ 已落地（2026-07-01）：新增 `scripts/check-router-modules.js` 静态校验 `router/modules/*.ts` 每文件至少一条 `path:`。Sidebar 数据由 `prue_backend/get-async-routes` 从 `prue_menu_configs` 读出，模块文件 ↔ 菜单配置项一一对应（路径前缀一致）。新视图上线流程：1) 写 view → 2) 写 `router/modules/<svc>.ts` → 3) 通过菜单配置 UI 创建并 publish 菜单项 → 4) sidebar 自动出现 |
| audit gateway 多段路径 | `audit.router.toml` | ✓ 已落地（2026-07-01）：`{rest}` **能**吞多段（`/v1/audit/export/{rest}` 吞 `compliance/operation/csv`；`/v1/audit/retention/{rest}` 吞 `stage/<date>`）。镜像 `services/api-gateway/src/template.rs:106-119` 的 TplPart::Var 抓取到 input 末尾或下个 literal 处。新增 `scripts/check-audit-routes.js` 跑测试，6/6 全过。无须改 router.toml |

### 7.3 P2 — 锦上添花

| 项 | 落点 | 动作 |
| --- | --- | --- |
| inbox 已读/未读 tab | `frontend/src/views/notification/inbox/index.vue` | ✓ 已落地（2026-07-01）：`useInbox` 加 `activeTab` + `useInboxReadState`（localStorage 持久化），`index.vue` 加 `<el-tabs>` 三 tab（全部/未读/已读），计数随列表变化；测试 `read-state.test.ts` 5/5 通过 |
| SSE 占位 | `frontend/src/api/config/sse.ts` | ✓ 已落地（2026-07-01）：新增 `watchConfig()` 占位 stub（不连 EventSource），注释里钉 `?token=` 用法 + gateway SSE feature gap 备注；gateway SSE 上线后取消注释即可 |
| WS 重连策略 | `frontend/src/utils/notification-socket.ts:17` | ✓ **追溯已落地**：`RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000]` 指数退避（封顶 30s），`onclose → scheduleReconnect()`，`retry<5` 后停止；AGENTS.md 加「WebSocket 重连」段说明策略 + 调用约定 |

### 7.4 隔壁建议（不属于对接清单）

- **user-roles redis 缓存**：§2.1 P0 联通后需确认 PUT 是否进 redis，否则频繁切角色拖慢（`redis.keys` + `digua_authz` lookup 应同步）。
- **docker 网络可达性回归**：§3「前端在服务起来后可直连」缺命令，补 `docker compose -f docker-compose-services.yml ps file-service config-service` 验证容器健康。

---

## 6. 不在本文档范围

- 后端 only 的 Phase 5/6/7/8 路线图（事件重试、可观测性等基础设施升级）见 `TODO.md`。
- 前端组件库 / 主题 / i18n 文案本身的 polish 不属于「对接缺口」。
- 旧 pure-admin 遗留 mock 端点（`system.ts` 的 `/user` `/list-all-role` 等）需后续清理，但与后端对接无关，不在本文档列项。
- 新增业务服务尚未启动，不在盘点范围。