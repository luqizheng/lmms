# 前端公共组件

> **适用范围**：`frontend/src/components/` 下的 `Re*` 系列可复用组件
> **技术栈**：Vue 3 + TypeScript + Element Plus

---

## 组件目录

所有公共组件放在 `frontend/src/components/Re*` 目录下，命名遵循 `ReXxx` 惯例。

| 组件 | 用途 |
|------|------|
| `ReUserLabel` | 输入 userId，显示昵称（带 LRU 缓存 + 请求去重） |
| `ReUserSelector` | 用户搜索选择器（下拉多选/单选） |
| `ReRoleSelector` | 角色选择器 |
| `ReOrgSelector` | 组织（部门）选择器 |
| `RePerms` | 按钮级权限控制 |
| `ReAuth` | 页面级权限守卫 |
| `ReDialog` | 可拖拽弹窗 |
| `ReText` | 文本溢出省略 + tooltip |
| `ReIcon` | 图标组件 |
| ... | 共 32 个组件 |

---

## ReUserLabel

### 功能

传入 `userId` 字符串，自动显示用户昵称。适用于表格列、列表项、详情页中需要展示用户名称的场景。

### 核心特性

| 特性 | 说明 |
|------|------|
| **LRU 缓存** | 全局 100 条，命中直接返回，零网络开销 |
| **请求去重** | 同一 userId 并发调用只发 1 次 HTTP 请求 |
| **Loading 骨架** | 加载中展示 shimmer 骨架动画，不破坏布局 |
| **失败回退** | API 失败时显示 userId 本身，不抛异常 |

### 使用方式

```vue
<script setup lang="ts">
import ReUserLabel from "@/components/ReUserLabel";
</script>

<template>
  <!-- 表格列中 -->
  <el-table-column label="创建者" width="120">
    <template #default="{ row }">
      <ReUserLabel :user-id="row.created_by" />
    </template>
  </el-table-column>

  <!-- 列表项中 -->
  <div v-for="item in list" :key="item.id">
    <ReUserLabel :user-id="item.owner_id" />
  </div>
</template>
```

### Props

| 属性 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `userId` | `string` | 是 | 用户 ID |

### 工作原理

```
┌───────────────────────────────────────────────┐
│  ReUserLabel                                │
│                                              │
│  onMounted ──→ getUserNickname(userId)       │
│                     │                        │
│              ┌──────┴──────┐                 │
│              ▼              ▼                │
│         LRU Cache       Pending Map         │
│        (100 entries)   (inflight dedup)      │
│              │              │                │
│              └──────┬──────┘                 │
│                     ▼                        │
│              getIamUser(id)                  │
│            GET /v1/iam/users/{id}            │
│                     │                        │
│                     ▼                        │
│              显示昵称 (nickname)              │
└───────────────────────────────────────────────┘
```

**场景示例**：列表 100 行，其中 40 行是同一 userId：

1. 第 1 个 `ReUserLabel` 挂载 → cache miss + pending miss → 发 HTTP 请求
2. 第 2~40 个几乎同时挂载 → pending 命中 → 复用同一个 Promise
3. 请求返回后 → 写入 LRU 缓存 → 40 个组件同时渲染昵称
4. 后续再有同 userId → 直接命中缓存，**零请求**

### 缓存管理

需要手动刷新用户昵称时（如用户修改了个人信息）：

```ts
import {
  clearUserLabelCache,
  invalidateUserLabel
} from "@/components/ReUserLabel";

// 清除单个用户的缓存
invalidateUserLabel("user-abc-123");

// 清除全部缓存（如全局用户信息变更）
clearUserLabelCache();
```

### 实现文件

```
frontend/src/components/ReUserLabel/
├── index.ts        ← 导出入口
├── index.vue       ← Vue 组件（shimmer loading + watch userId 自动刷新）
└── cache.ts        ← LRU 缓存 + 请求去重逻辑
```

---

## 添加新组件

在 `frontend/src/components/` 下新增组件时，遵循以下约定：

1. **目录命名**：`ReXxx/` 格式，驼峰命名
2. **文件结构**：采用扁平风格（`index.vue` + `index.ts`），与 `ReUserSelector`、`ReUserLabel` 一致
3. **TypeScript**：`<script setup lang="ts">` + 完整的 Props 类型声明
4. **导出**：`index.ts` 中 re-export 组件和类型，必要时导出管理函数

```ts
// 标准导出模板
export { default } from "./index.vue";
export type { SomeType } from "./index.vue";
export { clearSomeCache, invalidateSomething } from "./cache";
```
