# Frontend Menu Publish Process Guide (F3)

## Status: 🟡 待 user review

`frontend_todo.md §1.1/1.2` 显式提的缺口：「router module 文件就位但 `prue_menu_configs` 需运营/手动 publish 才在 sidebar 出现」。本文锁定「补一份操作指南」的设计、范围、内容大纲与验收。

---

## 1. 目标

让「新增 view 怎么进 sidebar」从「靠老人口口相传」变成「新人能照着文档一步一步走完」的指南。文档化新增 view → 路由注册 → 菜单 publish 的完整流程。

---

## 2. 范围

### 2.1 在范围

- 新建 `docs-site/guides/menu-publish.md`：操作指南，4-5 节
  1. 概述（为什么 sidebar 不自动出现）
  2. 完整流程图（新增 view → 路由注册 → 菜单 publish → 验证）
  3. 详细步骤（建 view / 注册 router module / 菜单 publish / 验证 sidebar）
  4. 常见问题（showLink: false / auths / etag 并发 / 路径前缀不一致 等）
  5. 相关脚本 + linter（`scripts/check-router-modules.js` 必跑、`scripts/check-vitest-coverage.js` 必跑）

### 2.2 不在范围（本文 commit）

- ❌ 不建 `.trae/skills/coder-menu-publish/` skill（YAGNI：文档指南已足；skill 在「操作步骤需要 AI 触发」时才有价值，本流程人工驱动）
- ❌ 不改 prue_backend 端点（已闭环）
- ❌ 不改 menu 配置 UI（`views/system/menu/index.vue` 已支持 CRUD + publish + clone）
- ❌ 不加 CI 门禁（prue_backend 端点已稳定）

---

## 3. 内容大纲

### 3.1 概述
- 解释 `frontend/src/router/modules/*.ts` 是**路由注册**（动态 import + meta）
- 解释 `prue_menu_configs` 是**菜单配置**（sidebar 数据源）
- 二者**一一对应**（路径前缀一致），缺一不可：
  - 只有 router module → URL 能访问，但 sidebar 不出现
  - 只有 menu config → sidebar 出现但点进去白屏
- 数据流：用户登录 → `get-async-routes` → prue_backend 读 active 菜单版本 → 返回给前端 → 前端 filter 路由 → 渲染 sidebar

### 3.2 完整流程图
```
1. 写 view:        frontend/src/views/<svc>/<feature>/index.vue
2. 写 router:      frontend/src/router/modules/<svc>.ts  (children 数组加新路由)
3. (CI 拦截)       pnpm run check:vitest-coverage.js  ← 新 view 必须有 .test.ts
4. (CI 拦截)       pnpm --filter vue-pure-admin run check:router-modules
5. 写菜单:         系统管理 → 菜单管理 → 新建草稿 → 填表单（path / name / title / icon / rank / auths）
6. 发布菜单:       选中草稿 → 点击「发布」→ 等 v(N) active 出现
7. 验证:           重新登录（JWT 需重发）→ sidebar 出现新菜单项 → 点击进 view
```

### 3.3 详细步骤

#### 步骤 1：写 view
```ts
// frontend/src/views/<svc>/<feature>/index.vue
<script setup lang="ts">
defineOptions({ name: "ServiceFeature" });
</script>
<template><div>feature</div></template>
```

name 必须与 router module children 中的 `name` 字段一致。

#### 步骤 2：注册 router module
如果新 view 在已有 service 下（如 `system/`、`notification/`、`monitor/`），**追加**到对应 module 的 children 数组：
```ts
// frontend/src/router/modules/<svc>.ts
{
  path: "/system/feature",
  name: "SystemFeature",
  component: () => import("@/views/system/feature/index.vue"),
  meta: {
    title: $t("menus.systemFeature"),
    icon: "ep/star",
    keepAlive: true,
    auths: ["IAM_PERM_xxx"]  // 可选，逗号分隔字符串
  }
}
```

如果新 view 在新 service 下（如新建 `frontend/src/views/foo/`），**新建** `frontend/src/router/modules/foo.ts`：
```ts
import { $t } from "@/plugins/i18n";
const Layout = () => import("@/layout/index.vue");
export default {
  path: "/foo",
  component: Layout,
  redirect: "/foo/index",
  name: "Foo",
  meta: { title: $t("menus.foo"), icon: "ep/something", rank: 50 },
  children: [
    // ...
  ]
} satisfies RouteConfigsTable;
```

并在 `frontend/src/router/enums.ts` 加 rank 常量、`frontend/locales/zh-CN.ts` 加 `menus.foo` 翻译。

#### 步骤 3-4：CI 拦截
- `pnpm run check:vitest-coverage.js` 必跑，新 view 缺 .test.ts → exit 1
- `pnpm --filter vue-pure-admin run check:router-modules` 必跑，router module 必含 path

#### 步骤 5-6：菜单 publish
**方式 A（推荐）：UI 操作**
1. 浏览器访问 `/system/menu`
2. 选择菜单版本下拉，点「新建草稿」
3. 填表单：
   - 名称：`<service-feature>`
   - 路径：与 router module 的 path 一致
   - 组件：与 router module 的 component 路径一致
   - 标题 / 图标 / 排序：与 router module 的 meta 一致
   - 权限码：meta.auths 数组，逗号分隔
4. 保存 → 选中新建的草稿 → 点「发布」

**方式 B：直接 curl**（CI / 自动化场景）
```bash
PRUE=http://localhost:6004
# 1. 拿现有 active 配置作为模板
ACTIVE_ID=$(curl -s $PRUE/pure/menu-configs | jq -r '.data.list[] | select(.is_active) | .id')
# 2. clone 为草稿
DRAFT_ID=$(curl -s -X POST $PRUE/pure/menu-configs/$ACTIVE_ID/clone | jq -r '.data.id')
# 3. PUT 修改 routes_json（vxe-grid 拖拽编辑后的 JSON）
curl -X PUT $PRUE/pure/menu-configs/$DRAFT_ID \
  -H "Content-Type: application/json" \
  -H "If-Match: <etag-from-step-2>" \
  -d '{"name":"v(N)","routes_json":[...]}'
# 4. publish
curl -X POST $PRUE/pure/menu-configs/$DRAFT_ID/publish
```

#### 步骤 7：验证
1. 退出登录（**必须**：JWT 内权限 + sidebar 来自 active menu config，缓存）
2. 重新登录
3. 检查 sidebar：应出现新菜单项
4. 点击 → URL 跳转 → view 渲染
5. 浏览器 DevTools Network：`/get-async-routes` 返回的 routes 数组里能找到新路由

### 3.4 常见问题

| 现象 | 原因 | 修复 |
|---|---|---|
| sidebar 不出现 | 未 publish 菜单 | 检查 `/system/menu` 是否有 active 版本 |
| sidebar 出现但点击白屏 | router module 没注册 / path 不一致 | 跑 `check:router-modules`；核对 path |
| 重新登录后仍不出现 | 用了 showLink: false | 显式 URL 访问，或改 showLink: true |
| 写操作按钮不响应 | 缺权限 | 确认 v-perms 引用的权限码在 active 菜单的 auths 里 |
| 403 跨租户 | tenant_id 不匹配 | `frontend_todo.md §8.3` 跨租户文档 |
| 并发 publish 失败 | 409 etag conflict | clone 一份重新编辑 |

### 3.5 相关脚本
- `scripts/check-router-modules.js`：校验 router module 文件含 path
- `scripts/check-vitest-coverage.js`：校验新 view 配 .test.ts
- `scripts/check-v-perms.js`：校验写按钮挂 v-perms
- `scripts/check-spec.js` / `contracts-gen.js --check`：契约一致

---

## 4. 验收

- [ ] `docs-site/guides/menu-publish.md` 存在 + 4-5 节齐全
- [ ] 步骤示例代码可复制粘贴
- [ ] curl 流程示例与 prue_backend 6 端点对齐
- [ ] FAQ 覆盖 §3.4 列的 6 类常见问题
- [ ] 引用 `frontend_todo.md §1.1/1.2`（闭环）
- [ ] 1 commit（仅含 1 个新 .md）

---

_最后更新：2026-07-08_
