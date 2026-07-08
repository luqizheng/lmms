# Remove Legacy Mock Demo Views in monitor/ (F4)

## Status: 🟡 待 user review

`frontend_todo.md §7.4.2` 提到「`monitor/logs/*` 因 legacy useRole 误用跳过」check-v-perms linter。本文决定是否删除这些 mock demo view，并设计范围 / 验收。

---

## 1. 现状盘点

| 文件 | 调用的 API | 后端真实接口 | 挂路由 | 实际行为 |
|---|---|---|---|---|
| `views/monitor/logs/login/index.vue` + `hook.tsx` | `getLoginLogsList`（mock） | `audit.ts:listLoginLogs` 已对接 | ❌ | demo 标题「仅演示」 |
| `views/monitor/logs/operation/index.vue` + `hook.tsx` | `getOperationLogsList`（mock） | `audit.ts:listOperationLogs` 已对接 | ❌ | demo 标题「仅演示」 |
| `views/monitor/logs/system/index.vue` + `detail.vue` + `hook.tsx` | `getSystemLogsList` / `getSystemLogsDetail`（mock） | 无对应后端 | ❌ | demo 标题「仅演示」 |
| `views/monitor/online/index.vue` + `hook.tsx` | `getOnlineLogsList`（mock） | `audit.ts:listOnline` 已对接 | ❌ | demo 标题「仅演示」 |

5 个 mock API 在 `src/api/system.ts:55-80`（`getOnlineLogsList` / `getLoginLogsList` / `getOperationLogsList` / `getSystemLogsList` / `getSystemLogsDetail`），无对应后端路由。

**项目自己的真 monitor 视图**（挂路由 + 用 audit-service）：
- `views/monitor/operation-log/index.vue` → `/monitor/operation-log`
- `views/monitor/login-log/index.vue` → `/monitor/login-log`
- `views/monitor/retention/index.vue` → `/monitor/retention`

**`useRole` 命名问题**：4 个 mock view 的 `hook.tsx` 都 `export function useRole()`，**但 hook 内部实现是日志列表数据加载**（form / dataList / pagination / columns / onSearch），**与「角色」概念无关**。`views/system/role/utils/hook.tsx:56` 也有 `useRole`，是真正的角色管理 hook — **两个 `useRole` 是同名不同物**，这是命名误导（vue-pure-admin 上游模板遗留）。

**为什么「误用」导致 check-v-perms 跳过**：`scripts/check-v-perms.js` 的 `EXEMPT_DIRS` 包含 `"logs"` 注释「legacy 副本（含 useRole 误用），未修复」。原因是 `useRole` 命名使 linter 的 `hasAuthImport` 启发式认为这些 view 没引 auth — 实际**hook 内部完全没写按钮权限**，写按钮（「清空日志」「批量删除」「强退」）**确实没挂 v-perms**。

---

## 2. 决策

**推荐：删除这 4 个 mock view + 5 个 mock API 函数**（不再保留作为 demo）。

理由：
1. 4 个 view 未挂路由（`router/modules/monitor.ts:15-46` 只挂 3 个真 view），**用户无法访问**
2. 调的是 mock API（`@/api/system`），**没连后端**，即使挂路由也是假数据
3. 项目已有真 view（`operation-log` / `login-log` / `retention`）对接 audit-service，**功能无缺口**
4. 标题已声明「仅演示」，删除不损失业务
5. 删后 `scripts/check-v-perms.js` 的 `logs` 豁免可移除，linter 覆盖范围变广

**不推荐：保留并改 hook 名**
- 改 `useRole` → `useLogs` 只解决命名误导，不解决 mock API + 未挂路由的根本问题
- 投入产出比低

**不推荐：迁移到真后端**
- 4 个 view → 真 audit-service 端点是大改造（需重写列定义、对接 audit.ts）
- 已有真 view 覆盖核心功能（operation-log / login-log）
- YAGNI

---

## 3. 范围

### 3.1 在范围（本文 commit）

| 删除 | 类型 |
|---|---|
| `frontend/src/views/monitor/logs/login/index.vue` | mock demo view |
| `frontend/src/views/monitor/logs/login/hook.tsx` | mock hook |
| `frontend/src/views/monitor/logs/operation/index.vue` | mock demo view |
| `frontend/src/views/monitor/logs/operation/hook.tsx` | mock hook |
| `frontend/src/views/monitor/logs/system/index.vue` | mock demo view |
| `frontend/src/views/monitor/logs/system/detail.vue` | mock 子组件（被 index 引用） |
| `frontend/src/views/monitor/logs/system/hook.tsx` | mock hook |
| `frontend/src/views/monitor/online/index.vue` | mock demo view |
| `frontend/src/views/monitor/online/hook.tsx` | mock hook |
| `frontend/src/api/system.ts:55-80` 5 个 mock 函数 | `getOnlineLogsList` / `getLoginLogsList` / `getOperationLogsList` / `getSystemLogsList` / `getSystemLogsDetail` |
| `scripts/check-v-perms.js:42` `"logs"` 豁免 | 删后 monitor 下没 logs/ 目录，豁免多余 |

共 9 个 view/hook 文件 + 5 个 mock API 函数 + 1 个 linter 行。

### 3.2 不在范围（本文 commit）

- ❌ 不改 `views/monitor/operation-log/` / `login-log/` / `retention/`（真 view，保留）
- ❌ 不改 `router/modules/monitor.ts`（本来就没挂这些路径）
- ❌ 不改 `@/api/system.ts` 其他函数（仅删 5 个 mock）
- ❌ 不删 `views/monitor/` 整个目录（保留 operation-log / login-log / retention）
- ❌ 不改 `frontend_todo.md`（已记录历史缺口，commit 后手动 close）
- ❌ 不建新 view

### 3.3 隐式清理（commit message 提及）

- `frontend/src/views/monitor/logs/` 整个空目录删除
- `frontend/src/api/system.ts` 删 5 个 mock 函数后保留其他（如 `getUserList` 等真 API）

---

## 4. 验收

- [ ] 9 个文件物理删除
- [ ] `@/api/system.ts` 不再 export 5 个 mock 函数
- [ ] `scripts/check-v-perms.js:42` 移除 `"logs"` 豁免
- [ ] `frontend/src/views/monitor/logs/` 目录不存在
- [ ] 现有 3 个真 monitor view（operation-log / login-log / retention）不受影响
- [ ] `pnpm run test:unit` 不退化（删文件不影响测试）
- [ ] `pnpm run typecheck` 干净
- [ ] `node scripts/check-router-modules.js` 干净
- [ ] 1 commit

---

## 5. 风险

| 风险 | 缓解 |
|---|---|
| 有人从源码引用了 5 个 mock API 函数 | grep 全仓验证（除 monitor/logs/* 和 monitor/online/ 外无引用） |
| 删除后 `views/monitor/logs/` 目录变空 | `rm -rf` 整个目录 |
| 路由模块 `monitor.ts` 检查（showLink:false 之类 hidden 路由） | 复查 monitor.ts 确认没引用这些 view |
| 文档 / spec 引用 | 复查 AGENTS.md / docs/ 提及 monitor 的位置 |

---

_最后更新：2026-07-08_
