# Frontend Simple View Mount Tests (F1 Round 3)

## Status: 🟡 待 user review

F1 第一波（commit `307d651`，rule + dataScope 测试）+ 第二波（commit `634a99b`，4 hook spy test）已落。本文决定 F1 第三波（**simple view 组件挂载测试**）的范围、依赖与模式。

---

## 1. 现状盘点

8 个 simple view 候选（无独立 utils/hook.tsx 抽离、逻辑直接写在 index.vue）：

| view | 行数 | 复杂度 | 已测覆盖 |
|---|---|---|---|
| `views/system/tenant/index.vue` | 108 | 简单：table + http 调后端 | ❌ |
| `views/system/session/index.vue` | 101 | 中：table + listIamSessions + revokeIamSession + ElTag JSX | ❌ |
| `views/config/namespace/index.vue` | 126 | 中：CRUD form + table | ❌ |
| `views/config/item/index.vue` | 201 | 中：CRUD + 详情 | ❌ |
| `views/file/index.vue` | 138 | 中：table + 上传/下载按钮 | ❌ |
| `views/file/presign.vue` | 91 | 简单 | ❌ |
| `views/file/upload.vue` | 109 | 中：el-upload | ❌ |
| `views/notification/inbox/index.vue` | 83 | 简单：用 `useInbox()` hook | ✓（read-state.ts 已测，view 本体未测） |

8 个 view 共 957 行。AGENTS.md「测试规范」强制每个 view 至少 1 条 happy-path。

---

## 2. 范围

### 2.1 在范围

- 装 dev dep：`happy-dom`（DOM 环境）+ `@vue/test-utils`（mount helper）
- vitest 配置：默认 node 不变；simple view mount test 顶部加 `// @vitest-environment happy-dom` directive
- 写 8 个 simple view 的 mount test，每个测 happy-path（5-10 it）：
  - 组件挂载成功（无 error）
  - 关键文本/元素存在（标题、按钮、表格列名等）
  - 关键 prop 传递正确（columns 数、search 按钮存在等）
  - 简单交互（点击刷新按钮触发 onSearch → 调 listXxx 等）

### 2.2 不在范围（本文 commit）

- ❌ 复杂 view 挂载测试（login / system/menu / monitor/* — 内部复杂，独立立项）
- ❌ form 提交 / dialog 弹窗 / el-table rowRenderer 完整覆盖（业务逻辑已在 hook test 测过）
- ❌ 修改 `vite.config.ts` 全局 env（用 per-file directive 控制）
- ❌ 改任何 simple view 的源码

---

## 3. 依赖与配置

### 3.1 dev dep

- `happy-dom` ~15.x（vitest 2.x 推荐）
- `@vue/test-utils` ~2.x

安装：
```bash
cd frontend && pnpm add -D happy-dom @vue/test-utils
```

### 3.2 vitest 配置

**不**改 `vite.config.ts`（避免全局改成 happy-dom 拖慢现有 node 测试）。

**用 per-file directive**：
```ts
// @vitest-environment happy-dom
import { mount } from "@vue/test-utils";
// ...
```

vitest 2.x 原生支持 `// @vitest-environment xxx` 注释在文件顶部。

### 3.3 Mock 模板（每个 mount test 顶部）

跟 hook test 类似，但需要 mount 时的 DOM 环境 + 父组件依赖：
- `@/api/iam` / `@/api/file` / `@/api/config` / `@/api/notification`（按 view 依赖）
- `@/utils/message`
- `@/utils/sso` / `@/router`（阻断 sso IIFE 副作用）
- `@/components/RePureTableBar` / `@/components/ReIcon/src/hooks`（stub 为简单组件）

按 view 复杂度 stub 5-10 个。

---

## 4. 测试矩阵（8 文件 / 5-10 it 每个）

### 4.1 tenant/index.vue（5-6 it）
- mount 不抛错
- 标题包含"租户"
- 5 个 columns 标签（ID / Slug / 显示名 / 管理员 / 创建时间）
- 搜索 + 新增 + 刷新按钮存在
- 点击刷新调 listIamTenants（或 http）

### 4.2 session/index.vue（5-6 it）
- mount 不抛错
- 6 个 columns 标签
- 状态列 cellRenderer 渲染 el-tag
- 踢人按钮存在

### 4.3 config/namespace/index.vue（5-6 it）
- mount 不抛错
- 4 个 columns 标签
- 新建 / 编辑 / 删除按钮存在

### 4.4 config/item/index.vue（6-7 it）
- mount 不抛错
- columns 含 key / value / status
- 暴露 / 复制按钮存在

### 4.5 file/index.vue（5-6 it）
- mount 不抛错
- columns 含文件名 / 大小 / 上传时间
- 上传 / 下载按钮存在

### 4.6 file/presign.vue（4-5 it）
- mount 不抛错
- 预签名表单字段（filename / content-type / expires-in）渲染
- 提交按钮存在

### 4.7 file/upload.vue（5-6 it）
- mount 不抛错
- el-upload 占位存在
- 临时 / 永久选项渲染

### 4.8 inbox/index.vue（4-5 it）
- mount 不抛错（useInbox hook 在 setup 同步产物）
- 3 个 tab 标签（全部 / 未读 / 已读）
- 标题含"站内信"

合计 ~40-50 it。

---

## 5. 风险与降级

| 风险 | 缓解 |
|---|---|
| `happy-dom` 装上后引入 SSR / 模块解析问题 | 隔离到 per-file directive，不影响 node test |
| `.vue` 组件内部有未 mock 的 element-plus / 业务组件 → mount 时崩 | 加完整 mock 列表（每个 view test 顶部 `vi.mock` 5-10 个） |
| `el-table` / `el-dialog` 等组件挂载需要更多 element-plus stub | happy-dom 渲染时 element-plus 走真实 DOM；只需 mock 业务 hook（`@/api/...`） |
| view 内部用 `useRoute()` / `useRouter()`（vue-router 上下文） | `vi.mock("vue-router", () => ({ useRoute: () => ({ params: {} }), useRouter: () => ({ push: vi.fn() }) }))` |
| `getCurrentInstance` 等 Vue 内部 API 在 test 缺失 | happy-dom 提供 component context，OK |

---

## 6. 验收

- [ ] `happy-dom` + `@vue/test-utils` dev dep 装上
- [ ] 8 个 mount test 文件，~40-50 it 全过
- [ ] `pnpm run test:unit` 21 files → 29 files，123 → 170+ 全过
- [ ] `pnpm run typecheck` 干净
- [ ] 现有 node env test（utils / hook）**0 退化**
- [ ] 1 commit

---

_最后更新：2026-07-08_
