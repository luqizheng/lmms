# Frontend Utils Test Coverage (P1 残留闭环 — 第一批)

## Status: 🟡 待 user review

`frontend_todo.md` §7.2/§7.3 列出的 P1/P2 残留中，**utils 单测补齐**为首批推进项。本文锁定第一批 9 个 util（含 `message.ts` + `http/index.ts` 高价值 vi.mock 模块）的覆盖范围、命名约定、测试模式、验收标准与 commit 节奏。

---

## 1. 背景与动机

`frontend/AGENTS.md`「测试规范」段强制：

> 任一新增 `src/views/**` view，**至少**交付一条 happy-path 单测。跨页通用 util、formatter、menu-adapter 之类的纯函数模块改动，**必须**伴随单测更新。

实际盘点（2026-07-08）：

- 146 个 `.vue` view，**仅 2 个 `.test.ts`**（`views/notification/inbox/utils/read-state.test.ts` + `views/system/menu/utils/menu-adapter.test.ts`）
- `frontend/src/utils/` 共 19 个文件（含 3 个子目录），**0 个 `.test.ts`**
- 整体覆盖率约 **1.4%**（按文件数），距 AGENTS.md 强制要求差距巨大

第一批聚焦**跨页通用 util**（非 view、非复杂副作用）。第二批再处理 view 与重依赖模块（http/message）。

---

## 2. 范围

### 2.1 第一批覆盖（9 个 util，12-14 个 `.test.ts`）

| 文件 | 纯函数 / 半纯 | 估计测试条数 | AGENTS.md 强制 | mock 依赖 |
|---|---|---|---|---|
| `frontend/src/utils/tree.ts` | ✅ 6 函数 | 8-10 | 是（菜单/部门树） | — |
| `frontend/src/utils/error-translate.ts` | ✅ 3 函数 | 4-5 | 是（错误码 i18n） | — |
| `frontend/src/utils/download.ts` | ✅ 2 函数 | 3 | 否（但 audit 导出按钮强依赖） | `document` (vi.stubGlobal) |
| `frontend/src/utils/notification-socket.ts` | 模块单例 | 3-4 | 是（AGENTS.md 详写策略） | `WebSocket` (vi.stubGlobal) |
| `frontend/src/utils/localforage/index.ts` | 半 | 3 | 否 | `localforage` (vi.mock) |
| `frontend/src/utils/iam-scopes-cache.ts` | 半 | 3 | 否 | `@/api/iam-scopes` (vi.mock) |
| `frontend/src/utils/auth.ts` | 部分 | 4-5 | 是（JWT 解析） | `@/store/modules/user` (vi.mock) |
| `frontend/src/utils/message.ts` | 半（ElMessage 包装） | 4-5 | 是（全局提示） | `element-plus` (vi.mock) |
| `frontend/src/utils/http/index.ts` | 类（PureHttp 拦截器） | 5-7 | 是（JWT 拦截） | `axios` + `@/store/modules/user` + `@/utils/auth` (vi.mock) |

合计 37-46 个 `it()`，文件 9 个 `.test.ts`，mock 5 个外部依赖。

### 2.2 显式排除（第二批 / 不做）

| 文件 | 排除原因 |
|---|---|
| `mitt.ts` | 透传 mitt 库，1 行初始化，无业务逻辑 |
| `sso.ts` | IIFE 副作用，几乎不可测 |
| `responsive.ts` | `app.use(Storage, ...)` 强 Vue App 上下文 |
| `print.ts` | 强 DOM 依赖，jest-doc-like 集成测试 |
| `progress/index.ts` | NProgress.configure 模块副作用 |
| `propTypes.ts` | 静态类型辅助，无需测 |
| `preventDefault.ts` | 1 行函数，不值 |
| `chinaArea.ts` | 静态数据 |
| `globalPolyfills.ts` | 全局 polyfill 注入副作用 |

### 2.3 不在本文范围

- `src/api/*.ts` 的 DTO 校验（codegen 产物，已由后端 schema 覆盖）
- `src/views/**` 的 view 单测（第二批立项）
- 浏览器级 E2E（AGENTS.md 明确「按需立项，不在本规范强制范围」）

---

## 3. 测试模式与基础设施

### 3.1 复用现有风格

参考 `frontend/src/views/notification/inbox/utils/read-state.test.ts`：

```ts
import { describe, expect, it, beforeEach, vi } from "vitest";
// 直接 import，describe/it/expect 显式（无 globals）
// 副作用用 vi.stubGlobal("localStorage", ...)
// 依赖 API 用 vi.mock("@/api/...", () => ({ ... }))
```

**禁止**用 globals（tsconfig.json 的 `types` 不含 `"vitest/globals"`）—— 与现有 2 个测试保持一致。

### 3.2 不引入 happy-dom / jsdom

`node_modules` 已无 happy-dom/jsdom；现有 2 个测试在 node env 跑通。第一批 util 全部是纯函数 + 半纯函数（`downloadRaw` 涉及的 `document` 用 `vi.stubGlobal` 模拟），**0 配置改动**。

### 3.3 vitest 配置

**不新建** `vitest.config.ts`。vitest 默认从 `vite.config.ts` 读 alias（`@ → ./src/*`），现有 `package.json` 的 `test:unit` / `test:unit:watch` / `test:unit:menu` 已能跑通。

### 3.4 文件放置

按 AGENTS.md「就近放置」：

```
frontend/src/utils/tree.ts
frontend/src/utils/tree.test.ts        ← 紧邻被测文件
frontend/src/utils/error-translate.ts
frontend/src/utils/error-translate.test.ts
...
```

**不**新建 `__tests__/` 目录。

---

## 4. 第一批每个 util 的测试矩阵

### 4.1 `tree.test.ts`（8-10 个 `it`）

| 函数 | 测什么 |
|---|---|
| `handleTree` | 标准 parent-children 构树；空数组；缺 parentId 的孤立节点；自定义 id/parentId/children 字段名 |
| `buildHierarchyTree` | 嵌套添加 id/parentId/pathList；空输入；非数组输入（console.warn） |
| `deleteChildren` | 单子节点自动删 children 字段；多子节点保留；嵌套；非数组输入 |
| `extractPathList` | 收集所有节点 uniqueId；嵌套；空输入 |
| `getNodeByUniqueId` | 顶层命中；深层命中；未命中返回 `[]`；非数组输入 |
| `appendFieldByUniqueId` | 追加字段到指定节点；fields 非对象时跳过；嵌套；空输入 |

### 4.2 `error-translate.test.ts`（4-5 个 `it`）

| 函数 | 测什么 |
|---|---|
| `translateError` | zh-CN 命中；en fallback（设 `setLocale("en")` 后查英文）；都未命中回退 code 自身 |
| `setLocale` / `getLocale` | 切换语言 |
| 默认 locale | 未调用 setLocale 时默认 zh-CN |

### 4.3 `download.test.ts`（3 个 `it`）

| 函数 | 测什么 |
|---|---|
| `downloadRaw` | 创建 Blob、`URL.createObjectURL` 被调；`<a>` 元素 append → click → remove；`URL.revokeObjectURL` 被调。**用 `vi.stubGlobal("document", ...)` 模拟 DOM** |
| `timestampedFilename` | 输出格式 `prefix_YYYYMMDD_HHMMSS.ext`；`padStart` 边界（单数字补 0）；自定义 prefix/ext |

### 4.4 `notification-socket.test.ts`（3-4 个 `it`）

| 函数 | 测什么 |
|---|---|
| `RETRY_DELAYS` 行为 | `scheduleReconnect` 内的 `Math.min(retry, length-1)` 边界（不可直接访问常量，但可通过 `connect → close` 序列观察 `setTimeout` 调用参数） |
| `connect()` | `VITE_NOTIFICATION_WS_DISABLED=true` 时不创建 socket；socket 已存在且 readyState ≤ 1 时幂等 |
| `disconnect()` | 清 retryTimer；清 socket；retry 归零 |
| `on(type, h)` | 注册/反注册；返回的 unsubscribe 函数可移除 handler |

**实现策略**：用 `vi.stubGlobal("WebSocket", class { ... })` mock WebSocket 构造，断言 `setTimeout` 被以 `RETRY_DELAYS[i]` 调度。

### 4.5 `localforage/index.test.ts`（3 个 `it`）

| 函数 | 测什么 |
|---|---|
| `StorageProxy.getItem` | expires=0 永久；expires > now 返回 data；expires < now 返回 null（已过期）；底层 localforage 抛错时 reject |
| `StorageProxy.setItem` | m=0 时 expires=0；m>0 时 expires=now+m*60*1000 |
| `localForage()` 工厂 | 每次返回新实例；config 被设 |

**实现策略**：`vi.mock("localforage", () => ({ default: mockForage }))` 注入 mock storage。

### 4.6 `iam-scopes-cache.test.ts`（3 个 `it`）

| 函数 | 测什么 |
|---|---|
| `loadScopes` 缓存命中 | 5min TTL 内第二次调用不打 API（用 `vi.mock("@/api/iam-scopes", ...)`） |
| `loadScopes` 缓存过期 | 模拟 `Date.now()` 推进 6min 后再次调用，重新打 API |
| `invalidateScopes` | 删 cache 后下次 `loadScopes` 重新打 API |

### 4.7 `auth.test.ts`（4-5 个 `it`）

| 函数 | 测什么 |
|---|---|
| `formatToken` | 返回 `"Bearer " + token` |
| `hasPerms` | 无 permissions → false；permissions=`[*:*:*]` → true；permissions 包含 value（string） → true；permissions 不包含 value → false；value=array 时 `isIncludeAllChildren` 命中 → true（mock pure-admin/utils） |
| `multipleTabsKey` / `TokenKey` / `userKey` 常量 | 字符串值正确（防 typo） |

**不测** `getToken` / `setToken` / `removeToken` —— 强依赖 Cookies + Pinia store，第二批单测视图时再覆盖。

### 4.8 `message.test.ts`（4-5 个 `it`）

| 函数 | 测什么 |
|---|---|
| `message()` 无 params | 调用 `ElMessage` 时 `customClass: "pure-message"` |
| `message()` 有 params（部分默认） | `type` 默认 `"info"`；`duration` 默认 2000；`offset` 默认 16；`placement` 默认 `"top"`；`appendTo` 默认 `document.body`；`grouping` 默认 `false`；`repeatNum` 默认 1；`customClass` `"antd"` 映射到 `"pure-message"`，`"el"` 映射到 `""` |
| `message()` 有 params（全显式） | 所有值透传 |
| `message()` `onClose` | 包装后被调；非函数 onClose 不报错 |
| `closeAllMessage()` | 调用 `ElMessage.closeAll` |

**实现策略**：`vi.mock("element-plus", () => ({ ElMessage: vi.fn(), closeAll: vi.fn() }))`，断言 `ElMessage` 收到正确参数。

### 4.9 `http/index.test.ts`（5-7 个 `it`）

测试目标是 **`http` 单例** —— 必须把 axios 整个换掉。

**实现策略**（必须在 `describe` 顶部一次性设置）：
- `vi.mock("axios", () => ({ default: { create: () => mockInstance, isCancel: vi.fn() } }))`
- `vi.mock("@/store/modules/user", () => ({ useUserStoreHook: () => ({ handRefreshToken, logOut }) }))`
- `vi.mock("@/utils/auth", () => ({ getToken, formatToken: t => "Bearer " + t }))`
- `vi.mock("@/utils/message", () => ({ message: vi.fn() }))`
- 动态 import `import { http } from "./index"` 必须在 mock 设置**之后**（用 `await import` 或 vi.resetModules 隔离）

| 函数 / 路径 | 测什么 |
|---|---|
| `http.request` 成功 | 透传 `axiosInstance.request` 返回 `response.data` |
| `http.request` 失败 | `isCancelRequest` flag 置位后 reject |
| request 拦截：白名单 URL | `/auth/login` / `/auth/refresh` 结尾不附 Authorization header |
| request 拦截：未过期 token | `expires > now` 时附 `Authorization: formatToken(data.accessToken)` |
| request 拦截：过期 token | `expires < now` 时调 `handRefreshToken({refreshToken})`；成功后附新 token；失败时 `logOut` + 弹 expired 消息 |
| request 拦截：无 token | 不附 Authorization header（resolve config） |
| `http.get` / `http.post` | 透传 method 参数 |

**注意事项**：
- `http` 是模块单例，**所有测试共享同一拦截器实例**。每次 `beforeEach` 必须 `vi.clearAllMocks()` 并可能需要 `vi.resetModules()` 重新 import 以隔离内部 `requests` 队列 / `isRefreshing` flag。
- 拦截器内部用 `useUserStoreHook().handRefreshToken().then().catch().finally()`，需用 `vi.fn().mockResolvedValueOnce(...)` 链式调用。

---

## 5. 验收

### 5.1 必须通过

```bash
cd frontend
pnpm run test:unit
```

预期：所有 `*.test.ts` 通过，新加 9 个 `.test.ts`，合计 11 个文件 / 37-46 个 `it()`，全绿。

### 5.2 不退化

- `pnpm run typecheck` 干净
- `pnpm run lint:eslint` 无新增 warning
- 现有 `read-state.test.ts` + `menu-adapter.test.ts` 仍通过

### 5.3 不做

- ❌ 不装 happy-dom / jsdom
- ❌ 不新建 `vitest.config.ts`
- ❌ 不动 `package.json`（除追加 `test:unit:tree` / `test:unit:auth` 等**可选**单文件跑入口，不强制）
- ❌ 不改 `tsconfig.json`
- ❌ 不改任何源文件（`tree.ts` / `error-translate.ts` / ...）
- ❌ 不删任何 util
- ❌ 不接 CI 门禁（第二批立项）

---

## 6. Commit 节奏

按项目规范（AGENTS.md §Git 与提交规范）：

- **一个 commit**：`test(frontend): add unit tests for 9 cross-page utils (AGENTS.md test spec)`
- 包含：
  - 9 个新 `.test.ts`
  - 1 个本文档（spec）
  - **不**包含任何 `frontend/src/utils/<util>.ts` 的改动
- commit message 模板：
  ```
  test(frontend): add unit tests for 9 cross-page utils

  补 frontend/src/utils/ 下 9 个跨页通用 util 的单测：
  tree / error-translate / download / notification-socket /
  localforage / iam-scopes-cache / auth / message / http

  闭合 frontend_todo.md §7.2 P1 残留「utils 单测补齐」首批。
  测试模式参考 read-state.test.ts（vi.stubGlobal 模拟 DOM，0 配置改动）；
  message / http 通过 vi.mock 包装 element-plus + axios。
  详见 docs/superpowers/specs/2026-07-08-frontend-utils-test-coverage-design.md

  Refs: docs/ai-context.md §"测试规范"；frontend/AGENTS.md §"测试规范"
  ```

---

## 7. 第二批 / 第三批（不在本文 commit 范围）

- **第二批**：`message.ts`（vi.mock element-plus）、`http/index.ts`（拦截器流，需 vitest 进阶）、核心 view 单测（login/user/role/menu/dept/tenant/file/config/notification）
- **第三批**：`scripts/check-vitest-coverage.js`（扫 `views/` 新增 .vue 是否有对应 .test.ts，作为 CI linter）

---

## 8. 风险与回滚

### 8.1 风险

- **Risk 1**：`notification-socket.ts` 内部用 `import.meta.env` —— vitest 默认支持，但需确认；如不支持降级为 `vi.stubEnv`
- **Risk 2**：`iam-scopes-cache.ts` 的 `Date.now()` 推进测试需用 `vi.useFakeTimers` + `vi.setSystemTime`，与 happy-dom 无关但需小心清理
- **Risk 3**：`auth.ts` 的 `hasPerms` 依赖 `@pureadmin/utils` 的 `isIncludeAllChildren` —— 需 `vi.mock("@pureadmin/utils", ...)` 提供 stub
- **Risk 4**：`http/index.ts` 是模块单例 + 多重 mock（axios / store / auth / message）—— 测试隔离困难，**最坏情况** `http.test.ts` 不稳定（interceptor 副作用泄漏），需 `vi.resetModules()` + 动态 import。**降级方案**：把 4-6 个 `it` 收敛到 3 个关键路径（白名单、过期 token、正常 token），不追求全分支。
- **Risk 5**：`message.ts` 测试依赖 `vi.mock("element-plus", ...)`，若 element-plus 包导出结构变化（`ElMessage.closeAll` 路径）需调整 mock surface

### 8.2 回滚

单个 commit `git revert <hash>` 即可，**0 源文件改动**意味着回滚后无副作用。

---

## 9. 拍板记录

- **范围**：✅ 9 个 util（含 `message.ts` + `http/index.ts` vi.mock 高价值模块）
- **commit 节奏**：✅ 1 个总 commit
- **配置改动**：0（沿用现有 vite.config.ts alias，node_modules 不装 happy-dom/jsdom）
- **第二批立项**：view 单测（login/user/role/menu/dept/tenant/file/config/notification）+ CI linter 留作后续 commit，不在本 commit 范围

---

_最后更新：2026-07-08_
