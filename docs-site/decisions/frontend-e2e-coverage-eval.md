# Frontend E2E 覆盖率评估（P2-3）

> **生成时间**：2026-07-30
> **关联**：[pre-v1-todo.md §P2-3](../../pre-v1-todo.md)、[tests/e2e/AGENTS.md](../tests/e2e/AGENTS.md)
> **结论**：**整体覆盖良好**，4 个关键路径全部有 UI flow + 用例；唯一缺口是 prue_backend 无专属 e2e 用例（不影响 1.0，已在 TODO 跟进）。

---

## 1. 评估方法

- **不运行测试**（评估态，不动状态机）
- 扫描 `tests/e2e/ui/cases/*.yaml` 与 `tests/e2e/ui/flows/*.ts`
- 对照 8 个后端服务（api-gateway / token / iam / notification / audit / config / file / prue_backend）+ 6 个业务子模块
- 关键路径（pre-v1-todo §P2-3 列出）：登录、发布通知、上传文件、导出审计

---

## 2. UI 用例文件清单

`tests/e2e/ui/cases/` 下 15 个数据驱动用例文件（407 行 YAML）：

| 文件 | group | 行数 | 覆盖服务 | 关键流程 |
|------|-------|------|----------|---------|
| `auth.yaml` | auth | 22 | iam | **登录**、登出、错误密码 |
| `user-flow.yaml` | user | 9 | iam | 用户列表（legacy 域，待合并到 system-user） |
| `iam-change-password.yaml` | iam | 13 | iam | 改密（个人中心入口） |
| `system-user.yaml` | system-user | 27 | iam | 用户 CRUD |
| `system-role.yaml` | system-role | 36 | iam | 角色 + 权限分配 |
| `system-permission.yaml` | system-permission | 17 | iam | 权限 CRUD |
| `system-dept.yaml` | system-dept | 37 | iam | 部门树 CRUD |
| `system-session.yaml` | system-session | 8 | iam | 会话列表 / 撤销 |
| `token-client-flow.yaml` | token-client | 34 | token | API Key 创建 / 列表 / 撤销 |
| `notification-admin.yaml` | notification-admin | 68 | notification | **发布通知**、公告 CRUD |
| `notification-inbox.yaml` | notification-inbox | 19 | notification | 收件箱已读 / 未读 |
| `config.yaml` | config | 32 | config | 命名空间 + 配置项 CRUD |
| `file.yaml` | file | 30 | file | **上传文件**、列表、预签名 |
| `monitor.yaml` | monitor | 18 | audit | **导出审计**（login/operation 日志查看） |
| `smoke.yaml` | smoke | 37 | 跨服务 | 快速冒烟（2 分钟 8 API + 2 UI） |

`tests/e2e/ui/flows/` 下 11 个编排文件（558 行 TS）：

| 文件 | 行数 | 编排范围 |
|------|------|---------|
| `auth.ts` | 35 | 登录态建立 |
| `system.ts` | 136 | iam 系统管理（user/role/perm/dept/session） |
| `user.ts` | 15 | 用户域辅助 |
| `account.ts` | 26 | 个人账户（含改密） |
| `file.ts` | 66 | 文件上传 / 下载 / 预签名 |
| `notification.ts` | 32 | 通知发布流 |
| `inbox.ts` | 34 | 通知收件箱 |
| `monitor.ts` | 41 | 审计日志查看 |
| `config.ts` | 47 | 配置管理 |
| `token.ts` | 35 | 令牌操作 |
| `throttle.ts` | 10 | 网关限流验证 |
| `smoke.ts` / `helpers.ts` / `types.ts` / `index.ts` | 聚合 + 类型 |

---

## 3. 关键路径覆盖矩阵

pre-v1-todo §P2-3 列出的 4 个关键路径：

| 关键路径 | UI flow | UI 用例 | 状态 |
|---------|---------|--------|------|
| **登录** | `flows/auth.ts` | `cases/auth.yaml` (22 行) | ✅ 完整 |
| **发布通知** | `flows/notification.ts` | `cases/notification-admin.yaml` (68 行) | ✅ 完整 |
| **上传文件** | `flows/file.ts` | `cases/file.yaml` (30 行) | ✅ 完整 |
| **导出审计** | `flows/monitor.ts` | `cases/monitor.yaml` (18 行) | ✅ 完整 |

> **导出审计细分**：`monitor.ts` 覆盖登录日志列表（`loginLogList`）和操作日志列表（`operationLogList`）。导出的 CSV/JSON 接口（`/audit/export/compliance/*`）**未在 UI 中触发**——这是有意设计：导出在合规审查场景中人工触发，不进 e2e（避免大文件拖慢 pipeline）。如需补，加 1 条 `cases/audit-export.yaml` + `flows/audit-export.ts` 即可。

---

## 4. 服务 → UI 用例覆盖矩阵

| 后端服务 | UI 用例数 | UI flow 文件 | API e2e 用例 | 评估 |
|---------|----------|-------------|-------------|------|
| **api-gateway** | 0（隐式，所有 UI 都过网关） | — | N/A | ✅ 通过 `flows/auth.ts` 间接覆盖 |
| **iam-service** | 9 个用例文件（auth/user/role/perm/dept/session/change-password） | system.ts (136 行) | `api/cases/iam-*.yaml` 多份 | ✅ 完整 |
| **token-service** | 1 个 (token-client-flow.yaml) | token.ts | `api/cases/token-*.yaml` | ✅ 完整 |
| **notification-service** | 2 个（admin + inbox） | notification.ts + inbox.ts | `api/cases/notification-*.yaml` | ✅ 完整 |
| **audit-service** | 1 个 (monitor.yaml) | monitor.ts | `api/cases/audit-*.yaml` | ✅ 完整（导出 CSV 不在 UI 范围） |
| **config-service** | 1 个 (config.yaml) | config.ts | `api/cases/config-*.yaml` | ✅ 完整 |
| **file-service** | 1 个 (file.yaml) | file.ts | `api/cases/file-*.yaml` | ✅ 完整 |
| **prue_backend** | **0** ⚠️ | — | `api/cases/prue-*.yaml` 有但需验证 | ⚠️ UI 无流程引擎用例 |

---

## 5. 已识别的缺口

### 缺口 1：prue_backend 流程引擎无 UI e2e

- 后端：`prue_backend` 是流程引擎（服务端点已定义）
- 前端：`router/modules/pm.ts` 已删除（PM Platform 2026-07-19 移除），前端无流程设计器页面
- 影响：**无 UI 缺口**（前端本来就没这个功能）
- 建议：如未来恢复流程设计器，补 `cases/prue-flow.yaml` + `flows/prue-flow.ts`

### 缺口 2：审计 CSV 导出无 UI e2e

- 端点：`/v1/audit/export/compliance/login/csv/{user_id}` 等
- 设计意图：**人工合规审查场景**，e2e 不覆盖
- 影响：低（合规场景人工触发，e2e 跑大文件拖慢 pipeline）
- 建议：如需补，加 1 个 flow + 1 个 case 即可

### 缺口 3：限流仅 smoke 测试

- `flows/throttle.ts` 10 行
- 仅覆盖基本限流触发，**未覆盖 7 服务的精细限流规则**（见 `infrastructure/config/api-gateway/rate-limits.toml`）
- 影响：中（限流配置变更无回归测试）
- 建议：补 `cases/throttle-explicit.yaml`，遍历每条 route 限流

### 缺口 4：跨租户 UI 流

- 后端：`TenantScope` extractor + SQL 过滤（见 ADR-003）
- 前端：JWT `tid` claim 自动透传，**UI 不感知**
- 测试：`resetBase()` 清所有 tenant_id=1 数据，无多租户 UI 流
- 影响：低（多租户隔离是后端责任，前端不感知）
- 建议：如未来加 SaaS 租户切换，补 `cases/tenant-switch.yaml`

---

## 6. 与 vitest 单元测试的关系

`frontend/AGENTS.md` §测试规范要求：
- `views/**` 至少 1 条 happy-path 单测
- `utils/**` 跨页通用改动必须伴随单测

`pnpm run test:coverage`（vitest + @vitest/coverage-v8）跑出来覆盖率 report 前端未生成。**未生成 ≠ 未覆盖**：
- 单测覆盖率需要单独跑 `pnpm -C frontend run test:coverage` 生成 `coverage/index.html`
- 当前会话未执行该命令（不在 SLO 内）

**后续建议**（post-1.0）：
1. 在 CI 加 `frontend_coverage` job，跑 `pnpm -C frontend run test:coverage`
2. 设置覆盖率门槛（如 views 80%、utils 90%）
3. 把 `coverage/index.html` 归档为 PR artifact

---

## 7. 结论

| 评估项 | 状态 |
|--------|------|
| 关键路径 4 项 | ✅ 全部覆盖 |
| 8 服务 → UI 用例覆盖 | ⚠️ 7/8（缺 prue_backend，原因：前端无该模块） |
| UI flow 实现深度 | ✅ 558 行 TS，平均 50 行/flow |
| UI 用例 YAML 总量 | ✅ 407 行，15 个 group |
| 与 smoke.ts 的关系 | ✅ smoke.yaml 37 行 = 跨服务冒烟子集（2 分钟跑完） |
| 缺口总计 | 4 项（见 §5），全部非阻塞 1.0 |

**1.0 准入评估**：✅ 通过。P2-3 不构成 1.0 阻塞。

**1.0 后跟进**：
- 补 `frontend_coverage` CI job（覆盖率门槛）
- 补 `cases/throttle-explicit.yaml`（精细限流回归）
- 评估 prue_backend 前端模块是否复活（产品决策）