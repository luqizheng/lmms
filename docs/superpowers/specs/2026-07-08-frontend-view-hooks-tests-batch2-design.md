# Frontend View Hooks Tests Batch (F1 Round 2)

## Status: 🟡 待 user review

F1 第一波（commit `307d651`）已为 4 个核心 view 补 rule / dataScope 单测（22 it）。本文决定 F1 第二波（**4 个 hook.tsx 完整测试**）的范围与策略。

---

## 1. 4 个 hook 真实复杂度盘点

| hook | 行数 | 顶层 import | JSX | onMounted API | 可抽纯函数 |
|---|---|---|---|---|---|
| `system/user/utils/hook.tsx` | 428 | reset.css + 2 .vue 子组件 + user.jpg + 7 api + element-plus | 多 | 3 个 (listIamUsers/listIamOrgUnits/listIamRoles) | ❌ 几乎都是 handler |
| `system/role/utils/hook.tsx` | 330 | form.vue + 7 api + element-plus | 多 | 2 个 (listIamRoles/listIamPermissions) | ✅ 已抽 `data-scope.ts` |
| `system/dept/utils/hook.tsx` | 240 | form.vue + 5 api + element-plus | 多 | 1 个 (listIamOrgUnits) | ❌ |
| `notification/admin/utils/hook.tsx` | 203 | form.vue + 4 api + element-plus | 多 | 1 个 (listAnnouncements) | ✅ `statusMap` dict |

**共同挑战**：
- 每个 hook 都 `import "../form/*.vue"` 子组件 → 需 Vue 编译器
- 都用 element-plus JSX → 需 mock el-tag/el-switch/el-image
- `import "@/assets/user.jpg"` 等 asset import → vite 静态资源处理
- `import "./reset.css"` 副作用 → vitest 需处理
- `onMounted` 内调用 1-3 个 API → mock 多个 vi.fn
- `addDialog` 弹窗内调用 `contentRenderer: () => h(editForm, ...)` → 需 stub ReDialog

**真实可独立测的纯函数**（4 个 hook 全部已盘点）：
- ✅ role hook 已抽 `data-scope.ts`（第一波完成）
- 🆕 admin hook `statusMap` dict（**未抽**）
- ❌ user / dept hook 无明显可抽纯函数

---

## 2. 三个路径

### 路径 A：admin statusMap 抽离 + 4 hook spy test

**范围**：
- 新增 `notification/admin/utils/status.ts` 含 `STATUS_LABEL_MAP` (statusMap) + 可能的 `getStatusInfo` 函数
- 写 `notification/admin/utils/status.test.ts`（4-6 it）
- 写 4 个 hook 的"shape + 关键 API spy" test：调用 `useXxx()` 后断言
  - 返回对象包含 5-10 个 expected key（columns, dataList, onSearch, handleDelete, etc.）
  - columns 数组长度
  - onSearch 调用时调对了 listIamXxx + 用 form 参数
  - handleDelete 调用时调对了 deleteIamXxx

**风险**：
- hook 顶层 import 的 .vue 组件可能让 vitest 报「无法解析」或「渲染错误」
- `import "./reset.css"` 副作用可能让 vitest warning（非 fail）
- asset import（user.jpg）可能让 vitest 报「no loader」

**预计**：8 个 .test.ts / 30-50 个 it / 300-500 行

**价值**：高（覆盖 hook 主流程）

### 路径 B：只 admin statusMap 抽离

**范围**：
- 新增 `notification/admin/utils/status.ts` 含 `STATUS_LABEL_MAP`
- 写 `notification/admin/utils/status.test.ts`（3-5 it）

**预计**：2 个文件 / 5-8 个 it / 50-100 行

**价值**：低（与第一波同模式，无新意）

### 路径 C：跳过 F1 第二波

跳到剩余项（B1 健康检查三级探针 / B2 migrations state 文档同步 / simple view 组件挂载测试）。

**预计**：无 F1 commit

**价值**：F1 第二波标"未来 wave"留在 spec，下次重新评估

---

## 3. 推荐

**路径 A**（最大价值，moderate 风险）。

理由：
1. 路径 B 价值低（重复第一波模式，无新进展）
2. 路径 C 完全放弃 F1 第二波
3. 路径 A 即使部分 hook 因 import 链失败，也能从失败信息学到底层如何 stub（落地经验）
4. AGENTS.md 强制 view 单测，hook 是 view 的核心业务逻辑，迟早要做

**降级策略**：
- 跑测试时如遇 import 链炸，先 mock 整个 import 链（`vi.mock` sso/router/element-plus/`<asset>`），不行再降级到路径 B

---

## 4. 范围（路径 A 详细）

### 4.1 status.ts 抽离（admin）

`notification/admin/utils/status.ts`：
```ts
export const STATUS_LABEL_MAP: Record<string, { label: string; type: "primary" | "success" | "warning" | "info" | "danger" }> = {
  draft: { label: "草稿", type: "info" },
  published: { label: "已发布", type: "success" },
  revoked: { label: "已撤销", type: "danger" }
};

export function getStatusInfo(s: string) {
  return STATUS_LABEL_MAP[s] || { label: s, type: "info" as const };
}
```

`admin/utils/hook.tsx` 改 import 自 `./status`。

### 4.2 status.test.ts（4-5 it）

- 3 个 status 标签正确
- getStatusInfo(unknown) fallback
- 字段类型断言

### 4.3 4 hook spy test（4 个 .test.ts，6-12 it 每个）

每个 hook.tsx 测：
- `useXxx()` 返回 shape（expected key 集合）
- `columns.length` 正确
- `onSearch` 调 listIamXxx 时 form 参数正确
- `handleDelete` 调 deleteIamXxx 时 row.id 正确（user hook）
- `handlePublish` 调 publishAnnouncement（admin hook）
- `onMounted` 副作用可被触发（用 vi.useFakeTimers 或直接调）

**Mock 模板**（每个 test 顶部，vi.hoisted 共享）：
```ts
const { listIamUsers, deleteIamUser, message, addDialog, ElMessage } = vi.hoisted(() => ({
  listIamUsers: vi.fn(),
  deleteIamUser: vi.fn(),
  message: vi.fn(),
  addDialog: vi.fn(),
  ElMessage: { error: vi.fn(), success: vi.fn() }
}));
vi.mock("@/api/iam", () => ({ listIamUsers, deleteIamUser, /* ... */ }));
vi.mock("@/utils/message", () => ({ message }));
vi.mock("@/components/ReDialog", () => ({ addDialog }));
```

### 4.4 不在范围

- 完整 hook 渲染测试（需 jsdom / happy-dom，独立立项）
- 测试 view 本身（.vue 挂载）
- 测试 `tree.vue`（system/user 子组件）

---

## 5. 验收

- [ ] `admin/utils/status.ts` 抽离 + `admin/utils/status.test.ts` 4-5 it 全过
- [ ] `admin/utils/hook.tsx` 改 import 自 ./status
- [ ] 4 个 hook 的 .test.ts 6-12 it 每个全过
- [ ] `pnpm run test:unit` 累计 93 → 130+ 全过
- [ ] typecheck 干净
- [ ] 1 commit

---

## 6. 风险与降级

| 风险 | 降级 |
|---|---|
| 4 个 hook.tsx 顶层 import 链触发 sso/router/element-plus 副作用 | 在 test 顶部 `vi.mock("@/utils/sso", () => ({}))` + `vi.mock("element-plus", ...)` + `vi.mock("@/router", () => ({}))` |
| .vue 组件 import 报「无法解析」 | vitest 配置或 @vue/compiler-sfc 已装（项目已用 vite-plugin-vue）；如仍失败 → mock 整个组件为 `<div />` 占位 |
| asset import（user.jpg）报「no loader」 | `vi.mock("@/assets/user.jpg", () => ({ default: "mocked-avatar" }))` |
| 4 个 hook spy test 都失败 | 退到路径 B（只 admin statusMap） |

---

_最后更新：2026-07-08_
