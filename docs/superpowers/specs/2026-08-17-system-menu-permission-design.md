# system 模块菜单权限（粗粒度单码 IAM_SYSTEM_READ）设计

- 日期：2026-08-17
- 范围：`services/iam-service`（V006 migration + db.rs 注册）+ `frontend/src/router/modules/system.ts`
- 状态：design / 待实施

## 1. 背景与现状

前端「系统管理」(`/system/*`) 菜单当前对所有登录用户可见，且任何用户都能直接访问 `/system/user`、`/system/role`、`/system/permission`、`/system/menu`、`/system/tenant`、`/system/dept` 6 个子页面（只有 hidden 状态、不进侧边栏的 `api-keys` / `session` / `resource-probe` 通过 `meta.auths` 做了按钮级控制，但**侧边栏的 system 父菜单完全裸奔**）。

**根因**（双层防线缺失）：

1. `frontend/src/router/modules/system.ts` 父路由 + 6 个展示型子路由的 `meta` **均无 `roles` 字段**。
2. `frontend/src/router/utils.ts:filterNoPermissionTree` 内的 `isOneOfArray(undefined, currentRoles)` 在 `a` 为 undefined 时**直接返回 `true`**（见 utils.ts:78-84 的 `Array.isArray(a) && Array.isArray(b) ? ... : true`），所以**没配 `meta.roles` 的路由对所有用户都显示**。

侧边栏渲染链路（已存在）：

```
constantMenus (router/index.ts:68-70, 全量)
  ↓ usePermissionStore.handleWholeMenus()
filterTree(ascending(constantMenus))            // utils.ts:58, 按 showLink 过滤
  ↓ filterNoPermissionTree(...)                  // utils.ts:87, 按 meta.roles 过滤
  ↓ filterChildrenTree(...)                      // utils.ts:69, 空 children 父目录过滤
usePermissionStore.wholeMenus  ←  侧边栏实际渲染这个
```

路由跳转守卫（已存在）：

```ts
// frontend/src/router/index.ts:154
if (to.meta?.roles && !isOneOfArray(to.meta?.roles, userInfo?.roles)) {
  return { path: "/error/403" };
}
```

**结论**：framework 层面**侧边栏 + 路由跳转双层过滤都已就位**，只是 system 模块没配 `meta.roles`，所以拦截规则没被触发。

后端 iam 现状（`services/iam-service/`）：

- `iam_permissions` 表 V001 已建表，**但 0 行 seed 数据**（V005 是一段 `INSERT IGNORE ... FROM iam_permissions WHERE status = 'enabled'` 的写法，依赖 iam_permissions 里有 enabled 行——V005 写时还没有任何 enabled 行，所以它实际上没绑到任何权限）。
- V005 seed 了 `platform_admin` 角色（`code='platform_admin'`，`id='role-platform-admin-seed'`，`tenant_id=1`），并把所有已 enabled 的权限绑给它 + 把 `admin` 用户（`tenant_id=1, username='admin'`）绑到 `platform_admin` 角色。**所以 admin 用户登录后 `user.roles = ['platform_admin']`**。
- `auth_svc.rs:330 get_user_permissions` 是真实 SQL join 出来的 `Vec<String>`（权限 code），登录响应 `LoginResponse.user.permissions` 携带真实权限码；前端 `user.ts:103-106` 的 `["*:*:*"]` 兜底只覆盖 `permissions` 字段，不影响 `roles` 字段比对。

## 2. 目标

- 「系统管理」侧边栏菜单 + 6 个展示型子页面（user/role/permission/menu/tenant/dept）**仅 `platform_admin` 角色可见可访问**。
- 其他角色：侧边栏看不到 system 父菜单，直接输入 `/system/*` 跳 `/error/403`。
- 引入一个 category=menu 的权限码 `IAM_SYSTEM_READ`，作为 system 模块统一的粗粒度权限标识；**当前由 `roles: ['platform_admin']` 字段在 framework 已有逻辑上立刻生效**，`IAM_SYSTEM_READ` 这个码**保留**作为未来按钮级 `v-perms` 控制或后端接口守卫的统一锚点（见 §6 后续）。

## 3. 设计决策

### 3.1 为什么用 `meta.roles` 而非 `meta.auths`

用户原始诉求"用同一个 auths 码（粗）"指的是"用一个权限 code 字符串统一控制"，并非 framework 字段名。framework 现状：

- `filterNoPermissionTree` 与路由守卫（`router/index.ts:154`）**只读 `meta.roles`**，不动 framework 即可零成本实现 100% 效果。
- `meta.auths` 是给 `v-auth` 指令 / `<Auth>` 组件 / `hasAuth()` 用的，**作用域是当前路由的按钮级**，不是菜单级。
- 若坚持改 `filterNoPermissionTree` 让它也读 `meta.auths`，会触动所有路由的过滤链路 + 路由守卫的双处改动 + 配套单测，**风险与改动范围过大**，超出本任务范围。

**决定**：用 `roles: ['platform_admin']` 做菜单 + 路由过滤；`IAM_SYSTEM_READ` 权限码独立 seed（绑给 `platform_admin` 角色），作为未来按钮级 / 接口守卫的统一锚点。具体落地由后续任务消费这个码。

### 3.2 为什么是 `platform_admin` 角色而不是新建 `admin` / `system_admin` 角色

- `platform_admin` 角色已由 V005 seed，且是 admin 用户的实际角色（`auth_svc.rs:82` `cross_tenant` 也基于此判断）。
- 新建一个 system 专用角色 + 改 admin 用户绑定关系 = 涉及 iam role assignment 端点 + 数据迁移 + 已部署环境的兼容回退，**超出本任务范围**。
- 当前 6 个子路由的粗粒度权限完全等同 platform_admin 的现有 scope（"系统级、跨租户"），不引入新粒度。

### 3.3 IAM_SYSTEM_READ 权限码定义

```text
code:        IAM_SYSTEM_READ
name:        系统管理菜单
description: 系统管理（用户/角色/权限/菜单/租户/部门）菜单入口
category:    menu
resource:    system
action:      read
sort_order:  14   ← 与 router/enums.ts 的 system=14 对齐
status:      enabled
```

id 用确定性字符串 `perm-system-read` 便于 V006 的 `INSERT IGNORE` 复用。

### 3.4 不在本次改动范围内

- 不修改 `filterNoPermissionTree` 或路由守卫让其支持 `meta.auths`（见 §3.1）。
- 不修改 `user.ts:103-106` 的 `permissions: ["*:*:*"]` 兜底（不影响 system 过滤，且是另一个独立质量修复）。
- 不做按钮级 `v-perms` 集成（保留 `IAM_SYSTEM_READ` 供后续消费）。
- 不做后端 `/v1/iam/*` 端点的 `IAM_SYSTEM_READ` 网关 / 中间件层校验（当前 iam 各路由的 `Principal` 校验 + 数据权限已足够）。

## 4. 改动清单

### 4.1 新增文件 `services/iam-service/migrations/mysql/V007__system_menu_permission.sql`

```sql
-- Phase 8.4: 系统管理菜单权限码（粗粒度单码）
-- 与 router/modules/system.ts 配合：frontend 路由 meta.roles=['platform_admin'] 走
-- filterNoPermissionTree 过滤菜单 + 路由守卫 403；本权限码 IAM_SYSTEM_READ
-- 作为未来 v-perms 按钮级 / 后端网关层校验的锚点，本任务不直接消费。

INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
VALUES (
  'perm-system-read',
  'IAM_SYSTEM_READ',
  '系统管理菜单',
  '系统管理（用户/角色/权限/菜单/租户/部门）菜单入口',
  'menu',
  'system',
  'read',
  14,
  'enabled',
  NOW(), NOW()
)
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  description = VALUES(description),
  sort_order = VALUES(sort_order),
  updated_at = NOW();

-- 把新权限码绑给 V005 已 seed 的 platform_admin 角色
-- V005 的 INSERT IGNORE ... FROM iam_permissions WHERE status = 'enabled' 是
-- V005 一次性动作，不会重新跑；V006 必须显式补绑定
INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT 'role-platform-admin-seed', 'perm-system-read', NOW();
```

### 4.2 修改 `services/iam-service/src/db.rs`

注册 V006（org_unit_path）+ V007（system_menu_permission） + marker table：

```rust
const V006_SQL: &str = include_str!("../migrations/mysql/V006__org_unit_path.sql");
const V007_SQL: &str = include_str!("../migrations/mysql/V007__system_menu_permission.sql");
const V006_MARKER_TABLE: &str = "iam_migration_v006_applied";
const V007_MARKER_TABLE: &str = "iam_migration_v007_applied";
```

在 `ensure_schema` 函数内追加 V006 + V007 两块。V006 是 pre-existing 别人写的、原本 db.rs 没引用——本任务**顺手把它也接入**，避免后续 db.rs 改动时再补充。

### 4.3 修改 `frontend/src/router/modules/system.ts`

6 个展示型子路由（user / role / permission / menu / tenant / dept）统一加 `roles: ['platform_admin']`：

- `/system/user`（SystemUser）
- `/system/role`（SystemRole）
- `/system/permission`（SystemPermission）
- `/system/menu`（SystemMenu）
- `/system/tenant`（SystemTenant）
- `/system/dept`（SystemDept）

**父路由 `/system` 不加 `roles`**：受 `frontend/types/router.d.ts:RouteConfigsTable` 类型约束（父路由 meta 仅含 title/icon/showLink/rank），且 framework 的 `filterChildrenTree` 已能联动：子路由全被过滤时父菜单自动消失，platform_admin 命中任一子路由时父菜单自动保留。所以父路由无需显式声明。

hidden 路由（`api-keys` / `session` / `resource-probe`）**不动**——它们 `showLink: false` 不进侧边栏菜单，访问它们的入口在 system/* 父页面内，命中父页 403 后已无法到达。

#### 4.3.1 边界：中间路由 `SystemTokenClient` 不动

`/system/token-client`（SystemTokenClient，line 51-58）是个**中间路由**：自带 `icon: "ep/key"` + component + 3 个 hidden children（api-keys / resource-probe / 其它 hidden）。按本次任务边界**不加 `roles: ['platform_admin']`**，但**实际行为**：

- 非 platform_admin 角色：`filterTree` 干掉 3 个 hidden children → `filterChildrenTree` 因为 children 长度为 0 → **token-client 父菜单项也被过滤掉**。
- platform_admin 角色：children 仍被 `filterTree` 过滤（hidden 本就不进菜单） → 父菜单项**同样**被 filterChildrenTree 过滤掉 → 侧边栏看不到 token-client 入口 → 3 个 hidden 子页面**暂时**失去访问入口。
- 路由跳转直接输入 `/system/token-client`：因 meta 无 `roles` 字段，路由守卫**不拦截**；非 platform_admin 角色可直达页面。

**结论**：本任务范围内 token-client 行为发生隐式变化（platform_admin 失去侧边栏入口），但不影响本次"系统管理菜单全员可见"的核心诉求。token-client 的菜单治理另起任务 plan-eng-review。

### 4.4 新增单测 `frontend/src/router/utils.test.ts`

覆盖三个点：

1. **system.ts 路由结构断言**：6 个展示型子路由（user / role / permission / menu / tenant / dept）的 `meta.roles` 都为 `['platform_admin']`。
2. **filterNoPermissionTree 端到端规约**（复制 framework 逻辑到单测里，不 import framework，避免触发 @/router/index 链上 @/utils/sso 顶层 IIFE 副作用）：
   - `user.roles = ['platform_admin']` → system 父 + 6 个子全部保留
   - `user.roles = ['common']` → system 父菜单被 filterChildrenTree 联动过滤；其它无 meta.roles 的菜单（/dashboard）保留
   - `user.roles = []` → 同上
   - 未登录（storageLocal 为空）→ 同上
3. `isOneOfArray` 行为规约（pure function 复制）：
   - `isOneOfArray(['platform_admin'], ['platform_admin'])` → true
   - `isOneOfArray(['platform_admin'], ['common'])` → false
   - `isOneOfArray(undefined, ['platform_admin'])` → true（**这是 system 现状的根因**——保留这个测试反向记录）

### 4.5 同步更新 `services/iam-service/src/routes/internal_migration_state.rs`

`IamMigrationState.migrations()` 列表加 V006 + V007；`iam_has_five_migrations` 改名 `iam_has_seven_migrations` 并更新断言：

```rust
&[
    MigrationDescriptor { version: "V001", name: "init", marker_table: "iam_users" },
    MigrationDescriptor { version: "V002", name: "outbox_archive", marker_table: "iam_outbox_events_archive" },
    MigrationDescriptor { version: "V003", name: "tenants", marker_table: "iam_tenants" },
    MigrationDescriptor { version: "V004", name: "tenant_business_tables", marker_table: "iam_migration_v004_applied" },
    MigrationDescriptor { version: "V005", name: "platform_admin_role", marker_table: "iam_migration_v005_applied" },
    MigrationDescriptor { version: "V006", name: "org_unit_path", marker_table: "iam_migration_v006_applied" },
    MigrationDescriptor { version: "V007", name: "system_menu_permission", marker_table: "iam_migration_v007_applied" },
]
```

### 4.6 同步更新 `services/iam-service/tests/v007_system_menu_permission.rs`（可选）

如果 iam-service 已有 testcontainers 集成测试框架，加 1 条 happy-path：

- 启动 iam-service → `iam_permissions` 包含 `IAM_SYSTEM_READ` (category=menu, status=enabled)
- `platform_admin` 角色的 `iam_role_permissions` 包含 `IAM_SYSTEM_READ` 对应 id
- 登录 `admin` / `admin`（默认种子）→ `user.roles` 包含 `platform_admin` + `user.permissions` 包含 `IAM_SYSTEM_READ`

## 5. 验证步骤（门禁）

按 `AGENTS.md` "push 前必过"清单：

```bash
# 1. 后端编译 + 单元测试（V006 新文件会被 db.rs 引用）
node scripts/run-tests-docker.js iam-service

# 2. 前端类型检查 + 单测
cd frontend && pnpm run typecheck
cd frontend && pnpm run test:unit

# 3. CI 5 门禁
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/check-db-naming.js
node scripts/dump-openapi-specs.js   # 改了 V006，但 iam 路由未变；openapi 应零变更
git diff --exit-code -- 'services/*/openapi.yaml'
```

预期：全部通过。Cargo.lock 若有非预期变更，需查 `cargo update` 是否被触发。

## 6. 后续（不在本任务范围）

1. `IAM_SYSTEM_READ` 接入 `v-perms`：在 system 6 个子页面内**关键写操作按钮**（如 `/system/user` 的"新建用户"）加 `v-perms="['IAM_SYSTEM_READ']"`，做按钮级二次保护。当前 `v-perms` 走的是当前路由的 `meta.auths`，要把 `meta.auths: ['IAM_SYSTEM_READ']` 也加到 6 个子路由——但这又会和 framework 的"auths 不进菜单过滤"产生混淆，需另起任务设计。
2. `IAM_SYSTEM_READ` 接入 api-gateway：把 `/v1/iam/users*` / `/v1/iam/roles*` / `/v1/iam/permissions*` / `/v1/pure/menu-configs*` / `/v1/iam/tenants*` / `/v1/iam/org-units*` 路由的 public=false，并在中间件层校验 `Principal.permissions` 包含 `IAM_SYSTEM_READ`。当前 iam 端点已有 `Principal` 提取，**已隐式提供保护**（无 bearer = 401），本次不显式加重。
3. 角色扩展：若未来需要"只管 user 不管 dept"等更细粒度，把 `IAM_SYSTEM_READ` 拆为 `IAM_USER_READ` / `IAM_ROLE_READ` 等 + 6 个子路由分别挂码。需要时再走 plan-eng-review。

## 7. 风险与回滚

- **回滚成本低**：V006 是纯 INSERT（idempotent），`git revert` 即可；frontend `roles` 字段是单侧加字段，不影响其它路由。
- **风险点**：`platform_admin` 是 admin 用户的唯一角色，**如果未来引入新的非 platform_admin 角色**（如只读 viewer）又希望其能看 system 菜单，需要在 §6.3 的细粒度拆分后单独给该角色加 `IAM_SYSTEM_READ` 权限。
- **不影响**：`user.ts:103-106` 的 `permissions: ["*:*:*"]` 兜底与本任务无关；现有 hidden 路由（api-keys / session / resource-probe）的 `auths` 字段不动；其它模块的 system.ts 改动零侵入。
