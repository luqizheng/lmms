# 菜单编辑对接 prue_backend 设计

## 目标

保留 `frontend/src/views/system/menu/` 现有菜单管理体验，只把后端访问改为 `prue_backend` 的菜单配置接口。

前端仍然使用当前表格树、弹窗表单、新增、修改、删除交互；后端仍以 `MenuConfigRow.routes_json` JSON 格式保存菜单。

## 核心约束

- 不新建独立编辑器页面。
- 不拆分现有 `form.vue`。
- 不改变现有 input 表单编辑方式。
- 不改变现有表格树展示方式。
- 允许在现有页面顶部增加版本下拉框。
- 后端接口使用 `prue_backend`：`/pure/menu-configs/*`。
- 保存时整体更新当前版本的 `routes_json`。

## 背景

现有菜单 UI 位于：

- `frontend/src/views/system/menu/index.vue`
- `frontend/src/views/system/menu/form.vue`
- `frontend/src/views/system/menu/utils/hook.tsx`
- `frontend/src/views/system/menu/utils/types.ts`

当前页面逻辑是纯演示：

- `index.vue` 展示树表格。
- `form.vue` 负责菜单属性编辑。
- `hook.tsx` 调用 `getMenuList()` 加载 mock 菜单。
- 新增、修改、删除只弹成功提示，没有真实保存。

后端 `prue_backend` 已有：

- `GET /pure/menu-configs`
- `POST /pure/menu-configs`
- `GET /pure/menu-configs/{id}`
- `PUT /pure/menu-configs/{id}`
- `POST /pure/menu-configs/{id}/publish`
- `POST /pure/menu-configs/{id}/clone`
- `DELETE /pure/menu-configs/{id}`
- `GET /get-async-routes`

后端保存结构是版本化 `routes_json`，不是行级菜单表。

## 设计总览

```
现有菜单页面
  index.vue
    ├─ 版本下拉框
    ├─ 树表格
    └─ 新增/修改/删除按钮

  form.vue
    └─ 原 input 表单不变

  hook.tsx
    ├─ 加载版本列表
    ├─ 加载当前版本 routes_json
    ├─ routes_json -> FormItemProps[]
    ├─ FormItemProps[] -> routes_json
    ├─ 新增/修改/删除本地菜单数组
    └─ PUT 保存到当前版本

prue_backend
  MenuConfigRow.routes_json
```

## 文件改动

| 文件 | 动作 |
|---|---|
| `frontend/src/api/menu-config.ts` | 新建，封装 `/pure/menu-configs/*` |
| `frontend/src/views/system/menu/index.vue` | 小改，增加版本下拉框与可选发布按钮 |
| `frontend/src/views/system/menu/utils/hook.tsx` | 重写数据加载、版本切换、保存逻辑 |
| `frontend/src/views/system/menu/utils/types.ts` | 扩展版本与 routes_json 类型 |
| `frontend/src/views/system/menu/utils/menu-adapter.ts` | 新建，负责双向转换 |
| `frontend/src/views/system/menu/form.vue` | 不改或仅做兼容性小修 |
| `frontend/src/api/system.ts` | 不再新增菜单真实接口，旧 `getMenuList` 可保留给其他 mock 页面 |
| `tests/e2e/iam-menus.e2e.test.ts` | 迁移为 menu-configs 端点测试 |

## 页面交互

### 版本下拉框

在现有搜索区域增加：

- `菜单版本` 下拉框
- 显示格式：`v{version} - {name} - {status}`
- active 版本加标签
- draft 版本可编辑
- published / archived 版本只读

切换版本流程：

```
选择版本
  |
  v
GET /pure/menu-configs/{id}
  |
  v
routes_json -> flat menu list
  |
  v
handleTree(flat list)
  |
  v
刷新现有表格树
```

### 新建草稿

保留现有按钮区域，增加 `新建草稿`：

- 默认基于当前选中版本 clone。
- 调 `POST /pure/menu-configs/{id}/clone`。
- clone 成功后自动选中新草稿。

如果当前没有任何版本：

- 调 `POST /pure/menu-configs`。
- `routes_json = []`。

### 新增 / 修改 / 删除菜单

保留现有弹窗逻辑。

保存流程改为：

```
弹窗确认
  |
  v
更新本地 flat menu list
  |
  v
flat menu list -> routes_json
  |
  v
PUT /pure/menu-configs/{selectedVersionId}
  |
  v
重新加载当前版本
```

删除流程：

```
确认删除
  |
  v
删除当前节点及子节点
  |
  v
flat menu list -> routes_json
  |
  v
PUT /pure/menu-configs/{selectedVersionId}
```

### 发布

可在版本下拉旁增加 `发布当前版本` 按钮。

规则：

- 仅 draft 状态显示。
- 点击后 `POST /pure/menu-configs/{id}/publish`。
- 成功后刷新版本列表与当前数据。

## 数据转换

### FormItemProps -> RouteNode

| 前端字段 | routes_json 字段 |
|---|---|
| `title` | `meta.title` |
| `name` | `name` |
| `path` | `path` |
| `component` | `component` |
| `rank` | `meta.rank` |
| `redirect` | `redirect` |
| `icon` | `meta.icon` |
| `extraIcon` | `meta.extraIcon` |
| `enterTransition` | `meta.enterTransition` |
| `leaveTransition` | `meta.leaveTransition` |
| `activePath` | `meta.activePath` |
| `frameSrc` | `meta.frameSrc` |
| `frameLoading` | `meta.frameLoading` |
| `keepAlive` | `meta.keepAlive` |
| `hiddenTag` | `meta.hiddenTag` |
| `fixedTag` | `meta.fixedTag` |
| `showLink` | `meta.showLink` |
| `showParent` | `meta.showParent` |
| `auths` | `meta.auths` |
| `children` | `children` |

### 兼容字段 `_menu`

为保持现有 UI 的 `id`、`parentId`、`menuType` 可无损往返，在 `routes_json` 中保存前端兼容字段：

```json
{
  "path": "/system/menu",
  "name": "SystemMenu",
  "component": "system/menu/index",
  "meta": {
    "title": "菜单管理",
    "icon": "ep/menu",
    "rank": 10
  },
  "_menu": {
    "id": 100,
    "parentId": 0,
    "menuType": 0,
    "auths": "system:menu:list"
  }
}
```

说明：

- `_menu` 仅给前端菜单管理页使用。
- 后端 `validate_routes_json` 不禁止额外字段。
- 动态路由消费方会忽略未知字段。
- 如果后续要清理，可在 `/get-async-routes` 输出前做 sanitize，当前不做。

### RouteNode -> FormItemProps

转换时优先读取 `_menu`：

- `id = route._menu.id || generatedId(route.path, route.name)`
- `parentId = route._menu.parentId || parent id from traversal`
- `menuType = route._menu.menuType || inferMenuType(route)`
- `auths = route._menu.auths || route.meta.auths?.join(',') || ''`

缺失 `_menu` 时，通过层级遍历生成 `parentId`。

## 版本策略

### 页面初始化

```
GET /pure/menu-configs
  |
  v
优先选择 draft
  |
  +-- 没有 draft，选择 active published
  |
  +-- 都没有，创建空 draft
  |
  v
加载选中版本详情
```

### 编辑权限

| 版本状态 | 菜单新增 | 菜单修改 | 菜单删除 | 发布 |
|---|---:|---:|---:|---:|
| draft | 可用 | 可用 | 可用 | 可用 |
| published | 禁用 | 禁用 | 禁用 | 禁用 |
| archived | 禁用 | 禁用 | 禁用 | 禁用 |

只读状态下：

- 新增按钮 disabled。
- 修改按钮 disabled。
- 删除按钮 disabled。
- 表格仍可查看。

## API 客户端

新增 `frontend/src/api/menu-config.ts`：

```typescript
export function listMenuConfigs()
export function createMenuConfig(data)
export function getMenuConfig(id: string)
export function updateMenuConfig(id: string, data)
export function publishMenuConfig(id: string)
export function cloneMenuConfig(id: string)
export function deleteMenuConfig(id: string)
```

响应结构使用后端统一格式：

```typescript
interface ApiResponse<T> {
  code: number;
  message: string;
  data: T;
}
```

## 错误处理

| 场景 | 行为 |
|---|---|
| 版本列表加载失败 | 提示错误，表格为空 |
| 当前版本加载失败 | 提示错误，保留旧数据 |
| 非 draft 编辑 | 禁用按钮，提示只允许编辑草稿 |
| 保存失败 | 保持弹窗不关闭，显示后端错误 |
| 发布失败 | 显示后端错误，保留当前 draft |
| path/name 重复 | 后端拒绝，前端展示错误 |

## 测试

### 单元测试

新增：

- `menu-adapter.test.ts`

覆盖：

- `routes_json -> FormItemProps[]`
- `FormItemProps[] -> routes_json`
- `_menu` 字段往返
- 无 `_menu` 时自动生成 `id` / `parentId`
- `auths` 字符串与数组转换
- children 层级转换

### E2E

迁移 `tests/e2e/iam-menus.e2e.test.ts` 为 `tests/e2e/menu-configs.e2e.test.ts`：

- `GET /pure/menu-configs`
- `POST /pure/menu-configs` 或 clone draft
- `GET /pure/menu-configs/{id}`
- `PUT /pure/menu-configs/{id}` 保存 routes_json
- `POST /pure/menu-configs/{id}/publish`
- `GET /get-async-routes` 校验发布后的 routes_json

### 手工验收

- 页面能选择版本。
- draft 版本可新增菜单。
- draft 版本可修改菜单。
- draft 版本可删除菜单。
- published 版本按钮禁用。
- 保存后刷新页面数据仍存在。
- 发布后 `/get-async-routes` 返回更新后的菜单。

## NOT in scope

- 不重写菜单 UI。
- 不新增 `editor.vue`。
- 不拆 `RouteTreePanel` / `RouteNodeForm`。
- 不做节点级 PATCH。
- 不改 `prue_backend` 后端结构。
- 不做多人编辑冲突控制。
- 不做权限过滤。

## 风险与权衡

- **`_menu` 扩展字段污染 routes_json**：当前后端允许额外字段，动态路由通常忽略未知字段。后续可在输出 active routes 时 sanitize。
- **保存粒度粗**：每次新增/修改/删除都整体 PUT `routes_json`，当前菜单规模可接受。
- **版本语义弱化**：现有 UI 是直接编辑体验，通过版本下拉补足版本选择，不引入完整版本管理页。
- **并发覆盖**：两个管理员编辑同一个 draft 时仍可能覆盖，后续再加冲突检测。

## 验收标准

1. 现有菜单页面结构基本不变。
2. 页面顶部可选择菜单配置版本。
3. 选中版本后表格展示该版本 `routes_json` 转换出的菜单树。
4. draft 版本可新增 / 修改 / 删除菜单并保存到后端。
5. published / archived 版本禁止编辑。
6. 保存后的 `routes_json` 包含可还原 UI 的 `_menu` 兼容字段。
7. 发布后 `/get-async-routes` 返回新菜单。
8. `menu-adapter.test.ts` 通过。
9. `menu-configs.e2e.test.ts` 通过。
