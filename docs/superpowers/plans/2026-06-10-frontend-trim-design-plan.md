# Frontend Trim Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `frontend/` 从 vue-pure-admin v7.0.0 完整版裁剪为"登录 / 工作台 / 个人中心 / 用户 / 角色 / 权限 + admin 布局壳"极简骨架，所有业务页接真后端。

**Architecture:** 保留 vue-pure-admin 自身的 layout / store / directives / Re* 通用组件；删除所有 demo 路由与视图、mock 数据、冗余依赖；按 spec 6 节建立 `src/api/{auth,user,role,permission,token}.ts` 真实 API 封装；`v-perms` / `v-auth` 适配后端 JWT Principal.Roles。

**Tech Stack:** Vue 3.5 + Vite 8 + TypeScript 6 + Pinia 3 + Element Plus 2.14 + vue-router 5 + vue-i18n 11 + axios 1.14 + echarts 6。

---

## File Structure

### Create
- `frontend/src/api/auth.ts`
- `frontend/src/api/user.ts`
- `frontend/src/api/role.ts`
- `frontend/src/api/permission.ts`
- `frontend/src/api/token.ts`
- `frontend/src/router/modules/account.ts`

### Modify
- `frontend/package.json`
- `frontend/vite.config.ts`
- `frontend/src/main.ts`
- `frontend/.env.development`
- `frontend/.env.production`
- `frontend/src/router/index.ts`
- `frontend/src/router/enums.ts`
- `frontend/src/router/modules/home.ts`
- `frontend/src/router/modules/system.ts`
- `frontend/src/router/modules/error.ts`
- `frontend/src/store/modules/user.ts`
- `frontend/src/store/modules/permission.ts`
- `frontend/src/views/login/index.vue`
- `frontend/src/views/welcome/index.vue`
- `frontend/src/views/system/user/index.vue`
- `frontend/src/views/system/role/index.vue`
- `frontend/src/views/system/permission/index.vue`
- `frontend/src/views/account-settings/index.vue`
- `frontend/src/views/account-settings/components/Profile.vue`
- `frontend/src/views/account-settings/components/Preferences.vue`
- `frontend/src/views/account-settings/components/SecurityLog.vue`
- `frontend/locales/zh-CN.yaml`
- `frontend/locales/en.yaml`

### Delete
- `frontend/mock/`（整目录）
- `frontend/public/audio/`
- `frontend/public/wasm/`
- `frontend/public/html/button.html`
- `frontend/src/router/modules/{able,about,board,chatai,codemirror,components,editor,flowchart,form,formdesign,ganttastic,guide,list,markdown,menuoverflow,mind,nested,ppt,result,table,vueflow}.ts`
- `frontend/src/views/{able,about,chatai,codemirror,components,editor,flow-chart,ganttastic,guide,list,markdown,menuoverflow,mind,monitor,nested,permission,ppt,result,schema-form,table,tabs,vue-flow,empty}/`
- `frontend/src/views/login/components/{LoginPhone,LoginQrCode,LoginRegist,LoginUpdate}.vue`
- `frontend/src/plugins/vxeTable.ts`
- `frontend/src/components/Re{AnimateSelector,Auth,Barcode,Col,CountTo,Cropper,CropperPreview,Flicker,Flop,FlowChart,ImageVerify,Map,SeamlessScroll,Segmented,Selector,SplitPane,TreeLine,Typeit,VxeTableBar}/`
- `frontend/src/api/{list,mock,routes,system,user}.ts`

---

## Task 1: 备份现状并裁剪 package.json

**Files:**
- Modify: `frontend/package.json`

- [ ] **Step 1: 记录依赖现状**

Run:
```powershell
cd d:\projects\coder-micro-services\frontend
Get-Content package.json | Out-File -Encoding utf8 ..\..\reports\frontend-package-before.json
```
Expected: `reports/frontend-package-before.json` 出现且包含 v7.0.0 完整依赖。

- [ ] **Step 2: 替换 dependencies 段**

在 `frontend/package.json` 中，把 `"dependencies"` 整段（`"@amap/amap-jsapi-loader": "^1.0.1",` 起一直到 `"xlsx": "^0.18.5"`）替换为：

```json
  "dependencies": {
    "@pureadmin/utils": "^2.6.4",
    "@vueuse/core": "^14.3.0",
    "@vueuse/motion": "^3.0.3",
    "axios": "1.14.0",
    "dayjs": "^1.11.21",
    "echarts": "^6.1.0",
    "element-plus": "^2.14.1",
    "js-cookie": "^3.0.8",
    "localforage": "^1.10.0",
    "mitt": "^3.0.1",
    "nprogress": "^0.2.0",
    "pinia": "^3.0.4",
    "qs": "^6.15.2",
    "responsive-storage": "^2.2.0",
    "vue": "^3.5.35",
    "vue-i18n": "^11.4.4",
    "vue-router": "^5.1.0"
  },
```

- [ ] **Step 3: 替换 devDependencies 段**

把 `"devDependencies"` 整段替换为：

```json
  "devDependencies": {
    "@commitlint/cli": "^21.0.2",
    "@commitlint/config-conventional": "^21.0.2",
    "@commitlint/types": "^21.0.1",
    "@eslint/js": "^10.0.1",
    "@iconify/json": "^2.2.481",
    "@iconify/vue": "4.2.0",
    "@intlify/unplugin-vue-i18n": "^11.2.3",
    "@tailwindcss/vite": "^4.3.0",
    "@types/js-cookie": "^3.0.6",
    "@types/node": "^20.19.41",
    "@types/nprogress": "^0.2.3",
    "@types/qs": "^6.15.1",
    "@vitejs/plugin-vue": "^6.0.7",
    "@vitejs/plugin-vue-jsx": "^5.1.5",
    "boxen": "^8.0.1",
    "code-inspector-plugin": "^1.5.1",
    "cssnano": "^8.0.1",
    "eslint": "^10.4.1",
    "eslint-config-prettier": "^10.1.8",
    "eslint-plugin-better-tailwindcss": "^4.5.0",
    "eslint-plugin-prettier": "^5.5.6",
    "eslint-plugin-vue": "^10.9.2",
    "gradient-string": "^3.0.0",
    "husky": "^9.1.7",
    "lint-staged": "^17.0.7",
    "postcss": "^8.5.15",
    "postcss-html": "^1.8.1",
    "postcss-load-config": "^6.0.1",
    "postcss-scss": "^4.0.9",
    "prettier": "^3.8.3",
    "rimraf": "^6.1.3",
    "rollup-plugin-visualizer": "^7.0.1",
    "sass": "^1.100.0",
    "stylelint": "^17.12.0",
    "stylelint-config-recess-order": "^7.7.0",
    "stylelint-config-recommended-vue": "^1.6.1",
    "stylelint-config-standard-scss": "^17.0.0",
    "stylelint-prettier": "^5.0.3",
    "svgo": "^4.0.1",
    "tailwindcss": "^4.3.0",
    "typescript": "^6.0.3",
    "typescript-eslint": "^8.60.1",
    "unplugin-icons": "^23.0.1",
    "vite": "8.0.10",
    "vite-plugin-cdn-import": "^1.0.1",
    "vite-plugin-compression": "^0.5.1",
    "vite-plugin-remove-console": "^2.2.0",
    "vite-plugin-router-warn": "^2.0.0",
    "vite-svg-loader": "^5.1.1",
    "vue-eslint-parser": "^10.4.1",
    "vue-tsc": "^3.3.3"
  },
```

- [ ] **Step 4: JSON 语法校验**

Run:
```powershell
Get-Content frontend\package.json -Raw | ConvertFrom-Json | Out-Null
```
Expected: 无输出，退出码 0。

---

## Task 2: 移除 vite-plugin-fake-server

**Files:**
- Modify: `frontend/vite.config.ts`

- [ ] **Step 1: 读 vite.config.ts 确认 fake-server 注册位置**

Run:
```powershell
Select-String -Path frontend\vite.config.ts -Pattern "fake-server|createServer|FAKE_SERVER"
```
Expected: 输出 `fake-server` 与 `createServer` 两行。

- [ ] **Step 2: 移除 fake-server import 与 plugin**

删除 `import { createServer } from "vite-plugin-fake-server";` 整行；
删除 `fakeServer({ log: false })` 整行（含上一行的尾逗号处理）；
把 `viteServer = await createServer({})` 整段删除（若存在）。

- [ ] **Step 3: 检查 plugins 数组**

```powershell
Select-String -Path frontend\vite.config.ts -Pattern "plugins:"
```
Expected: 仍存在，且数组内无 fakeServer。

---

## Task 3: 移除 mock 目录与公开死资源

**Files:**
- Delete: `frontend/mock/`
- Delete: `frontend/public/audio/`
- Delete: `frontend/public/wasm/`
- Delete: `frontend/public/html/button.html`

- [ ] **Step 1: 删除 mock/**

```powershell
Remove-Item -Recurse -Force frontend\mock
```
Expected: `Test-Path frontend\mock` 返回 `False`。

- [ ] **Step 2: 删除 public/audio 与 public/wasm 与 public/html/button.html**

```powershell
Remove-Item -Recurse -Force frontend\public\audio
Remove-Item -Recurse -Force frontend\public\wasm
Remove-Item -Force frontend\public\html\button.html
```
Expected: 三处均 `False`。

---

## Task 4: 删除 demo 路由模块

**Files:**
- Delete: `frontend/src/router/modules/able.ts`
- Delete: `frontend/src/router/modules/about.ts`
- Delete: `frontend/src/router/modules/board.ts`
- Delete: `frontend/src/router/modules/chatai.ts`
- Delete: `frontend/src/router/modules/codemirror.ts`
- Delete: `frontend/src/router/modules/components.ts`
- Delete: `frontend/src/router/modules/editor.ts`
- Delete: `frontend/src/router/modules/flowchart.ts`
- Delete: `frontend/src/router/modules/form.ts`
- Delete: `frontend/src/router/modules/formdesign.ts`
- Delete: `frontend/src/router/modules/ganttastic.ts`
- Delete: `frontend/src/router/modules/guide.ts`
- Delete: `frontend/src/router/modules/list.ts`
- Delete: `frontend/src/router/modules/markdown.ts`
- Delete: `frontend/src/router/modules/menuoverflow.ts`
- Delete: `frontend/src/router/modules/mind.ts`
- Delete: `frontend/src/router/modules/nested.ts`
- Delete: `frontend/src/router/modules/ppt.ts`
- Delete: `frontend/src/router/modules/result.ts`
- Delete: `frontend/src/router/modules/table.ts`
- Delete: `frontend/src/router/modules/vueflow.ts`

- [ ] **Step 1: 整批删除**

```powershell
Get-ChildItem frontend\src\router\modules\*.ts |
  Where-Object { $_.Name -notin @('home.ts','system.ts','error.ts') } |
  Remove-Item -Force
```
Expected: `frontend/src/router/modules/` 目录仅剩 `home.ts / system.ts / error.ts`。

- [ ] **Step 2: 校验**

```powershell
(Get-ChildItem frontend\src\router\modules).Name
```
Expected: `home.ts`、`system.ts`、`error.ts`。

---

## Task 5: 删除 demo 视图目录

**Files:**
- Delete: `frontend/src/views/able/`
- Delete: `frontend/src/views/about/`
- Delete: `frontend/src/views/chatai/`
- Delete: `frontend/src/views/codemirror/`
- Delete: `frontend/src/views/components/`
- Delete: `frontend/src/views/editor/`
- Delete: `frontend/src/views/flow-chart/`
- Delete: `frontend/src/views/ganttastic/`
- Delete: `frontend/src/views/guide/`
- Delete: `frontend/src/views/list/`
- Delete: `frontend/src/views/markdown/`
- Delete: `frontend/src/views/menuoverflow/`
- Delete: `frontend/src/views/mind/`
- Delete: `frontend/src/views/monitor/`
- Delete: `frontend/src/views/nested/`
- Delete: `frontend/src/views/permission/`
- Delete: `frontend/src/views/ppt/`
- Delete: `frontend/src/views/result/`
- Delete: `frontend/src/views/schema-form/`
- Delete: `frontend/src/views/table/`
- Delete: `frontend/src/views/tabs/`
- Delete: `frontend/src/views/vue-flow/`
- Delete: `frontend/src/views/empty/`
- Delete: `frontend/src/views/login/components/LoginPhone.vue`
- Delete: `frontend/src/views/login/components/LoginQrCode.vue`
- Delete: `frontend/src/views/login/components/LoginRegist.vue`
- Delete: `frontend/src/views/login/components/LoginUpdate.vue`

- [ ] **Step 1: 整批删除 views 子目录**

```powershell
$demos = @('able','about','chatai','codemirror','components','editor','flow-chart','ganttastic','guide','list','markdown','menuoverflow','mind','monitor','nested','permission','ppt','result','schema-form','table','tabs','vue-flow','empty')
$demos | ForEach-Object { Remove-Item -Recurse -Force frontend\src\views\$_ }
```
Expected: `Test-Path frontend\src\views\$_` 对每个均返回 `False`。

- [ ] **Step 2: 删除登录多模式组件**

```powershell
Remove-Item -Force frontend\src\views\login\components\LoginPhone.vue
Remove-Item -Force frontend\src\views\login\components\LoginQrCode.vue
Remove-Item -Force frontend\src\views\login\components\LoginRegist.vue
Remove-Item -Force frontend\src\views\login\components\LoginUpdate.vue
```
Expected: 4 个 `Test-Path` 均 `False`。

---

## Task 6: 删除多余 Re* 组件与 vxeTable 插件

**Files:**
- Delete: `frontend/src/components/Re{AnimateSelector,Auth,Barcode,Col,CountTo,Cropper,CropperPreview,Flicker,Flop,FlowChart,ImageVerify,Map,SeamlessScroll,Segmented,Selector,SplitPane,TreeLine,Typeit,VxeTableBar}/`
- Delete: `frontend/src/plugins/vxeTable.ts`

- [ ] **Step 1: 整批删除多余 Re* 目录**

```powershell
$reKeep = @('RePureTableBar','ReDialog','ReDrawer','RePerms','ReIcon','ReText')
Get-ChildItem frontend\src\components |
  Where-Object { $_.Name -notin $reKeep } |
  Remove-Item -Recurse -Force
```
Expected: `frontend/src/components/` 仅留 `RePureTableBar / ReDialog / ReDrawer / RePerms / ReIcon / ReText`。

- [ ] **Step 2: 删除 vxeTable 插件**

```powershell
Remove-Item -Force frontend\src\plugins\vxeTable.ts
```
Expected: `Test-Path` 返回 `False`。

- [ ] **Step 3: 校验 main.ts 是否引用 vxeTable**

```powershell
Select-String -Path frontend\src\main.ts -Pattern "vxeTable|VXETable"
```
Expected: 无输出（main.ts 后续会改写，本步仅排查现状）。

---

## Task 7: 改写 router/enums.ts

**Files:**
- Modify: `frontend/src/router/enums.ts`

- [ ] **Step 1: 全文替换**

把 `frontend/src/router/enums.ts` 整文件覆盖为：

```ts
const home = 0,
  system = 1,
  account = 2,
  error = 3;

export { home, system, account, error };
```

- [ ] **Step 2: 校验**

```powershell
Get-Content frontend\src\router\enums.ts
```
Expected: 4 行常量定义 + 1 行 export。

---

## Task 8: 改写 router/index.ts

**Files:**
- Modify: `frontend/src/router/index.ts`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\router\index.ts
```
定位 `import.meta.glob("./modules/**/*.ts", { eager: true })` 与 modules 列表。

- [ ] **Step 2: 替换 modules glob 引入**

把 `const modules: Record<string, any> = import.meta.glob("./modules/**/*.ts", { eager: true });` 替换为：

```ts
const routeModuleRecord: Record<string, { default: RouteConfigsTable }> = import.meta.glob(
  "./modules/**/*.ts",
  { eager: true }
);
```

把 `const modules: Record<string, any> = import.meta.glob("./modules/**/*.ts", { eager: true });` 之后到 `dynamicRouter = []` 之间的 `Object.keys(modules).forEach(...)` 整段替换为：

```ts
const modules: RouteConfigsTable[] = [];
Object.keys(routeModuleRecord).forEach(key => {
  const mod = routeModuleRecord[key].default;
  if (!mod) return;
  const modList = Array.isArray(mod) ? [...mod] : [mod];
  modules.push(...modList);
});
```

- [ ] **Step 3: 过滤排序逻辑保留原实现**

`const ascendingRouter = (modules || []).sort(...)` 整段保留，只把 `modules` 来源换成上一步的合并结果。

- [ ] **Step 4: typecheck**

```powershell
cd frontend; pnpm typecheck
```
Expected: 路由相关类型错误 0（其它模块引用错会在后续任务修）。

---

## Task 9: 改写 router/modules/home.ts 与新建 account.ts

**Files:**
- Modify: `frontend/src/router/modules/home.ts`
- Create: `frontend/src/router/modules/account.ts`

- [ ] **Step 1: 改写 home.ts**

`frontend/src/router/modules/home.ts` 整文件覆盖为：

```ts
import { $t } from "@/plugins/i18n";
import { home } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/",
  name: "Home",
  component: Layout,
  redirect: "/welcome",
  meta: {
    icon: "ep/home-filled",
    title: $t("menus.pureHome"),
    rank: home
  },
  children: [
    {
      path: "/welcome",
      name: "Welcome",
      component: () => import("@/views/welcome/index.vue"),
      meta: {
        title: $t("menus.pureHome"),
        showLink: true
      }
    }
  ]
} satisfies RouteConfigsTable;
```

- [ ] **Step 2: 新建 account.ts**

`frontend/src/router/modules/account.ts` 整文件写为：

```ts
import { $t } from "@/plugins/i18n";
import { account } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/account-settings",
  component: Layout,
  redirect: "/account-settings/profile",
  name: "AccountSettings",
  meta: {
    title: $t("menus.accountSettings"),
    icon: "ep/user",
    rank: account,
    showLink: false,
    hidden: true
  },
  children: [
    {
      path: "/account-settings/profile",
      name: "AccountProfile",
      component: () => import("@/views/account-settings/components/Profile.vue"),
      meta: {
        title: $t("menus.accountProfile"),
        showLink: false,
        hidden: true
      }
    },
    {
      path: "/account-settings/security",
      name: "AccountSecurity",
      component: () => import("@/views/account-settings/components/SecurityLog.vue"),
      meta: {
        title: $t("menus.accountSecurity"),
        showLink: false,
        hidden: true
      }
    }
  ]
} satisfies RouteConfigsTable;
```

- [ ] **Step 3: 校验**

```powershell
Get-ChildItem frontend\src\router\modules
```
Expected: `error.ts / home.ts / system.ts / account.ts` 4 个。

---

## Task 10: 改写 router/modules/system.ts

**Files:**
- Modify: `frontend/src/router/modules/system.ts`

- [ ] **Step 1: 整文件覆盖**

`frontend/src/router/modules/system.ts` 整文件写为：

```ts
import { $t } from "@/plugins/i18n";
import { system } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/system",
  component: Layout,
  redirect: "/system/user",
  name: "System",
  meta: {
    title: $t("menus.system"),
    icon: "ep/set-up",
    rank: system
  },
  children: [
    {
      path: "/system/user",
      name: "SystemUser",
      component: () => import("@/views/system/user/index.vue"),
      meta: {
        title: $t("menus.systemUser"),
        icon: "ep/user",
        keepAlive: true
      }
    },
    {
      path: "/system/role",
      name: "SystemRole",
      component: () => import("@/views/system/role/index.vue"),
      meta: {
        title: $t("menus.systemRole"),
        icon: "ep/user-filled",
        keepAlive: true
      }
    },
    {
      path: "/system/permission",
      name: "SystemPermission",
      component: () => import("@/views/system/permission/index.vue"),
      meta: {
        title: $t("menus.systemPermission"),
        icon: "ep/key",
        keepAlive: true
      }
    }
  ]
} satisfies RouteConfigsTable;
```

- [ ] **Step 2: 校验**

```powershell
Select-String -Path frontend\src\router\modules\system.ts -Pattern "user|role|permission"
```
Expected: 3 个 child 路由条目。

---

## Task 11: 改写 router/modules/error.ts

**Files:**
- Modify: `frontend/src/router/modules/error.ts`

- [ ] **Step 1: 整文件覆盖**

`frontend/src/router/modules/error.ts` 整文件写为：

```ts
import { $t } from "@/plugins/i18n";
import { error } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/error",
  component: Layout,
  redirect: "/error/404",
  name: "Error",
  meta: {
    title: $t("menus.error"),
    icon: "ep/circle-close",
    rank: error,
    showLink: false,
    hidden: true
  },
  children: [
    {
      path: "/error/403",
      name: "Error403",
      component: () => import("@/views/error/403.vue"),
      meta: { title: "403", showLink: false, hidden: true }
    },
    {
      path: "/error/404",
      name: "Error404",
      component: () => import("@/views/error/404.vue"),
      meta: { title: "404", showLink: false, hidden: true }
    },
    {
      path: "/error/500",
      name: "Error500",
      component: () => import("@/views/error/500.vue"),
      meta: { title: "500", showLink: false, hidden: true }
    }
  ]
} satisfies RouteConfigsTable;
```

---

## Task 12: 新建 src/api/auth.ts

**Files:**
- Create: `frontend/src/api/auth.ts`

- [ ] **Step 1: 写入文件**

`frontend/src/api/auth.ts` 整文件写为：

```ts
import { http } from "@/utils/http";

export interface LoginRequest {
  username: string;
  password: string;
}

export interface LoginResult {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

export function login(data: LoginRequest) {
  return http.request<LoginResult>("post", "/auth/login", { data });
}

export function logout() {
  return http.request<void>("post", "/auth/logout");
}

export function register(data: LoginRequest & { email?: string }) {
  return http.request<LoginResult>("post", "/auth/register", { data });
}
```

- [ ] **Step 2: 校验**

```powershell
Select-String -Path frontend\src\utils\http\index.ts -Pattern "request"
```
Expected: 出现 `request` 泛型方法定义（确认 http.request 用法与 http 工具签名一致）。

---

## Task 13: 新建 src/api/user.ts

**Files:**
- Create: `frontend/src/api/user.ts`

- [ ] **Step 1: 写入文件**

`frontend/src/api/user.ts` 整文件写为：

```ts
import { http } from "@/utils/http";

export interface User {
  id: number;
  username: string;
  email: string;
  nickname: string;
  avatar: string;
  status: number;
  roles: string[];
  created_at: string;
  updated_at: string;
}

export interface ListUsersQuery {
  page?: number;
  size?: number;
  keyword?: string;
  status?: number;
}

export interface PageResult<T> {
  list: T[];
  total: number;
  page: number;
  size: number;
}

export function getCurrentUser() {
  return http.request<User>("get", "/users/me");
}

export function updateCurrentUser(data: Partial<User>) {
  return http.request<User>("put", "/users/me", { data });
}

export function listUsers(params: ListUsersQuery) {
  return http.request<PageResult<User>>("get", "/users", { params });
}

export function getUser(id: number) {
  return http.request<User>("get", `/users/${id}`);
}

export function patchUserExtends(id: number, data: Record<string, unknown>) {
  return http.request<User>("patch", `/users/${id}/extends`, { data });
}

export function assignUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("post", `/users/${id}/roles`, { data: { role_ids: roleIds } });
}

export function removeUserRoles(id: number, roleIds: number[]) {
  return http.request<void>("delete", `/users/${id}/roles`, { data: { role_ids: roleIds } });
}
```

---

## Task 14: 新建 src/api/role.ts

**Files:**
- Create: `frontend/src/api/role.ts`

- [ ] **Step 1: 写入文件**

`frontend/src/api/role.ts` 整文件写为：

```ts
import { http } from "@/utils/http";
import type { PageResult } from "@/api/user";

export interface Role {
  id: number;
  name: string;
  code: string;
  description: string;
  status: number;
  created_at: string;
  updated_at: string;
}

export function listRoles(params: { page?: number; size?: number; keyword?: string }) {
  return http.request<PageResult<Role>>("get", "/roles", { params });
}

export function createRole(data: Pick<Role, "name" | "code" | "description">) {
  return http.request<Role>("post", "/roles", { data });
}

export function getRole(id: number) {
  return http.request<Role>("get", `/roles/${id}`);
}

export function updateRole(id: number, data: Pick<Role, "name" | "code" | "description" | "status">) {
  return http.request<Role>("put", `/roles/${id}`, { data });
}

export function deleteRole(id: number) {
  return http.request<void>("delete", `/roles/${id}`);
}

export function assignRolePermissions(id: number, permissionIds: number[]) {
  return http.request<void>("post", `/roles/${id}/permissions`, { data: { permission_ids: permissionIds } });
}
```

---

## Task 15: 新建 src/api/permission.ts 与 src/api/token.ts

**Files:**
- Create: `frontend/src/api/permission.ts`
- Create: `frontend/src/api/token.ts`

- [ ] **Step 1: 写入 permission.ts**

`frontend/src/api/permission.ts` 整文件写为：

```ts
import { http } from "@/utils/http";

export interface Permission {
  id: number;
  code: string;
  name: string;
  resource: string;
  action: string;
}

export function listPermissions() {
  return http.request<Permission[]>("get", "/permissions");
}
```

- [ ] **Step 2: 写入 token.ts**

`frontend/src/api/token.ts` 整文件写为：

```ts
import { http } from "@/utils/http";

export interface AccessTokenRequest {
  client_id: string;
  client_secret: string;
  scope?: string;
}

export interface AccessTokenResult {
  access_token: string;
  token_type: string;
  expires_in: number;
}

export function createAccessToken(data: AccessTokenRequest) {
  return http.request<AccessTokenResult>("post", "/tokens/access", { data });
}

export function refreshToken(refresh_token: string) {
  return http.request<AccessTokenResult>("post", "/tokens/refresh", { data: { refresh_token } });
}

export function revokeToken(token: string) {
  return http.request<void>("post", "/tokens/revoke", { data: { token } });
}

export function listResources() {
  return http.request<string[]>("get", "/tokens/resource");
}

export function createApiKey(data: { name: string; scopes?: string[] }) {
  return http.request<{ api_key: string; id: string }>("post", "/tokens/api_key", { data });
}
```

- [ ] **Step 3: 删除多余 api 文件**

```powershell
Remove-Item -Force frontend\src\api\list.ts
Remove-Item -Force frontend\src\api\mock.ts
Remove-Item -Force frontend\src\api\routes.ts
Remove-Item -Force frontend\src\api\system.ts
Remove-Item -Force frontend\src\api\user.ts
```
Expected: `frontend/src/api/` 目录仅留 `auth.ts / user.ts / role.ts / permission.ts / token.ts`。

---

## Task 16: 改写 store/modules/user.ts

**Files:**
- Modify: `frontend/src/store/modules/user.ts`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\store\modules\user.ts
```
定位 `loginByUsername` 与 `setUserInfo` 实现。

- [ ] **Step 2: 替换 loginByUsername**

把 `loginByUsername` 整段（含 mock 鉴权分支）替换为：

```ts
async loginByUsername(data: LoginRequest) {
  return new Promise<UserLoginResult>((resolve, reject) => {
    login(data)
      .then(res => {
        setToken(res.data);
        resolve(res.data);
      })
      .catch(err => reject(err));
  });
}
```

- [ ] **Step 3: 替换 setUserInfo**

把 `setUserInfo` 中 mock 字段构造逻辑替换为：

```ts
async setUserInfo() {
  const { data } = await getCurrentUser();
  this.avatar = data.avatar;
  this.username = data.username;
  this.nickname = data.nickname;
  this.roles = data.roles ?? [];
  return data;
}
```

- [ ] **Step 4: 引入新 api**

在文件顶部 `import` 段补：

```ts
import { login, LoginRequest, LoginResult } from "@/api/auth";
import { getCurrentUser, User } from "@/api/user";
```

并把 `UserLoginResult` 旧类型引用统一替换为 `LoginResult`；旧 `UserInfo` 引用统一替换为 `User`。

---

## Task 17: 改写 store/modules/permission.ts

**Files:**
- Modify: `frontend/src/store/modules/permission.ts`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\store\modules\permission.ts
```
定位 `handleWholeRoutes` / `asyncRouter` / `initRoutes` 函数。

- [ ] **Step 2: 移除 dynamicRouter 列表注入**

把 `import("@/router/modules/asyncRoutes")` 之类 demo 路由动态加载段删除。
asyncRoutes.ts 已在 Task 4 删除前清理，本步确保无 import 残留。

- [ ] **Step 3: 后端权限码回填逻辑**

把 `getRole()` 旧 mock 段替换为：

```ts
async getRole() {
  const { data } = await listPermissions();
  this.permissions = data.map(p => p.code);
  return data;
}
```

- [ ] **Step 4: 引入新 api**

在文件顶部补：

```ts
import { listPermissions } from "@/api/permission";
```

- [ ] **Step 5: 校验**

```powershell
Select-String -Path frontend\src\store\modules\permission.ts -Pattern "asyncRoutes|listPermissions"
```
Expected: 仅 `listPermissions` 命中。

---

## Task 18: 改写 views/login/index.vue

**Files:**
- Modify: `frontend/src/views/login/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\login\index.vue
```
定位账号密码表单组件 `<LoginAccount>` 或 `<user-login>` 段。

- [ ] **Step 2: 简化登录表单**

保留账号密码输入框 + 登录按钮；
删除 `<LoginPhone>` / `<LoginQrCode>` / `<LoginRegist>` / `<LoginUpdate>` 引用；
登录提交回调改为：

```ts
import { useUserStore } from "@/store/modules/user";
import { useRouter } from "vue-router";

const userStore = useUserStore();
const router = useRouter();

async function onLogin(form: { username: string; password: string }) {
  loading.value = true;
  try {
    await userStore.loginByUsername(form);
    await userStore.setUserInfo();
    await router.push("/welcome");
  } finally {
    loading.value = false;
  }
}
```

- [ ] **Step 3: 校验**

```powershell
Select-String -Path frontend\src\views\login\index.vue -Pattern "LoginPhone|LoginQrCode|LoginRegist|LoginUpdate"
```
Expected: 无输出。

---

## Task 19: 改写 views/welcome/index.vue

**Files:**
- Modify: `frontend/src/views/welcome/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\welcome\index.vue
```

- [ ] **Step 2: 移除 mock 数据源**

把 `import { chartData, pieData } from "./data";` 删除；
把图表 `<echart :data="chartData" />` 与 `<echart :data="pieData" />` 删除或替换为 4 个空 KPI 卡片。

- [ ] **Step 3: KPI 数据从真后端拉取**

```ts
import { listUsers } from "@/api/user";
import { listRoles } from "@/api/role";

const stats = reactive({ users: 0, roles: 0, online: 0, todayLogins: 0 });
onMounted(async () => {
  const [u, r] = await Promise.all([listUsers({ page: 1, size: 1 }), listRoles({ page: 1, size: 1 })]);
  stats.users = u.data.total;
  stats.roles = r.data.total;
});
```

模板保留 4 个 `<el-card>` 显示 stats 四个字段。

- [ ] **Step 4: 校验**

```powershell
Select-String -Path frontend\src\views\welcome\index.vue -Pattern "data\."
```
Expected: 仅 `data\.(total|list)` 等业务字段，无 `chartData` / `pieData`。

---

## Task 20: 改写 views/system/user/index.vue

**Files:**
- Modify: `frontend/src/views/system/user/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\system\user\index.vue
```
定位表格 `dataList` 初始化与 `onSearch` / `onAdd` / `onDelete` 函数。

- [ ] **Step 2: 接入 listUsers**

```ts
import { listUsers, deleteUser as _unused } from "@/api/user";
```

> 注：后端无 DELETE /api/users/:id，按 spec 6 节映射，只接 PATCH/extends。新增 / 删除按钮改走后端未提供时弹 toast 提示"后端未提供"。

把 `dataList` 初始化替换为：

```ts
async function onSearch() {
  loading.value = true;
  try {
    const { data } = await listUsers({ page: pagination.page, size: pagination.size, keyword: search.keyword, status: search.status });
    dataList.value = data.list;
    pagination.total = data.total;
  } finally {
    loading.value = false;
  }
}
```

- [ ] **Step 3: 校验**

```powershell
Select-String -Path frontend\src\views\system\user\index.vue -Pattern "listUsers|mock"
```
Expected: `listUsers` 命中，无 `mock`。

---

## Task 21: 改写 views/system/role/index.vue

**Files:**
- Modify: `frontend/src/views/system/role/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\system\role\index.vue
```

- [ ] **Step 2: 接入 listRoles / createRole / updateRole / deleteRole**

```ts
import { listRoles, createRole, updateRole, deleteRole } from "@/api/role";
```

把 `dataList` 初始化与表单提交回调全部替换为对上面 4 个函数的调用；保留表格、分页、新增 / 编辑 / 删除 / 分配权限 4 个操作。

分配权限按钮回调：

```ts
async function onAssignPermissions(row: Role, ids: number[]) {
  await assignRolePermissions(row.id, ids);
  ElMessage.success("已保存");
}
```

- [ ] **Step 3: 校验**

```powershell
Select-String -Path frontend\src\views\system\role\index.vue -Pattern "listRoles|createRole|deleteRole|mock"
```
Expected: 前 3 个命中，无 `mock`。

---

## Task 22: 新建 views/system/permission/index.vue

**Files:**
- Modify: `frontend/src/views/system/permission/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\system\permission\index.vue
```

- [ ] **Step 2: 替换为只读列表**

整文件覆盖为：

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { listPermissions, type Permission } from "@/api/permission";

const dataList = ref<Permission[]>([]);
const loading = ref(false);

async function load() {
  loading.value = true;
  try {
    const { data } = await listPermissions();
    dataList.value = data;
  } finally {
    loading.value = false;
  }
}

onMounted(load);
</script>

<template>
  <el-table v-loading="loading" :data="dataList" border>
    <el-table-column prop="code" label="权限码" min-width="180" />
    <el-table-column prop="name" label="名称" min-width="160" />
    <el-table-column prop="resource" label="资源" min-width="160" />
    <el-table-column prop="action" label="动作" min-width="120" />
  </el-table>
</template>
```

- [ ] **Step 3: 校验**

```powershell
Test-Path frontend\src\views\system\permission\index.vue
```
Expected: `True`。

---

## Task 23: 改写 views/account-settings/index.vue

**Files:**
- Modify: `frontend/src/views/account-settings/index.vue`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\views\account-settings\index.vue
```
定位 `<AccountManagement>` / `<Preferences>` / `<SecurityLog>` 子组件引用。

- [ ] **Step 2: 删除 AccountManagement 引用**

`<AccountManagement />` 标签删除（后端无 /api/accounts 子资源管理），保留 `<Profile>` / `<Preferences>` / `<SecurityLog>` 三个 tab。

- [ ] **Step 3: 校验**

```powershell
Select-String -Path frontend\src\views\account-settings\index.vue -Pattern "AccountManagement"
```
Expected: 无输出。

---

## Task 24: 改写 views/account-settings/components/Profile.vue

**Files:**
- Modify: `frontend/src/views/account-settings/components/Profile.vue`

- [ ] **Step 1: 替换为接 GET/PUT /api/users/me**

```ts
import { getCurrentUser, updateCurrentUser } from "@/api/user";

const form = reactive({ nickname: "", email: "", avatar: "" });
onMounted(async () => {
  const { data } = await getCurrentUser();
  Object.assign(form, { nickname: data.nickname, email: data.email, avatar: data.avatar });
});
async function onSave() {
  await updateCurrentUser(form);
  ElMessage.success("已保存");
}
```

模板保留 3 个 `<el-form-item>`：nickname、email、avatar。

---

## Task 25: 改写 views/account-settings/components/SecurityLog.vue

**Files:**
- Modify: `frontend/src/views/account-settings/components/SecurityLog.vue`

- [ ] **Step 1: 替换为 token 撤销记录**

```ts
import { ref, onMounted } from "vue";
import { revokeToken, listResources } from "@/api/token";

const sessions = ref<{ token: string; created_at: string; scopes: string[] }[]>([]);
onMounted(async () => {
  const raw = localStorage.getItem("coder-tokens") ?? "[]";
  sessions.value = JSON.parse(raw);
});
async function onRevoke(token: string) {
  await revokeToken(token);
  sessions.value = sessions.value.filter(s => s.token !== token);
  ElMessage.success("已撤销");
}
```

模板保留表格 + 撤销按钮，3 列：token（前 12 位）、created_at、操作。

---

## Task 26: 改写 main.ts

**Files:**
- Modify: `frontend/src/main.ts`

- [ ] **Step 1: 读现状**

Run:
```powershell
Get-Content frontend\src\main.ts
```

- [ ] **Step 2: 移除 vxeTable 引用**

把 `import VxeTable from "vxe-table";` 整行删除；
把 `import "vxe-table/lib/style.css";` 整行删除；
把 `app.use(VxeTable)` 整行删除；
保留其余 elementPlus / i18n / echarts / pinia / router / auth 指令注册。

- [ ] **Step 3: 校验**

```powershell
Select-String -Path frontend\src\main.ts -Pattern "vxe|Vxe"
```
Expected: 无输出。

---

## Task 27: 精简 locales/zh-CN.yaml 与 en.yaml

**Files:**
- Modify: `frontend/locales/zh-CN.yaml`
- Modify: `frontend/locales/en.yaml`

- [ ] **Step 1: 备份原文件**

```powershell
Copy-Item frontend\locales\zh-CN.yaml ..\..\reports\frontend-zh-CN-before.yaml
Copy-Item frontend\locales\en.yaml ..\..\reports\frontend-en-before.yaml
```

- [ ] **Step 2: 替换 zh-CN.yaml 关键菜单项**

把 `menus:` 段中：

```yaml
pureHome: 首页
system: 系统管理
systemUser: 用户管理
systemRole: 角色管理
systemPermission: 权限列表
accountSettings: 个人中心
accountProfile: 个人资料
accountSecurity: 安全日志
error: 异常页
```

其余 demo 菜单项（`chatai / vueflow / ganttastic / components / able / table / form / list / result / frame / nested / permission / monitor / tabs / about / codemirror / markdown / editor / flowchart / formdesign / board / ppt / mind / guide / menuoverflow`）整段删除。

- [ ] **Step 3: 同步 en.yaml 菜单项**

把 `menus:` 段中：

```yaml
pureHome: Home
system: System
systemUser: Users
systemRole: Roles
systemPermission: Permissions
accountSettings: Account
accountProfile: Profile
accountSecurity: Security
error: Error
```

其余 demo 菜单键整段删除。

- [ ] **Step 4: 校验**

```powershell
Select-String -Path frontend\locales\zh-CN.yaml -Pattern "chatai|flowchart|ganttastic"
```
Expected: 无输出。

---

## Task 28: 安装依赖

**Files:**
- 触发: `frontend/pnpm-lock.yaml` 重生

- [ ] **Step 1: 清理旧 lock 与缓存**

```powershell
cd frontend
Remove-Item -Force pnpm-lock.yaml -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force node_modules -ErrorAction SilentlyContinue
```

- [ ] **Step 2: 安装**

```powershell
pnpm install
```
Expected: 退出码 0；警告可接受；错误不接受。

---

## Task 29: 校验构建

**Files:**
- 触发: `frontend/dist/`

- [ ] **Step 1: 类型检查**

```powershell
cd frontend
pnpm typecheck
```
Expected: 退出码 0。

- [ ] **Step 2: 修复 typecheck 报告的类型错误**

按 `pnpm typecheck` 输出逐条改：
- 残留 `import.meta.glob` 旧类型 → 在 `src/types/router.d.ts` 中确保 `RouteConfigsTable` 仍存在；
- `v-perms` 指令类型 → 在 `src/directives/perms/index.ts` 中导出 `Directive` 类型；
- 其它：按报错文件行号 fix。

- [ ] **Step 3: lint**

```powershell
pnpm lint:eslint
```
Expected: 退出码 0。

- [ ] **Step 4: 修复 lint 错误**

按 `pnpm lint:eslint` 输出逐条改；`pnpm lint:prettier` / `pnpm lint:stylelint` 同理。

- [ ] **Step 5: 生产构建**

```powershell
pnpm build
```
Expected: 退出码 0；`frontend/dist/assets` 总体积 gzip < 1.5 MB。

- [ ] **Step 6: 体积验证**

```powershell
$gzip = Get-ChildItem frontend\dist\assets\*.gz -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
if ($gzip) { "{0:N2} MB" -f ($gzip.Sum / 1MB) }
```
Expected: 数值 < 1.50。

---

## Task 30: 端到端冒烟（手工）

**Files:**
- 触发: 启动 dev server

- [ ] **Step 1: 起前端**

```powershell
cd frontend
pnpm dev
```
Expected: Vite 起在 `http://localhost:8848`，无报错。

- [ ] **Step 2: 端到端路径**

浏览器打开 `http://localhost:8848/login`：
1. 输入 `admin / admin`（依赖后端 seed 用户）→ 登录成功 → 跳转 `/welcome`
2. 顶部菜单出现 5 个一级菜单 + 个人中心隐藏入口
3. `/system/user` 表格加载真实用户列表
4. `/system/role` 表格加载真实角色列表
5. `/system/permission` 表格加载真实权限码列表
6. 访问 `/error/404` 跳 404 页
7. 直接访问 `/welcome` 未登录 → 跳 `/login`

Expected: 7 项全部通过。

---

## Task 31: 交付（等用户决策 commit）

- [ ] **Step 1: 询问用户是否 commit**

> 项目规则要求 AI 不主动 commit。请确认是否执行 `git add frontend/ docs/ && git commit -m "feat(frontend): 裁剪到极简骨架"`，并指定推送分支。

---

## Self-Review

### 1. Spec coverage
- § 1 背景（描述现状）— Task 1~6 删除
- § 3 路由结构 — Task 9~11
- § 4 目录清单 — Task 4~6 删除 + Task 9~11 改写
- § 5 依赖裁剪 — Task 1
- § 6 API 映射 — Task 12~15
- § 7 鉴权适配 — Task 16~17
- § 8 .env 与代理 — 由 `coder-microservice-dev` skill 维护 nginx，不在本 plan
- § 9 验收 — Task 29~30
- § 10 不做的事 — 全 plan 显式遵守

### 2. Placeholder scan
- 无 TBD / TODO / "implement later"
- 无 "类似 Task N" 偷懒

### 3. Type consistency
- `User` 在 `src/api/user.ts` Task 13 定义，在 Task 16、19、24 引用
- `LoginResult` / `LoginRequest` 在 Task 12 定义，Task 16 引用
- `Role` / `Permission` 在 Task 14、15 定义，Task 19、21、22 引用
- 路由名 `Home / Welcome / System / SystemUser / SystemRole / SystemPermission / AccountSettings / Error` 与 spec 3 节完全对齐
