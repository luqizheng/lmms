# 菜单 Publish 操作指南

> 适用：新增前端 view 后如何让它在 sidebar 出现。
> 关联：`frontend_todo.md §1.1/1.2`「router module 文件就位但 `prue_menu_configs` 需运营/手动 publish 才在 sidebar 出现」。
> 维护：frontend AGENTS.md 改动 / prue_backend 端点变化时同步更新。

---

## 1. 为什么需要这步

平台前端有两个独立系统：

| 系统 | 文件 | 作用 |
|---|---|---|
| **路由注册** | `frontend/src/router/modules/<svc>.ts` | 动态 import + 路径 + meta + 权限码 |
| **菜单配置** | `prue_backend` 的 `prue_menu_configs` 表 | sidebar 数据源 |

二者**一一对应**（路径前缀一致），缺一不可：

- ❌ 只有 router module → URL 能访问但 sidebar 不出现
- ❌ 只有 menu config → sidebar 出现但点进去白屏
- ✅ **两者都到位** → sidebar 出现 + 点击正常进 view

数据流：

```
用户登录 → JWT 带 role/perm
       → 前端 pinia store 调 /get-async-routes
       → prue_backend 读 active 菜单版本（is_active=true）
       → 返回 sanitized 路由树
       → 前端 filter + 渲染 sidebar
```

---

## 2. 完整流程

```
1. 写 view         frontend/src/views/<svc>/<feature>/index.vue
                  （name 必须与 router module children.name 一致）
2. 写 router       frontend/src/router/modules/<svc>.ts
                  （追加到 children，或新建 module 文件）
3. CI 拦截 1       pnpm run check:vitest-coverage.js
                  （新 view 必须配 .test.ts）
4. CI 拦截 2       node scripts/check-router-modules.js
                  （router module 必含 path）
5. 写菜单          系统管理 → 菜单管理 → 新建草稿 → 填表单
                  （或 curl 调 prue_backend API）
6. 发布菜单        选中草稿 → 点击「发布」→ 等 v(N) 出现 active 标记
7. 验证            退出登录 → 重新登录 → sidebar 出现新菜单项 → 点击进 view
```

---

## 3. 详细步骤

### 3.1 写 view

`frontend/src/views/<svc>/<feature>/index.vue`：

```vue
<script setup lang="ts">
import { ref } from "vue";

defineOptions({
  name: "ServiceFeature"   // 必须与 router module 的 children.name 一致
});
</script>

<template>
  <div class="main">
    <h1>Feature</h1>
  </div>
</template>
```

注意：

- `defineOptions({ name })` 必填，pure-admin 用 `name` 作为 keep-alive / 标签页 key
- 写操作按钮（新增/删除/导出/分配）按 `frontend/AGENTS.md §测试规范` 加 `v-perms` / `v-if="hasPerms(...)"`

### 3.2 注册 router module

#### 3.2.1 已有 service（如 system / notification / monitor / file / config / account-settings）

**追加**到对应 module 的 `children` 数组。

`frontend/src/router/modules/system.ts`（已有 9 个 children，新增第 10 个）：

```ts
{
  path: "/system/feature",
  name: "SystemFeature",
  component: () => import("@/views/system/feature/index.vue"),
  meta: {
    title: $t("menus.systemFeature"),
    icon: "ep/star",
    keepAlive: true,
    auths: ["IAM_PERM_READ"]   // 可选，按钮级别权限码
  }
}
```

#### 3.2.2 新建 service（如新建 `foo/`）

**新建** `frontend/src/router/modules/foo.ts`：

```ts
import { $t } from "@/plugins/i18n";
import { foo } from "@/router/enums";

const Layout = () => import("@/layout/index.vue");

export default {
  path: "/foo",
  component: Layout,
  redirect: "/foo/index",
  name: "Foo",
  meta: {
    title: $t("menus.foo"),
    icon: "ep/something",
    rank: foo
  },
  children: [
    {
      path: "/foo/index",
      name: "FooIndex",
      component: () => import("@/views/foo/index.vue"),
      meta: {
        title: $t("menus.fooIndex"),
        icon: "ep/menu",
        keepAlive: true
      }
    }
  ]
} satisfies RouteConfigsTable;
```

**并补**：

- `frontend/src/router/enums.ts` 加 `foo` 常量（rank 唯一递增）
- `frontend/locales/zh-CN.ts` 加 `menus.foo` + `menus.fooIndex` 翻译
- `frontend/locales/en.yaml` 同样

#### 3.2.3 菜单级权限：仅特定角色可见

> 适用：希望某条路由（顶级菜单或子页面）**只对特定角色**显示，其它角色侧边栏看不到、强进 URL 跳 `/error/403`。
> 与 §3.2 的按钮级 `auths`（`meta.auths` / `v-perms`）**正交**——可同时配。
> 关联：`docs/superpowers/specs/2026-08-17-system-menu-permission-design.md` 完整设计 spec。

| 维度 | 菜单级（`meta.roles`）| 按钮级（`meta.auths` / `v-perms`）|
|---|---|---|
| 控制目标 | 整条路由是否可见 / 可访问 | 单个按钮 / 区域是否渲染 |
| 字段值 | `iam_roles.code` 角色 code 列表 | `iam_permissions.code` 权限码列表 |
| framework 触发点 | `filterNoPermissionTree`（菜单）+ `router/index.ts:154` 路由守卫 | `v-perms` 指令 / `<Auth>` 组件 |
| iam 表 `category` | （**不**直接 seed，靠目标角色自带的权限）| `api` / `button` / `menu` |
| 默认行为（缺字段时）| 全员可见（framework 历史行为，详见 §4 故障表）| 全员不可见 |

**framework 现状**：菜单过滤和路由守卫都已就位，只需给 router module 配 `meta.roles` + 后端 seed 权限码绑给目标角色。

##### 步骤 1：router module 加 `meta.roles`

`frontend/src/router/modules/<svc>.ts`：

```ts
{
  path: "/system/menu",
  name: "SystemMenu",
  component: () => import("@/views/system/menu/index.vue"),
  meta: {
    title: $t("menus.systemMenu"),
    icon: "ep/menu",
    keepAlive: true,
    roles: ["platform_admin"]   // 仅 platform_admin 角色可见
  }
}
```

`meta.roles` 是 `Array<string>`，值是 `iam_roles.code` 字段（**不是** `iam_permissions.code`）。

framework 行为（已实现，无需扩展）：

- `frontend/src/router/utils.ts:filterNoPermissionTree`（line 87）按 `meta.roles` 比对当前用户的 `user.roles`：
  - 非命中 → 菜单项从 `wholeMenus` 过滤掉
  - 父路由 children 全部被过滤 → `filterChildrenTree` 联动过滤父目录
- `frontend/src/router/index.ts:154` 路由守卫：直接 URL 跳转 /system/* 时，命中失败跳 `/error/403`

**父路由的 `meta.roles`**：受 `frontend/types/router.d.ts:RouteConfigsTable`（line 82-104）类型限制（父路由 meta 仅含 `title`/`icon`/`showLink`/`rank`），**不需要**显式加——子路由加 `roles` 即可由 `filterChildrenTree` 联动。

##### 步骤 2：iam seed 权限码 + 绑给目标角色

新建 `services/iam-service/migrations/mysql/V00N__<feature>_menu_permission.sql`（V 号选未被占用的下一个，**先** `ls services/iam-service/migrations/mysql/` 确认）：

```sql
-- 菜单级权限码 seed
-- id 用确定性字符串便于 ON DUPLICATE KEY UPDATE
INSERT INTO iam_permissions (id, code, name, description, category, resource, action, sort_order, status, created_at, updated_at)
VALUES (
  'perm-<feature>-read',
  'IAM_<FEATURE>_READ',
  '<feature> 菜单',
  '<feature>（xxx/yyy/zzz）菜单入口',
  'menu',                       -- 关键：category=menu（与 button/api 区分）
  '<feature>',
  'read',
  50,
  'enabled',
  NOW(), NOW()
)
ON DUPLICATE KEY UPDATE
  name = VALUES(name),
  description = VALUES(description),
  sort_order = VALUES(sort_order),
  updated_at = NOW();

-- 显式绑给目标角色
-- V005 的 INSERT IGNORE ... FROM iam_permissions WHERE status = 'enabled'
-- 是 V005 一次性动作，V00X 新加的权限码不会被自动绑——必须显式写
INSERT IGNORE INTO iam_role_permissions (role_id, permission_id, created_at)
SELECT '<role-id>', 'perm-<feature>-read', NOW();
```

**同时**改 2 处 iam 启动时 schema 注册（forward-only，V005 / V006 是先例）：

1. `services/iam-service/src/db.rs` —— 加 `V00N_SQL` 常量 + `V00N_MARKER_TABLE` 常量 + 在 `ensure_schema` 末尾追加 apply 块
2. `services/iam-service/src/routes/internal_migration_state.rs` —— `IamMigrationState.migrations()` 数组加新 descriptor；如有 `iam_has_*_migrations` 测试更新断言（个数 + 末位 version / marker_table）

##### 步骤 3：验证

```bash
# 1. 后端编译 + 单测
node scripts/run-tests-docker.js iam-service
# 期望：14/14 单测通过（含 iam_has_*_migrations）

# 2. 真 MySQL apply（验证 SQL 语法 + role binding）
docker exec -i infrastructure-mysql-1 mysql -uroot -prootpass iam_smoke \
  < services/iam-service/migrations/mysql/V00N__*.sql
docker exec infrastructure-mysql-1 mysql -uroot -prootpass iam_smoke \
  -e "SELECT p.code FROM iam_role_permissions rp JOIN iam_permissions p ON rp.permission_id=p.id WHERE rp.role_id='<role-id>';"
# 期望：看到 IAM_<FEATURE>_READ

# 3. 前端单测（路由结构 + framework 行为规约）
cd frontend && pnpm exec vitest run src/router/utils.test.ts
# 期望：6 个子路由 meta.roles 断言通过 + filterNoPermissionTree 规约通过

# 4. 5 个 CI 门禁
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/check-db-naming.js
node scripts/dump-openapi-specs.js
git diff --exit-code -- 'services/*/openapi.yaml'

# 5. 端到端
# - admin 用户（绑 platform_admin）登录 → 侧边栏看到目标菜单
# - 新建一个非 platform_admin 角色 + 用户 → 侧边栏看不到目标菜单
# - 直接 URL 访问 /<svc>/<feature> → 跳 /error/403
```

### 3.3 CI 拦截

新 view 上线前**必跑**：

```bash
# 1. 视图单测覆盖 linter（de2132a）
node scripts/check-vitest-coverage.js main
# 或
pnpm --filter vue-pure-admin run check:vitest-coverage

# 2. router module 结构 linter
node scripts/check-router-modules.js

# 3. 写按钮权限 linter（已有 view 改写按钮时）
node scripts/check-v-perms.js
```

任一 exit 1 → 必须修复再 commit。

### 3.4 写菜单

**方式 A：UI（推荐）**

1. 浏览器访问 `/system/menu`
2. 顶部「菜单版本」下拉默认显示 active 版本
3. 点「新建草稿」→ 弹窗填表单：
   - **名称**：`v(N)`（如 v3，递增）
   - **菜单标题**（i18n key）：`menus.systemFeature`（与 router module 的 `meta.title` 一致）
   - **路由 path**：`/system/feature`（与 router module 的 `path` 一致）
   - **组件路径**：`system/feature/index`（pure-admin 内部约定）
   - **图标**：`ep/star`（与 meta.icon 一致）
   - **排序**：`50`（与 router/enums.ts 的常量一致）
   - **权限码**：`IAM_PERM_READ`（逗号分隔；与 meta.auths 一致）
   - **菜单类型**：0 菜单 / 1 iframe / 2 外链 / 3 按钮
4. 点「保存」

**方式 B：直接 curl**（CI / 自动化 / 调试）

prue_backend 端点表（来自 `services/prue_backend/AGENTS.md`）：

| 端点 | 方法 | 作用 |
|---|---|---|
| `/get-async-routes` | GET | sidebar 加载入口（前端用） |
| `/pure/menu-configs` | GET | 列表（含 active 标记） |
| `/pure/menu-configs` | POST | 新建草稿 |
| `/pure/menu-configs/{id}` | GET | 详情 |
| `/pure/menu-configs/{id}` | PUT | 更新草稿（需 `If-Match: <etag>`） |
| `/pure/menu-configs/{id}/publish` | POST | 发布为 active |
| `/pure/menu-configs/{id}/clone` | POST | 克隆历史版本为新草稿 |
| `/pure/menu-configs/{id}` | DELETE | 删除未发布草稿 |

```bash
PRUE=http://localhost:8087  # prue_backend compose host 端口（容器内也是 8087）

# 1. 拿当前 active 配置 ID
ACTIVE_ID=$(curl -s $PRUE/pure/menu-configs | jq -r '.data.list[] | select(.is_active) | .id')
echo "active id: $ACTIVE_ID"

# 2. clone 一份作为新草稿
DRAFT_ID=$(curl -s -X POST $PRUE/pure/menu-configs/$ACTIVE_ID/clone | jq -r '.data.id')
echo "draft id: $DRAFT_ID"

# 3. 拿 draft 的 etag（PUT 必带）
ETAG=$(curl -s $PRUE/pure/menu-configs/$DRAFT_ID | jq -r '.data.etag')
echo "etag: $ETAG"

# 4. PUT 修改 routes_json（在 active 基础上追加新路由）
ROUTES_JSON=$(curl -s $PRUE/pure/menu-configs/$DRAFT_ID | jq -r '.data.routes_json')
NEW_ROUTES_JSON=$(echo "$ROUTES_JSON" | jq '
  .[0].children += [{
    "path": "/system/feature",
    "name": "SystemFeature",
    "component": "system/feature/index",
    "meta": {
      "title": "menus.systemFeature",
      "icon": "ep/star",
      "rank": 50,
      "auths": ["IAM_PERM_READ"],
      "showLink": true,
      "showParent": false
    }
  }]
')

curl -X PUT $PRUE/pure/menu-configs/$DRAFT_ID \
  -H "Content-Type: application/json" \
  -H "If-Match: $ETAG" \
  -d "$(jq -n --arg rj "$NEW_ROUTES_JSON" '{name:"v3", routes_json:$rj}')"

# 5. publish
curl -X POST $PRUE/pure/menu-configs/$DRAFT_ID/publish
```

### 3.5 发布菜单

UI：草稿列表选中刚保存的 v(N) → 点「发布」按钮 → 状态变 `published` + `is_active=true`，旧 active 自动失效。

curl：步骤 5 的最后一步即 publish。

### 3.6 验证

1. **退出登录**（必须）：JWT 携带的角色 + 权限从服务端来，sidebar 数据也是服务端返回的 active 菜单；缓存贯穿整个 session
2. 重新登录
3. 左侧 sidebar 应出现新菜单项
4. 点击 → URL 跳到 `/system/feature` → view 渲染
5. 浏览器 DevTools → Network → 找 `get-async-routes` 请求 → 响应 body 的 `data.routes` 数组里能找到新路由（path、name、component 三个字段都正确）

---

## 4. 常见问题

| 现象 | 原因 | 修复 |
|---|---|---|
| **system / notification 等管理菜单对非管理员也全员显示** | router module 没配 `meta.roles`；framework `isOneOfArray(undefined, currentRoles)` 缺省返回 true（历史行为）| 配 `meta.roles: ['<role-code>']` + iam seed 权限码 + 绑给目标角色（§3.2.3）|
| sidebar 不出现 | 未 publish / publish 后没重新登录 | 检查 `/system/menu` 列表的 `is_active` 标记；强制退出重登 |
| sidebar 出现但点击白屏 | router module 没注册 / path 不一致 | 跑 `check:router-modules`；核对 router module children 的 `path` |
| 重新登录后仍不出现 | 该菜单项 `showLink: false` | 显式 URL 访问（深链），或改 `showLink: true`（适合作为顶级菜单）；保留 `showLink: false`（适合只通过父菜单跳转） |
| 写操作按钮 403 / 不响应 | 缺权限码 / 权限码拼错 | 确认按钮 `v-perms="['CODE']"` 中的 CODE 在 iam-service 已分配；`translateError('IAM_PERM_3001')` = 权限不足 |
| 跨租户 403 | `tenant_id` 不匹配 | 见 `docs-site/ai-context.md §8 Phase 8.3` + `scripts/check-tenant-scope.js` |
| publish 时 409 etag conflict | 别人也在改 | 重新 GET 拿最新 etag，重做 PUT |
| 跨 service 父菜单冲突 | 两个 module 文件顶层 path 重名 | 检查 `check:router-modules` 输出 |
| 切换租户后菜单不刷新 | JWT 缓存 | 退出重登，新 JWT 触发新的 `get-async-routes` |

---

## 5. 相关脚本与门禁

| 脚本 | 作用 | 时机 |
|---|---|---|
| `scripts/check-vitest-coverage.js` | 新 view 必带 .test.ts | 每次改 `frontend/src/views/**` |
| `scripts/check-router-modules.js` | router module 必含 path | 每次改 `frontend/src/router/modules/**` |
| `scripts/check-v-perms.js` | 写按钮必挂 v-perms | 每次改 `frontend/src/views/**` 写按钮 |
| `scripts/check-spec.js` | OpenAPI / 错误码 / 权限码一致 | CI 必跑 |
| `scripts/contracts-gen.js --check` | 契约生成代码与 YAML 一致 | CI 必跑 |
| `scripts/check-db-naming.js` | 表名带服务前缀 | 每次改 `services/*/migrations/**` |

本地开发流程：

```bash
# 改 view / router 之后
node scripts/check-vitest-coverage.js main
node scripts/check-router-modules.js
node scripts/check-v-perms.js

# 全套
cd frontend && pnpm run lint
```

---

_最后更新：2026-08-17（新增 §3.2.3 菜单级权限配置 + §4 "管理菜单全员显示" 故障条目）_
