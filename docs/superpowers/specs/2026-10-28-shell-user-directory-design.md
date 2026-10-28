# 壳级用户目录 — 设计文档

> 日期：2026-10-28
> 关联：`docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`（壳设计）、`docs/superpowers/plans/2026-10-26-prism-admin-ui-roadmap.md`（计划 5 框架系统页 / 计划 7 业务迁移）
> 范围：`frontend/src/shell/`（新增 api / store / 组件，接入 2 个系统页）
> 风险：低（纯前端增量，不改后端契约，不改登录态）

---

## 1. 现状

### 1.1 具体行为

审计操作日志、系统会话管理两个列表页直接渲染 `user_id` 原始串：

| 页面 | 列 | 实际渲染 |
|---|---|---|
| `views/audit/operation-log` | 操作人 | `u-1` / `dev__abc` |
| `views/system/session` | 用户 | `u-1` / `dev__abc` |

业务人员看不出"是谁"操作了、这条会话属于谁。

### 1.2 根因

壳层没有 userId → 昵称的解析能力。壳重建前 pure-admin 侧有 `frontend/src/components/ReUserLabel`（LRU 缓存 + 逐个请求 `GET /v1/iam/users/{id}`），壳重建时该目录被 `components/ui` 取代、等价能力没有迁移过来。`docs-site/guides/frontend-components.md` 仍在描述该旧组件，源码已不存在（本次不改该文档，见 §6.3）。

### 1.3 影响面

当前仅框架系统页 2 处。下游业务系统迁移（路线图计划 7）后，所有"负责人 / 创建人 / 操作人"列都会重复这个问题——所以能力要建在壳里，而不是逐页实现。

## 2. 目标与非目标

### 目标

1. 壳提供可复用的 userId → 昵称 展示能力，业务页零配置接入。
2. **请求数与列表行数解耦**：不能出现 N 行 N 次请求的 N+1。
3. 解析失败不阻断页面：回退到可读的 ID 文案，而不是空白或抛错。

### 非目标

- 不做用户搜索选择器（`ReUserSelector` 等价物）——留给计划 7 的实际业务需求。
- 不改后端接口、不新增端点、不动 OpenAPI。
- 不做跨会话持久化缓存（见 §6.1）。

## 3. 方案选型

| 方案 | 请求数（100 行 / 3 个不同 userId） | 后端改动 | 结论 |
|---|---|---|---|
| A 逐行 `GET /v1/iam/users/{id}` + LRU 缓存 | 首次 3 次，后续 0 | 无 | 旧 `ReUserLabel` 做法；行数与用户数同步增长时请求数上升，N+1 问题被 LRU 缓解但未消除 |
| **B 一次性拉全量 `/v1/iam/users/simple` 分页合并 → 会话内目录** | **⌈用户数/200⌉，与行数无关** | **无** | **采用** |
| C 后端批量端点 `GET /v1/iam/users?ids=` | 1 | 需要（iam-service + 网关） | 最优，但要动后端；本次不做，列为后续演进方向 |

**选 B 的理由**：后端已有现成接口，前端零后端改动即可把"查昵称"降为 O(1) 内存查表；当前租户用户量级（百 ~ 千）下全量目录体积可接受。代价是目录随租户用户数线性增长，边界条件见 §6.2。

## 4. 详细设计

### 4.1 接口契约（后端既有，前端只消费）

`GET /v1/iam/users/simple?page={n}&size={m}&keyword={kw}`

```jsonc
{ "items": [{ "id": "u-1", "nickname": "张三" }], "total": 42, "page": 1, "size": 200 }
```

- 分页 **1-based**（后端 `offset = (page - 1) * size`），前端从 `page=1` 起拉。
- 只返回当前租户内 `status='active'` 且未删除的用户。
- 字段只有 `id` / `nickname`，与 `services/iam-service/openapi.yaml` 的 `UserSimpleListResponse` 一致。
- 权限：与 IAM 其他端点同走 `Authorization: Bearer`（壳的 `http` 客户端统一注入）。

### 4.2 模块划分

| 文件 | 职责 |
|---|---|
| `frontend/src/shell/api/users.ts` | `listUsersSimple(params)` —— 唯一的后端调用点 |
| `frontend/src/shell/store/user-directory.ts` | pinia store `prism-user-directory`：`ensureLoaded()` / `nameOf(id)`；另有 `shortUserId(id)` 工具函数 |
| `frontend/src/shell/components/UserLabel.vue` | 展示组件，props：`userId`、`fallback` |
| `frontend/src/shell/components/index.ts` | 导出 `UserLabel`（壳组件统一出口） |

### 4.3 加载策略

1. **懒加载**：组件 `onMounted`（或 `userId` 变化）时才触发拉取，不在应用启动时打扰。
2. **单飞去重**：并发调用复用同一个 in-flight Promise——列表页同一个 userId 出现在多行也只打一次后端。
3. **分页合并**：`PAGE_SIZE = 200`，循环拉完所有页后一次性写入 `names`。
4. **失败软化**：请求失败吞掉异常，保持空目录并允许下次调用重试；渲染层永远不因昵称解析失败而报错。

### 4.4 文案与降级

| 场景 | 展示 | 文案来源 |
|---|---|---|
| 命中目录 | 昵称 | 后端 `nickname` |
| 未命中目录 | `用户 #<id>` / `User #{id}` | i18n `common.user`（`{id}` 占位） |
| 无 userId | `—` | 组件内固定占位 |

`dev__` 前缀：`NoOpJwtVerifier` 造出的占位用户 ID 形如 `dev__abc`，回退展示时只保留 `__` 之后的有效部分（`shortUserId`），无前缀则原样返回。

### 4.5 接入点

| 页面 | 列定义改动 | 插槽内容 |
|---|---|---|
| `views/audit/operation-log` | `{ label: t("system.audit.actor"), prop: "user_id", slot: "user" }` | `<UserLabel :user-id="userIdOf(row)" />` |
| `views/system/session` | `{ label: t("system.session.user"), prop: "user_id", slot: "user" }` | `<UserLabel :user-id="userIdOf(row)" />` |

`DataTable` 的列插槽机制是 `column.slot ?? column.prop`；不提供插槽时回退渲染原始字段值，因此加插槽不影响其它列。插槽里的行类型是 `Record<string, unknown>`，`user_id` 用 `userIdOf(row)` 做窄化（只认字符串），避免 `unknown` 直接透传给组件 props。

## 5. 验证

### 5.1 单测

| 文件 | 覆盖点 |
|---|---|
| `shell/components/UserLabel.test.ts` | 按 id 解析昵称 / 同 id 只请求一次 / `dev__` 前缀剥离 / 无前缀原样保留 / 无 id 显示占位且不请求 / 目录分页合并 |
| `shell/views/audit/operation-log/index.test.ts` | 操作人列显示昵称而不是 `userId` |
| `shell/views/system/session/index.test.ts` | 用户列显示昵称而不是 `userId` |

### 5.2 命令

```bash
cd frontend
npx vitest run                                   # 全量单测
npx vue-tsc --noEmit --skipLibCheck              # 类型检查
npx eslint --max-warnings 0 src/shell            # 壳目录 lint
cd .. && node scripts/check-prism-tokens.js      # PRISM token 校验
```

## 6. 已知限制与后续

1. **目录是会话级缓存**：用户改昵称后需刷新页面才更新（未实现主动失效入口）。旧 `ReUserLabel` 提供过 `invalidateUserLabel` / `clearUserLabelCache`，本次未做——壳内尚无"用户信息变更"的广播机制，等有真实需求再补。
2. **全量拉取的规模上限**：请求数随租户用户数线性增长（每 200 人一次）。用户量达到万级时应改为"精确查询 + LRU 兜底"，或推动 §3 方案 C（后端批量端点）。
3. **旧文档未同步**：`docs-site/guides/frontend-components.md` 描述的 `ReUserLabel` 源码已不存在（壳重建时随 pure-admin 一并删除），该文档整体已陈旧，不在本次改动范围内。

## 7. 影响面

- 后端：无改动（复用既有 `/v1/iam/users/simple`）。
- 契约：无改动（OpenAPI / contracts YAML 均不动）。
- 壳运行时：新增一次会话级目录请求（懒触发）；页面本身不阻塞在昵称解析上。
- 下游业务系统：壳拷贝升级后可直接用 `UserLabel`。
