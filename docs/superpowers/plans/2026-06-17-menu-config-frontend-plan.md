# 菜单配置前端适配 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 保留现有菜单管理 UI，仅把菜单数据读写切换到 `prue_backend` 的 `/pure/menu-configs/*` 版本化 JSON 接口。

**Architecture:** `index.vue` 只增加版本下拉、草稿、发布按钮；`hook.tsx` 负责版本选择、加载、保存、删除、发布；`menu-adapter.ts` 负责 `FormItemProps[]` 与 `routes_json` 双向转换。后端保存完整 `routes_json`，前端通过 `_menu` 兼容字段保留现有 UI 所需的 `id`、`parentId`、`menuType`。

**Tech Stack:** Vue 3、TypeScript、Element Plus、vue-pure-admin HTTP 工具、Vitest、现有 E2E supertest。

---

## 文件结构

### 新增

- `frontend/src/api/menu-config.ts`：封装 `prue_backend` 菜单配置接口。
- `frontend/src/views/system/menu/utils/menu-adapter.ts`：菜单 UI 数据与 `routes_json` 的双向转换。
- `frontend/src/views/system/menu/utils/menu-adapter.test.ts`：转换逻辑单元测试。
- `tests/e2e/menu-configs.e2e.test.ts`：后端菜单配置链路 E2E。

### 修改

- `frontend/package.json`：增加 `test:unit:menu` 脚本和 Vitest 开发依赖。
- `frontend/src/views/system/menu/utils/types.ts`：补充 `id`、`children`、菜单配置版本类型、路由 JSON 类型。
- `frontend/src/views/system/menu/utils/hook.tsx`：从 mock 菜单切换到 `prue_backend`，实现版本选择、保存、删除、发布。
- `frontend/src/views/system/menu/index.vue`：增加版本下拉、新建草稿、发布按钮，禁用非 draft 编辑。
- `tests/e2e/package.json`：增加 `test:menu-configs` 脚本。
- `tests/e2e/iam-menus.e2e.test.ts`：删除旧 IAM menus 测试或改为空文件删除。

---

## Task 1: 前端单元测试入口

**Files:**
- Modify: `frontend/package.json`

- [ ] **Step 1: 增加菜单适配器测试脚本**

在 `frontend/package.json` 的 `scripts` 中追加：

```json
"test:unit:menu": "vitest run src/views/system/menu/utils/menu-adapter.test.ts"
```

把 `devDependencies` 中追加：

```json
"vitest": "^2.1.9"
```

- [ ] **Step 2: 安装依赖**

Run:

```powershell
pnpm -C frontend install
```

Expected:

```text
Done
```

如果 lockfile 有更新，保留 `pnpm-lock.yaml` 的变更。

- [ ] **Step 3: 验证空测试文件不存在时失败**

Run:

```powershell
pnpm -C frontend test:unit:menu
```

Expected:

```text
No test files found
```

---

## Task 2: 写菜单适配器失败测试

**Files:**
- Create: `frontend/src/views/system/menu/utils/menu-adapter.test.ts`

- [ ] **Step 1: 写转换测试**

写入 `frontend/src/views/system/menu/utils/menu-adapter.test.ts`：

```typescript
import { describe, expect, it } from "vitest";
import {
  formItemsToRoutes,
  routesToFormItems,
  type RouteNode
} from "./menu-adapter";

const routeTree: RouteNode[] = [
  {
    path: "/system",
    name: "System",
    component: "LAYOUT",
    redirect: "/system/menu",
    meta: {
      title: "系统管理",
      icon: "ep/setting",
      rank: 1,
      auths: ["system:view"],
      showLink: true,
      showParent: true
    },
    _menu: {
      id: 1,
      parentId: 0,
      menuType: 0,
      auths: "system:view"
    },
    children: [
      {
        path: "/system/menu",
        name: "SystemMenu",
        component: "system/menu/index",
        meta: {
          title: "菜单管理",
          icon: "ep/menu",
          rank: 2,
          auths: ["system:menu:list"],
          showLink: true,
          showParent: false
        },
        _menu: {
          id: 2,
          parentId: 1,
          menuType: 0,
          auths: "system:menu:list"
        }
      }
    ]
  }
];

describe("menu-adapter", () => {
  it("converts routes_json to flat menu items", () => {
    const items = routesToFormItems(routeTree);

    expect(items).toHaveLength(2);
    expect(items[0]).toMatchObject({
      id: 1,
      parentId: 0,
      menuType: 0,
      title: "系统管理",
      name: "System",
      path: "/system",
      component: "LAYOUT",
      rank: 1,
      auths: "system:view"
    });
    expect(items[1]).toMatchObject({
      id: 2,
      parentId: 1,
      menuType: 0,
      title: "菜单管理",
      name: "SystemMenu",
      path: "/system/menu",
      component: "system/menu/index",
      rank: 2,
      auths: "system:menu:list"
    });
  });

  it("converts flat menu items back to routes_json with _menu", () => {
    const items = routesToFormItems(routeTree);
    const routes = formItemsToRoutes(items);

    expect(routes).toEqual(routeTree);
  });

  it("generates id and parentId when _menu is missing", () => {
    const items = routesToFormItems([
      {
        path: "/dashboard",
        name: "Dashboard",
        component: "dashboard/index",
        meta: { title: "首页" },
        children: [
          {
            path: "/dashboard/workbench",
            name: "Workbench",
            component: "dashboard/workbench",
            meta: { title: "工作台" }
          }
        ]
      }
    ]);

    expect(items).toHaveLength(2);
    expect(typeof items[0].id).toBe("number");
    expect(items[0].parentId).toBe(0);
    expect(items[1].parentId).toBe(items[0].id);
  });

  it("converts comma separated auths to meta auths", () => {
    const routes = formItemsToRoutes([
      {
        id: 10,
        menuType: 3,
        higherMenuOptions: [],
        parentId: 0,
        title: "按钮",
        name: "ButtonAuth",
        path: "/button-auth",
        component: "",
        rank: 99,
        redirect: "",
        icon: "",
        extraIcon: "",
        enterTransition: "",
        leaveTransition: "",
        activePath: "",
        auths: "a,b, c ",
        frameSrc: "",
        frameLoading: true,
        keepAlive: false,
        hiddenTag: false,
        fixedTag: false,
        showLink: true,
        showParent: false
      }
    ]);

    expect(routes[0].meta?.auths).toEqual(["a", "b", "c"]);
    expect(routes[0]._menu?.auths).toBe("a,b, c ");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```powershell
pnpm -C frontend test:unit:menu
```

Expected:

```text
Failed to resolve import "./menu-adapter"
```

---

## Task 3: 实现类型与菜单适配器

**Files:**
- Modify: `frontend/src/views/system/menu/utils/types.ts`
- Create: `frontend/src/views/system/menu/utils/menu-adapter.ts`

- [ ] **Step 1: 扩展菜单类型**

将 `frontend/src/views/system/menu/utils/types.ts` 替换为：

```typescript
interface FormItemProps {
  id?: number;
  /** 菜单类型（0代表菜单、1代表iframe、2代表外链、3代表按钮）*/
  menuType: number;
  higherMenuOptions: Record<string, unknown>[];
  parentId: number;
  title: string;
  name: string;
  path: string;
  component: string;
  rank: number;
  redirect: string;
  icon: string;
  extraIcon: string;
  enterTransition: string;
  leaveTransition: string;
  activePath: string;
  auths: string;
  frameSrc: string;
  frameLoading: boolean;
  keepAlive: boolean;
  hiddenTag: boolean;
  fixedTag: boolean;
  showLink: boolean;
  showParent: boolean;
  children?: FormItemProps[];
}

interface FormProps {
  formInline: FormItemProps;
}

interface MenuConfigSummary {
  id: string;
  version: number;
  name: string;
  description?: string | null;
  status: "draft" | "published" | "archived" | string;
  is_active: boolean;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
  published_at?: string | null;
}

interface MenuConfigDetail extends MenuConfigSummary {
  routes_json: unknown[];
}

interface MenuConfigListResponse {
  items: MenuConfigSummary[];
  total: number;
}

export type {
  FormItemProps,
  FormProps,
  MenuConfigSummary,
  MenuConfigDetail,
  MenuConfigListResponse
};
```

- [ ] **Step 2: 实现 menu-adapter**

写入 `frontend/src/views/system/menu/utils/menu-adapter.ts`：

```typescript
import type { FormItemProps } from "./types";

export interface RouteMeta {
  title?: string;
  icon?: string;
  extraIcon?: string;
  rank?: number;
  roles?: string[];
  auths?: string[];
  enterTransition?: string;
  leaveTransition?: string;
  activePath?: string;
  frameSrc?: string;
  frameLoading?: boolean;
  keepAlive?: boolean;
  hiddenTag?: boolean;
  fixedTag?: boolean;
  showLink?: boolean;
  showParent?: boolean;
  [key: string]: unknown;
}

export interface RouteMenuCompat {
  id?: number;
  parentId?: number;
  menuType?: number;
  auths?: string;
}

export interface RouteNode {
  path: string;
  name?: string;
  redirect?: string;
  component?: string;
  meta?: RouteMeta;
  children?: RouteNode[];
  _menu?: RouteMenuCompat;
  [key: string]: unknown;
}

function hashId(input: string): number {
  let hash = 0;
  const text = input || `menu_${Date.now()}`;
  for (let index = 0; index < text.length; index += 1) {
    hash = (hash << 5) - hash + text.charCodeAt(index);
    hash |= 0;
  }
  return Math.abs(hash) + 1;
}

function splitAuths(auths: string): string[] | undefined {
  const values = auths
    .split(",")
    .map(item => item.trim())
    .filter(Boolean);
  return values.length > 0 ? values : undefined;
}

function inferMenuType(route: RouteNode): number {
  if (route._menu?.menuType !== undefined) return route._menu.menuType;
  if (route.meta?.frameSrc) return 1;
  if (/^https?:\/\//.test(route.path)) return 2;
  return 0;
}

function toFormItem(route: RouteNode, parentId: number): FormItemProps[] {
  const meta = route.meta ?? {};
  const id = route._menu?.id ?? hashId(`${parentId}:${route.path}:${route.name ?? ""}`);
  const auths = route._menu?.auths ?? meta.auths?.join(",") ?? "";

  const current: FormItemProps = {
    id,
    menuType: inferMenuType(route),
    higherMenuOptions: [],
    parentId: route._menu?.parentId ?? parentId,
    title: meta.title ?? "",
    name: route.name ?? "",
    path: route.path ?? "",
    component: route.component ?? "",
    rank: meta.rank ?? 99,
    redirect: route.redirect ?? "",
    icon: meta.icon ?? "",
    extraIcon: meta.extraIcon ?? "",
    enterTransition: meta.enterTransition ?? "",
    leaveTransition: meta.leaveTransition ?? "",
    activePath: meta.activePath ?? "",
    auths,
    frameSrc: meta.frameSrc ?? "",
    frameLoading: meta.frameLoading ?? true,
    keepAlive: meta.keepAlive ?? false,
    hiddenTag: meta.hiddenTag ?? false,
    fixedTag: meta.fixedTag ?? false,
    showLink: meta.showLink ?? true,
    showParent: meta.showParent ?? false
  };

  const children = route.children?.flatMap(child => toFormItem(child, id)) ?? [];
  return [current, ...children];
}

export function routesToFormItems(routes: RouteNode[]): FormItemProps[] {
  return routes.flatMap(route => toFormItem(route, 0));
}

function itemToRoute(item: FormItemProps, children: RouteNode[]): RouteNode {
  const meta: RouteMeta = {
    title: item.title,
    icon: item.icon,
    extraIcon: item.extraIcon,
    rank: item.rank,
    enterTransition: item.enterTransition,
    leaveTransition: item.leaveTransition,
    activePath: item.activePath,
    frameSrc: item.frameSrc,
    frameLoading: item.frameLoading,
    keepAlive: item.keepAlive,
    hiddenTag: item.hiddenTag,
    fixedTag: item.fixedTag,
    showLink: item.showLink,
    showParent: item.showParent
  };

  const auths = splitAuths(item.auths);
  if (auths) meta.auths = auths;

  Object.keys(meta).forEach(key => {
    if (meta[key] === "" || meta[key] === undefined) delete meta[key];
  });

  const route: RouteNode = {
    path: item.path,
    name: item.name,
    component: item.component,
    meta,
    _menu: {
      id: item.id,
      parentId: item.parentId,
      menuType: item.menuType,
      auths: item.auths
    }
  };

  if (item.redirect) route.redirect = item.redirect;
  if (children.length > 0) route.children = children;

  return route;
}

function buildRoutes(items: FormItemProps[], parentId: number): RouteNode[] {
  return items
    .filter(item => (item.parentId ?? 0) === parentId)
    .sort((left, right) => (left.rank ?? 99) - (right.rank ?? 99))
    .map(item => itemToRoute(item, buildRoutes(items, item.id ?? 0)));
}

export function formItemsToRoutes(items: FormItemProps[]): RouteNode[] {
  return buildRoutes(items, 0);
}
```

- [ ] **Step 3: 运行单元测试**

Run:

```powershell
pnpm -C frontend test:unit:menu
```

Expected:

```text
4 passed
```

---

## Task 4: 新增菜单配置 API 客户端

**Files:**
- Create: `frontend/src/api/menu-config.ts`

- [ ] **Step 1: 写 API 客户端**

写入 `frontend/src/api/menu-config.ts`：

```typescript
import { http } from "@/utils/http";
import type {
  MenuConfigDetail,
  MenuConfigListResponse
} from "@/views/system/menu/utils/types";

interface ApiResponse<T> {
  code: number;
  message: string;
  data: T;
}

interface CreateMenuConfigRequest {
  name: string;
  description?: string;
  routes_json: unknown[];
  created_by?: string;
}

interface UpdateMenuConfigRequest {
  name?: string;
  description?: string;
  routes_json?: unknown[];
}

export const listMenuConfigs = () => {
  return http.request<ApiResponse<MenuConfigListResponse>>(
    "get",
    "/pure/menu-configs"
  );
};

export const createMenuConfig = (data: CreateMenuConfigRequest) => {
  return http.request<ApiResponse<MenuConfigDetail>>("post", "/pure/menu-configs", {
    data
  });
};

export const getMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "get",
    `/pure/menu-configs/${id}`
  );
};

export const updateMenuConfig = (id: string, data: UpdateMenuConfigRequest) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "put",
    `/pure/menu-configs/${id}`,
    { data }
  );
};

export const publishMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "post",
    `/pure/menu-configs/${id}/publish`
  );
};

export const cloneMenuConfig = (id: string) => {
  return http.request<ApiResponse<MenuConfigDetail>>(
    "post",
    `/pure/menu-configs/${id}/clone`
  );
};

export const deleteMenuConfig = (id: string) => {
  return http.request<void>("delete", `/pure/menu-configs/${id}`);
};
```

- [ ] **Step 2: 运行类型检查**

Run:

```powershell
pnpm -C frontend typecheck
```

Expected:

```text
No errors
```

---

## Task 5: 重写 hook 数据流

**Files:**
- Modify: `frontend/src/views/system/menu/utils/hook.tsx`

- [ ] **Step 1: 替换导入**

在 `hook.tsx` 删除：

```typescript
import { getMenuList } from "@/api/system";
```

新增：

```typescript
import {
  cloneMenuConfig,
  createMenuConfig,
  getMenuConfig,
  listMenuConfigs,
  publishMenuConfig,
  updateMenuConfig
} from "@/api/menu-config";
import { formItemsToRoutes, routesToFormItems } from "./menu-adapter";
import type { MenuConfigSummary } from "../utils/types";
```

- [ ] **Step 2: 增加版本状态**

在 `loading` 后加入：

```typescript
const menuConfigs = ref<MenuConfigSummary[]>([]);
const selectedConfigId = ref("");
const currentConfig = ref<MenuConfigSummary | null>(null);
const flatMenuList = ref<FormItemProps[]>([]);

const isEditable = computed(() => currentConfig.value?.status === "draft");
```

把 Vue 导入改为：

```typescript
import { reactive, ref, onMounted, h, computed } from "vue";
```

- [ ] **Step 3: 新增版本加载函数**

在 `resetForm` 后加入：

```typescript
async function loadConfigList() {
  const { code, data } = await listMenuConfigs();
  if (code !== 0) {
    menuConfigs.value = [];
    return;
  }

  menuConfigs.value = data.items;
  const preferred =
    data.items.find(item => item.status === "draft") ??
    data.items.find(item => item.is_active) ??
    data.items[0];

  if (preferred) {
    selectedConfigId.value = preferred.id;
    await loadSelectedConfig(preferred.id);
    return;
  }

  const created = await createMenuConfig({
    name: `菜单草稿 ${new Date().toLocaleString()}`,
    description: "前端菜单管理自动创建",
    routes_json: []
  });
  selectedConfigId.value = created.data.id;
  await loadConfigList();
}

async function loadSelectedConfig(id = selectedConfigId.value) {
  if (!id) return;
  loading.value = true;
  try {
    const { code, data } = await getMenuConfig(id);
    if (code !== 0) return;
    currentConfig.value = data;
    selectedConfigId.value = data.id;
    flatMenuList.value = routesToFormItems(data.routes_json as any[]);
    onSearch();
  } finally {
    loading.value = false;
  }
}
```

- [ ] **Step 4: 改造 onSearch**

把 `onSearch` 替换为：

```typescript
async function onSearch() {
  loading.value = true;
  let newData = [...flatMenuList.value];
  if (!isAllEmpty(form.title)) {
    newData = newData.filter(item =>
      transformI18n(item.title).includes(form.title)
    );
  }
  dataList.value = handleTree(newData);
  setTimeout(() => {
    loading.value = false;
  }, 200);
}
```

- [ ] **Step 5: 增加保存、删除辅助函数**

在 `formatHigherMenuOptions` 前加入：

```typescript
function nextMenuId() {
  const maxId = flatMenuList.value.reduce(
    (max, item) => Math.max(max, Number(item.id ?? 0)),
    0
  );
  return maxId + 1;
}

async function saveCurrentMenus() {
  if (!selectedConfigId.value || !isEditable.value) {
    message("只允许编辑草稿版本", { type: "warning" });
    return false;
  }

  const routes_json = formItemsToRoutes(flatMenuList.value);
  const { code, data, message: msg } = await updateMenuConfig(
    selectedConfigId.value,
    { routes_json }
  );

  if (code !== 0) {
    message(msg || "保存菜单失败", { type: "error" });
    return false;
  }

  currentConfig.value = data;
  flatMenuList.value = routesToFormItems(data.routes_json as any[]);
  await onSearch();
  return true;
}

function removeMenuAndChildren(id: number) {
  const removing = new Set<number>([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const item of flatMenuList.value) {
      const itemId = Number(item.id ?? 0);
      if (!removing.has(itemId) && removing.has(Number(item.parentId ?? 0))) {
        removing.add(itemId);
        changed = true;
      }
    }
  }
  flatMenuList.value = flatMenuList.value.filter(
    item => !removing.has(Number(item.id ?? 0))
  );
}
```

- [ ] **Step 6: 改造 openDialog 保存逻辑**

在 `beforeSure` 内，把 `chores` 函数替换为异步保存：

```typescript
async function chores() {
  if (!isEditable.value) {
    message("只允许编辑草稿版本", { type: "warning" });
    return;
  }

  if (title === "新增") {
    flatMenuList.value.push({
      ...curData,
      id: nextMenuId(),
      higherMenuOptions: []
    });
  } else {
    const index = flatMenuList.value.findIndex(item => item.id === row?.id);
    if (index >= 0) {
      flatMenuList.value[index] = {
        ...flatMenuList.value[index],
        ...curData,
        id: row?.id,
        higherMenuOptions: []
      };
    }
  }

  const saved = await saveCurrentMenus();
  if (!saved) return;

  message(`您${title}了菜单名称为${transformI18n(curData.title)}的这条数据`, {
    type: "success"
  });
  done();
}
```

把 `FormRef.validate` 内的成功分支替换为：

```typescript
if (valid) {
  chores();
}
```

- [ ] **Step 7: 改造删除逻辑**

把 `handleDelete` 替换为：

```typescript
async function handleDelete(row) {
  if (!isEditable.value) {
    message("只允许编辑草稿版本", { type: "warning" });
    return;
  }
  removeMenuAndChildren(Number(row.id));
  const saved = await saveCurrentMenus();
  if (!saved) return;
  message(`您删除了菜单名称为${transformI18n(row.title)}的这条数据`, {
    type: "success"
  });
}
```

- [ ] **Step 8: 增加草稿和发布函数**

在 `handleDelete` 后加入：

```typescript
async function createDraftFromCurrent() {
  if (selectedConfigId.value) {
    const { code, data, message: msg } = await cloneMenuConfig(selectedConfigId.value);
    if (code !== 0) {
      message(msg || "新建草稿失败", { type: "error" });
      return;
    }
    selectedConfigId.value = data.id;
    await loadConfigList();
    await loadSelectedConfig(data.id);
    return;
  }

  const created = await createMenuConfig({
    name: `菜单草稿 ${new Date().toLocaleString()}`,
    description: "前端菜单管理创建",
    routes_json: []
  });
  selectedConfigId.value = created.data.id;
  await loadConfigList();
  await loadSelectedConfig(created.data.id);
}

async function publishCurrentConfig() {
  if (!selectedConfigId.value || !isEditable.value) return;
  const { code, data, message: msg } = await publishMenuConfig(selectedConfigId.value);
  if (code !== 0) {
    message(msg || "发布菜单版本失败", { type: "error" });
    return;
  }
  message("发布成功", { type: "success" });
  currentConfig.value = data;
  await loadConfigList();
  await loadSelectedConfig(data.id);
}
```

- [ ] **Step 9: 修改 mounted 与 return**

把 `onMounted` 改为：

```typescript
onMounted(() => {
  loadConfigList();
});
```

在 `return` 中加入：

```typescript
menuConfigs,
selectedConfigId,
currentConfig,
isEditable,
loadSelectedConfig,
createDraftFromCurrent,
publishCurrentConfig,
```

- [ ] **Step 10: 运行类型检查**

Run:

```powershell
pnpm -C frontend typecheck
```

Expected:

```text
No errors
```

---

## Task 6: 更新现有菜单页面版本控件

**Files:**
- Modify: `frontend/src/views/system/menu/index.vue`

- [ ] **Step 1: 接收 hook 新状态**

把 `useMenu()` 解构改为：

```typescript
const {
  form,
  loading,
  columns,
  dataList,
  menuConfigs,
  selectedConfigId,
  currentConfig,
  isEditable,
  onSearch,
  resetForm,
  openDialog,
  handleDelete,
  handleSelectionChange,
  loadSelectedConfig,
  createDraftFromCurrent,
  publishCurrentConfig
} = useMenu();
```

- [ ] **Step 2: 增加版本下拉框**

在菜单名称 `el-form-item` 后插入：

```vue
<el-form-item label="菜单版本：">
  <el-select
    v-model="selectedConfigId"
    class="w-70!"
    placeholder="请选择菜单版本"
    @change="loadSelectedConfig"
  >
    <el-option
      v-for="item in menuConfigs"
      :key="item.id"
      :label="`v${item.version} - ${item.name} - ${item.status}${item.is_active ? ' - active' : ''}`"
      :value="item.id"
    />
  </el-select>
</el-form-item>
```

- [ ] **Step 3: 改表格标题**

把 `PureTableBar` 的标题改为：

```vue
:title="`菜单管理${currentConfig ? `（v${currentConfig.version} ${currentConfig.status}）` : ''}`"
```

- [ ] **Step 4: 增加草稿与发布按钮**

在 `新增菜单` 按钮前插入：

```vue
<el-button :loading="loading" @click="createDraftFromCurrent">
  新建草稿
</el-button>
<el-button
  v-if="isEditable"
  type="success"
  :loading="loading"
  @click="publishCurrentConfig"
>
  发布当前版本
</el-button>
```

- [ ] **Step 5: 禁用非草稿编辑按钮**

给 `新增菜单` 按钮增加：

```vue
:disabled="!isEditable"
```

给操作列三个按钮分别增加：

```vue
:disabled="!isEditable"
```

- [ ] **Step 6: 运行类型检查**

Run:

```powershell
pnpm -C frontend typecheck
```

Expected:

```text
No errors
```

---

## Task 7: 后端菜单配置 E2E

**Files:**
- Create: `tests/e2e/menu-configs.e2e.test.ts`
- Modify: `tests/e2e/package.json`
- Delete: `tests/e2e/iam-menus.e2e.test.ts`

- [ ] **Step 1: 增加 E2E 脚本**

在 `tests/e2e/package.json` 的 `scripts` 中加入：

```json
"test:menu-configs": "vitest run menu-configs"
```

- [ ] **Step 2: 写 menu-configs E2E**

写入 `tests/e2e/menu-configs.e2e.test.ts`：

```typescript
import { describe, expect, it } from 'vitest';
import { authedReq } from './helpers/http';

const adminToken = (globalThis as unknown as { __adminToken__: string })
  .__adminToken__;

const OK = [200, 201];

const routesJson = [
  {
    path: `/e2e-menu-${Date.now()}`,
    name: `E2eMenu${Date.now()}`,
    component: 'system/menu/index',
    meta: {
      title: 'E2E 菜单',
      auths: ['e2e:menu:list'],
      rank: 99,
    },
    _menu: {
      id: 900001,
      parentId: 0,
      menuType: 0,
      auths: 'e2e:menu:list',
    },
  },
];

describe('menu configs', () => {
  let configId: string | undefined;

  it('should list menu configs', async () => {
    const res = await authedReq(adminToken).get('/pure/menu-configs');
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data).toHaveProperty('items');
  });

  it('should create a draft menu config', async () => {
    const res = await authedReq(adminToken).post('/pure/menu-configs', {
      name: `e2e menu config ${Date.now()}`,
      description: 'created by e2e',
      routes_json: routesJson,
      created_by: 'e2e',
    });

    expect(OK).toContain(res.status);
    expect(res.body.code).toBe(0);
    expect(res.body.data.status).toBe('draft');
    expect(res.body.data.routes_json).toEqual(routesJson);
    configId = res.body.data.id;
  });

  it('should get menu config detail', async () => {
    if (!configId) return;
    const res = await authedReq(adminToken).get(`/pure/menu-configs/${configId}`);
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.id).toBe(configId);
  });

  it('should update routes_json', async () => {
    if (!configId) return;
    const updatedRoutes = [
      {
        ...routesJson[0],
        meta: {
          ...routesJson[0].meta,
          title: 'E2E 菜单已更新',
        },
      },
    ];

    const res = await authedReq(adminToken).put(`/pure/menu-configs/${configId}`, {
      routes_json: updatedRoutes,
    });

    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.routes_json).toEqual(updatedRoutes);
  });

  it('should publish menu config', async () => {
    if (!configId) return;
    const res = await authedReq(adminToken).post(`/pure/menu-configs/${configId}/publish`);
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(res.body.data.status).toBe('published');
    expect(res.body.data.is_active).toBe(true);
  });

  it('should expose published routes from get-async-routes', async () => {
    const res = await authedReq(adminToken).get('/get-async-routes');
    expect(res.status).toBe(200);
    expect(res.body.code).toBe(0);
    expect(Array.isArray(res.body.data)).toBe(true);
    expect(JSON.stringify(res.body.data)).toContain('E2E 菜单已更新');
  });
});
```

- [ ] **Step 3: 删除旧 IAM menus 测试**

删除文件：

```text
tests/e2e/iam-menus.e2e.test.ts
```

- [ ] **Step 4: 运行 E2E**

Run:

```powershell
pnpm -C tests/e2e test:menu-configs
```

Expected:

```text
6 passed
```

---

## Task 8: 最终校验

**Files:**
- All changed files

- [ ] **Step 1: 前端单元测试**

Run:

```powershell
pnpm -C frontend test:unit:menu
```

Expected:

```text
4 passed
```

- [ ] **Step 2: 前端类型检查**

Run:

```powershell
pnpm -C frontend typecheck
```

Expected:

```text
No errors
```

- [ ] **Step 3: 前端 lint**

Run:

```powershell
pnpm -C frontend lint:eslint
```

Expected:

```text
No warnings
```

- [ ] **Step 4: 菜单配置 E2E**

Run:

```powershell
pnpm -C tests/e2e test:menu-configs
```

Expected:

```text
6 passed
```

- [ ] **Step 5: 查看 git diff**

Run:

```powershell
git diff --check
```

Expected:

```text
无 trailing whitespace 错误
```

- [ ] **Step 6: 提交变更**

仅当用户明确要求提交时执行：

```powershell
git add frontend/package.json frontend/src/api/menu-config.ts frontend/src/views/system/menu/index.vue frontend/src/views/system/menu/utils/types.ts frontend/src/views/system/menu/utils/hook.tsx frontend/src/views/system/menu/utils/menu-adapter.ts frontend/src/views/system/menu/utils/menu-adapter.test.ts tests/e2e/package.json tests/e2e/menu-configs.e2e.test.ts tests/e2e/iam-menus.e2e.test.ts pnpm-lock.yaml
git commit -m "feat(frontend): wire menu editor to menu configs"
```

---

## Self-review

- Spec coverage: 覆盖版本下拉、保留现有 UI、API 客户端、双向转换、草稿保存、发布、E2E、单元测试。
- Placeholder scan: 无 `TBD`、无未定义占位步骤、无空泛错误处理步骤。
- Type consistency: `MenuConfigSummary`、`MenuConfigDetail`、`FormItemProps`、`RouteNode` 在任务中统一。
- Scope check: 只改前端菜单适配和对应 E2E，不改后端结构，不做并发控制，不做 sanitize 输出。
