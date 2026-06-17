# 菜单编辑对接 prue_backend 设计

## 目标

将 `frontend/src/views/system/menu/` 改造为对接 `prue_backend` 的版本化菜单编辑器，
替代当前无效的 `/iam/menus/*` 调用，使管理员可通过 UI 创建草稿、编辑路由树、发布菜单版本，
发布后 `/get-async-routes` 自动返回新 active 路由。

## 背景

- 前端菜单管理页（`views/system/menu/`）已存在，但 API 客户端调用的是
  `iam-service` 的 `/iam/menus/*`，而 `iam-service` 当前**未实现**该端点
  （无 `menus` 路由、无 `Menu` 模型、无 `Menu` DTO，AGENTS.md 端点域也未列出）。
- `services/prue_backend/` 已实现 `MenuConfigRow`（`routes_json` 字段），
  提供 `/pure/menu-configs/*` 端点 + `/get-async-routes` 输出 active 路由。
- 两者数据模型不同：
  - `MenuConfigRow`：版本化 JSON 整体（draft / published / archived / is_active）
  - `IamMenu`（前端旧类型）：行级树（id / parent_id / 单条 CRUD）

## 范围

在范围内：

- 新建 `frontend/src/api/menu-config.ts` API 客户端
- 重写 `frontend/src/views/system/menu/index.vue` 为版本列表
- 新建 `frontend/src/views/system/menu/editor.vue` 为树节点 + 节点表单编辑器
- 调整 `frontend/src/router/modules/system.ts` 注册两个路由
- 删除或迁移 `tests/e2e/iam-menus.e2e.test.ts`（指向不存在端点）

不在范围内：

- 新建 iam-service `/iam/menus/*` 端点
- iam-service 内的 `Menu` 模型
- 角色/权限与菜单的绑定
- `getAsyncRoutes` 由 mock 切到 prue_backend（独立任务）
- IAM 用户的菜单权限过滤
- prue_backend 后端代码改动（接口已就绪）

## 架构

```
frontend/src/
  api/menu-config.ts                  # 新建：封装 prue_backend /pure/menu-configs/*
  views/system/menu/
    index.vue                         # 重写：版本列表 + 顶部"新建草稿"
    editor.vue                        # 新增：节点树 + 节点表单
    utils/types.ts                    # 重写：MenuConfig / MenuConfigSummary / RouteNode
    utils/hook.tsx                    # 重写：useMenuConfigList
    utils/editor-hook.tsx             # 新增：useMenuConfigEditor（含未保存守卫）
  router/modules/system.ts            # 调整：注册 /system/menu 列表与 editor
```

后端服务 `prue_backend` 不动。

## 数据契约

### 端点映射

| 前端 API | HTTP | 后端路径 | 备注 |
|----------|------|----------|------|
| `listMenuConfigs()` | GET | `/pure/menu-configs` | 返回 `MenuConfigListResponse` |
| `getMenuConfig(id)` | GET | `/pure/menu-configs/{id}` | 返回 `MenuConfigDetail` |
| `createMenuConfig(req)` | POST | `/pure/menu-configs` | `req.routes_json: RouteNode[]` |
| `updateMenuConfig(id, req)` | PUT | `/pure/menu-configs/{id}` | 仅 draft 可改 |
| `publishMenuConfig(id)` | POST | `/pure/menu-configs/{id}/publish` | 原子激活 |
| `cloneMenuConfig(id)` | POST | `/pure/menu-configs/{id}/clone` | 复制为新 draft |
| `deleteMenuConfig(id)` | DELETE | `/pure/menu-configs/{id}` | 仅 draft / archived 可删 |

`createMenuConfig` 的 `routes_json` 初始化策略：

- 用户勾选"基于当前 active 创建" → 前端先 `GET /get-async-routes` 拿当前 active 路由
  数组作为 `routes_json` 提交
- 否则传 `[]`

### TypeScript 类型

```typescript
// 与 prue_backend/src/dto/menu_config.rs 对齐
export interface MenuConfigSummary {
  id: string;
  version: number;
  name: string;
  description?: string;
  status: "draft" | "published" | "archived";
  is_active: boolean;
  created_by?: string;
  created_at: string;
  updated_at: string;
  published_at?: string;
}

export interface MenuConfigDetail extends MenuConfigSummary {
  routes_json: RouteNode[];
}

export interface RouteNode {
  path: string;
  name?: string;
  redirect?: string;
  component?: string;
  meta?: {
    title?: string;
    icon?: string;
    rank?: number;
    roles?: string[];
    auths?: string[];
    keepAlive?: boolean;
    hiddenTag?: boolean;
    fixedTag?: boolean;
    showLink?: boolean;
    showParent?: boolean;
    activePath?: string;
    frameSrc?: string;
    frameLoading?: boolean;
    enterTransition?: string;
    leaveTransition?: string;
    extraIcon?: string;
  };
  children?: RouteNode[];
}

export interface CreateMenuConfigRequest {
  name: string;
  description?: string;
  routes_json: RouteNode[];
  created_by?: string;
}

export interface UpdateMenuConfigRequest {
  name?: string;
  description?: string;
  routes_json?: RouteNode[];
}
```

## 组件设计

### 列表页 `index.vue`

- 顶部搜索：`name` 模糊匹配
- 顶部按钮：`新建草稿`（弹框：name 必填、description 可选、`基于 active` 复选框）
- 表格列：version、name、description、status、is_active、updated_at、created_by
- 行操作：
  - `编辑`：跳转 `/system/menu/editor?id={id}`，仅 draft 可用
  - `克隆`：弹框确认后调 `cloneMenuConfig(id)`，刷新列表
  - `发布`：弹框确认（"将立即影响线上菜单"）后调 `publishMenuConfig(id)`，刷新列表
  - `删除`：弹框确认，仅 draft/archived 可用

### 编辑器 `editor.vue`

布局：左 40% 树 + 右 60% 表单 + 顶部条 + 底部状态条。

- 顶部条：版本名、version、status、is_active 徽标；
  按钮 `保存草稿` / `发布` / `返回`
- 左侧 `el-tree`：
  - `node-key="path"`（后端 validate 强制 path 唯一）
  - 节点内容：`path`（粗体） + `meta.title`（灰色）
  - 工具按钮：新增子节点 / 新增同级 / 删除当前节点
  - 拖拽排序（draggable），仅在 `status === "draft"` 启用
- 右侧表单（节点未选中时显示提示）：
  - `path`（必填，唯一）
  - `name`（可选，唯一）
  - `redirect`
  - `component`
  - `meta.title`（必填）
  - `meta.icon`（复用 `IconSelect`）
  - `meta.rank`
  - `meta.roles`（多字符串输入）
  - `meta.auths`（多字符串输入）
  - `meta.keepAlive`、`hiddenTag`、`fixedTag`、`showLink`、`showParent`（开关）
  - `meta.frameSrc`、`frameLoading`
  - `meta.enterTransition`、`leaveTransition`、`extraIcon`
  - `meta.activePath`
- 底部状态条：未保存修改计数 / 错误数 / 上次保存时间
- 表单变更时直接修改 `localRoutes` 树中对应节点，**不立即 PUT**
- `保存草稿` 按钮：disabled 当无修改或非 draft 时；点击后 PUT 整个 `routes_json`
- `发布` 按钮：draft 状态才显示；点击后 `publishMenuConfig(id)`，成功后状态变 published

### 未保存守卫

- `useMenuConfigEditor` 维护 `isDirty: ComputedRef<boolean>`（深比较 `localRoutes` 与初始 `routes_json`）
- `onBeforeRouteLeave((to) => { return isDirty.value ? confirmLeaveDialog() : true })`
- `onBeforeUnmount` + `beforeunload` 事件监听，仅在 isDirty 时弹原生确认

### 验证

- 前端在保存前调用与后端一致的本地校验（抽到 `utils/validate-routes.ts`）：
  必填字段、path 唯一、name 唯一、roles/auths 必须为字符串数组
- 提交后端失败时显示后端返回的具体错误（如 `duplicate route.path: /x`）

## 路由调整

在 `router/modules/system.ts` 中 `system` 子路由下新增：

```typescript
{
  path: "/system/menu",
  name: "SystemMenu",
  component: () => import("@/views/system/menu/index.vue"),
  meta: { title: $t("menus.pureSystemMenu"), icon: "ep/menu", keepAlive: true }
},
{
  path: "/system/menu/editor",
  name: "SystemMenuEditor",
  component: () => import("@/views/system/menu/editor.vue"),
  meta: {
    title: "编辑菜单版本",
    keepAlive: false,
    showLink: false,    // 不在侧边栏显示，通过列表页跳转
    activePath: "/system/menu"
  }
}
```

`i18n` 文案 `menus.pureSystemMenu` 已存在；编辑器标题用普通字符串（无侧边栏展示，
无需 i18n）。

## 错误处理

| 场景 | 行为 |
|------|------|
| 非 draft 状态下点保存 | 按钮 disabled + tooltip "只有草稿可保存" |
| 重复 path / name | 前端实时高亮冲突节点；保存时弹错误 |
| 后端返回 4xx | el-message 展示后端 `message` |
| 后端返回 5xx | el-message "服务异常，请稍后重试" |
| 发布后端失败 | 弹框保留，状态不变 |
| 路由离开时 | 弹 el-popconfirm；选"留在当前页"则中止跳转 |

## 测试

### 单元（可选）

- `utils/validate-routes.ts` 导出 `validateRouteTree(nodes): string[]` 返回首个错误信息
- 覆盖用例：path 缺失、path 重复、name 重复、roles 不是字符串数组、children 非数组

### E2E

- 删除/迁移 `tests/e2e/iam-menus.e2e.test.ts`：该文件指向不存在的 `/iam/menus`，
  改为 `tests/e2e/menu-configs.e2e.test.ts`：
  - `GET /pure/menu-configs` → 期望 200
  - `POST /pure/menu-configs` 创建一个 draft
  - `GET /pure/menu-configs/{id}` → routes_json 与提交一致
  - `PUT /pure/menu-configs/{id}` 修改 routes_json
  - `POST /pure/menu-configs/{id}/publish` → 状态变 published、is_active=true
  - `GET /get-async-routes` → 内容与最新 published 一致
  - `POST /pure/menu-configs/{id}/clone` 复制为新 draft
  - `DELETE /pure/menu-configs/{id}` 软删除未发布版本

## 风险与权衡

- **整体重写 UI**：用户从"行级表格"切换到"树节点编辑器"，交互有变化
  → 通过 A1 形态（树 + 表单）尽量降低学习成本
- **保存粒度粗**：整个 `routes_json` 一次性提交，节点级 patch 不支持
  → 暂不实现 YAGNI；后端 validate 已经覆盖整体一致性
- **草稿并发**：两个管理员同时编辑同一 draft，后保存者覆盖前者
  → 暂不实现；后续如出现冲突可加 `updated_at` 比对
- **路由树体积**：当菜单量大时单次 PUT 较大
  → 当前菜单规模小，不分片

## 验收标准

1. 在 `views/system/menu/index.vue` 可看到 prue_backend 中所有菜单配置版本
2. 可创建新草稿（可勾选"基于 active"）
3. 编辑器可新增 / 编辑 / 删除 / 拖拽排序节点，并保存为草稿
4. 发布后再次进入编辑器显示 published 状态，`保存草稿` 按钮 disabled
5. 关闭浏览器或离开页面在有未保存修改时弹确认
6. E2E `menu-configs.e2e.test.ts` 全部通过
7. 旧的 `iam-menus.e2e.test.ts` 已删除或迁移
