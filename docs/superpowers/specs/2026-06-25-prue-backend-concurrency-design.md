# prue_backend 草稿并发控制 + Active Routes 净化设计

> 日期：2026-06-25
> 范围：`services/prue_backend/`（migration + service + dto + routes + tests）+ 少量前端协作（`frontend/src/views/system/menu`）
> 状态：待用户审阅
> 关联：[overall-roadmap](../plans/2026-06-25-overall-roadmap.md) P1.1 + P1.2

---

## 1. 目标

把 prue_backend 收尾 TODO.md 中的两项已知风险：

1. **P1.1 草稿并发控制**：菜单草稿 `PUT /pure/menu-configs/{id}` 增加基于版本号的乐观锁，避免两个管理员同时编辑同一草稿时后者静默覆盖前者
2. **P1.2 Active Routes 净化**：`GET /get-async-routes` 输出递归剥离 `_menu` 等菜单管理内部字段，避免污染动态路由消费方

完成标志：

- 同一草稿并发 PUT，后写入者拿 409，body 含 server 当前 `routes_etag` + `updated_at`
- `GET /get-async-routes` 响应里任何节点 `meta._menu` 都不出现；草稿 / 详情接口保持 `_menu` 不变（菜单管理 UI 依赖它）
- 单元测试覆盖：版本不匹配返回 409；递归净化在 3 层嵌套上仍然干净
- 前端编辑面板捕获 409 → 提示「他人已修改，请刷新」（轻量改动；不在本 spec 强制范围）

---

## 2. 当前事实

- 表 `menu_configs` 已有 `version BIGINT NOT NULL`（**全局自增 / UNIQUE，作为业务版本号**，不是行版本号；不能复用做乐观锁）
- `UPDATE menu_configs SET name = ?, description = ?, routes_json = ? WHERE id = ? AND deleted_at IS NULL` — 无并发保护
- `MenuConfigDetail` 已经回写 `updated_at` 给前端，但 PUT 请求体 `UpdateMenuConfigRequest` 不含任何版本/etag 字段
- `active_routes()` 直接把存的 `routes_json` 反序列化原样返回，没有任何字段过滤
- `_menu` 字段被前端写入 routes_json 的 `meta._menu`，存了 `id` / `parentId` / `menuType` 给菜单管理 UI 用，但动态路由消费方不需要

---

## 3. 数据模型

新增一列 `routes_etag CHAR(32) NOT NULL`：MD5（routes_json） 在每次 create / update / publish / clone 时由后端计算并写入；客户端 PUT 时回传 If-Match-Etag 字段。

> 为什么用 etag 而不是 `row_version` 整数：菜单 JSON 结构复杂，etag 既能做并发锁，又能作为缓存校验头复用（未来给 `/get-async-routes` 配 `ETag` header 自然衔接）。同时不与现有的业务 `version` 字段语义冲突。

### Migration

新增 `services/prue_backend/migrations/{mysql,sqlite}/V002__routes_etag.sql`：

**MySQL**：
```sql
ALTER TABLE menu_configs ADD COLUMN routes_etag CHAR(32) NOT NULL DEFAULT '' AFTER routes_json;
UPDATE menu_configs SET routes_etag = MD5(CAST(routes_json AS CHAR)) WHERE routes_etag = '';
```

**SQLite**（无 MD5 内建，先加列；服务启动时 backfill 用 application-side MD5）：
```sql
ALTER TABLE menu_configs ADD COLUMN routes_etag TEXT NOT NULL DEFAULT '';
```

启动 `db::ensure_schema` 末尾：扫描 `routes_etag = ''` 的行，应用层算 MD5 回填一次（单次启动一次，幂等）。

### 修改 `MenuConfigRow`、`MenuConfigDetail`、`MenuConfigSummary`

三个结构体均加 `pub routes_etag: String`。`MenuConfigDetail` 序列化为 JSON 时字段名 `routesEtag`（沿用 pure-admin camelCase 约定；若现有用 snake_case，与之保持一致 — 验证现有 detail 输出后定）。

---

## 4. 接口契约

### `PUT /pure/menu-configs/{id}`

请求体新增可选字段 `if_match_etag`（snake_case，对齐项目已有 routes_json 风格）：

```json
{
  "name": "...",
  "description": "...",
  "routes_json": [...],
  "if_match_etag": "9f2a..."
}
```

服务端行为：

| 情况 | 状态码 | 响应 |
|---|---|---|
| `if_match_etag` 缺失 | 400 | `{"code":400,"message":"if_match_etag is required for updates","data":null}` |
| `if_match_etag` 与 server 当前不一致 | 409 | `{"code":409,"message":"draft was modified by someone else","data":{"current_etag":"...","updated_at":"..."}}` |
| status ≠ draft（已有逻辑） | 409 | 沿用现有 `only draft menu configs can be updated` |
| 一致 | 200 | 返回新的 `MenuConfigDetail`（含新 etag） |

> create / clone 自动算 etag 写入，前端拿到的 detail 即首次 etag；publish 不改 routes_json 不变 etag。

### `GET /get-async-routes`

输出新增递归净化：把 routes 树每一节点 `meta` 中的 `_menu` 字段剥离（如果存在）。

```jsonc
// before
{ "path": "/sys", "meta": { "title": "系统", "_menu": { "id": 1, "menuType": 0 } } }
// after
{ "path": "/sys", "meta": { "title": "系统" } }
```

- 节点没有 `meta` 字段或 `_menu` 字段时 → 原样返回
- 递归处理 `children`
- **只在 `GET /get-async-routes` 净化**；`GET /pure/menu-configs/{id}` 与 list 保留 `_menu` 给菜单管理 UI

---

## 5. 代码改动范围

**新增**：
- `services/prue_backend/migrations/mysql/V002__routes_etag.sql`
- `services/prue_backend/migrations/sqlite/V002__routes_etag.sql`
- `services/prue_backend/src/service/etag.rs` — `pub fn compute(value: &Value) -> String`（MD5 over canonical JSON）
- `services/prue_backend/src/service/sanitize.rs` — `pub fn strip_internal_fields(value: &mut Value)`

**修改**：
- `services/prue_backend/src/dto/menu_config.rs` — 加 `routes_etag` 字段到 Row/Detail/Summary；`UpdateMenuConfigRequest` 加 `if_match_etag: Option<String>`
- `services/prue_backend/src/service/menu_config_svc.rs`：
  - `create` / `update` / `clone_config`：写入前 compute etag
  - `update`：读取行后比较 `req.if_match_etag` 与 `existing.routes_etag`；不一致返回 `AppError::Conflict` 含 etag 详情
  - `active_routes`：复用 `sanitize::strip_internal_fields`
  - 所有 SELECT / INSERT / UPDATE 加 `routes_etag` 列
- `services/prue_backend/src/db.rs`（`ensure_schema`）：sqlite 启动 backfill 一次
- `services/prue_backend/src/error.rs`：新增 `AppError::EtagConflict { current_etag: String, updated_at: chrono::DateTime<Utc> }` 变体（旧的 `Conflict(String)` 保留给 status 非 draft 的语义）；`IntoResponse` 把 EtagConflict 的 payload 放进 body `data` 字段
- `services/prue_backend/tests/menu_config_service_test.rs`（新增集成测试 sqlite）：
  - happy path: create → update with right etag → 200，etag 改变
  - conflict path: create → update with stale etag → 409，data.current_etag 匹配 server
  - missing etag path: 400
  - active routes 净化：write 含 `_menu`、读 active 不含 `_menu`

**前端**（小补丁，本 spec 内不强制，纳入收尾任务）：
- `frontend/src/api/menu-configs.ts`：PUT 请求带上当前 detail 的 `routes_etag` 作为 `if_match_etag`
- `frontend/src/views/system/menu/utils/hook.ts`：捕获 409 时弹 `ElMessageBox.confirm("草稿已被他人修改，请刷新后再保存")`

---

## 6. 错误处理

| 场景 | 处理 |
|---|---|
| `if_match_etag` 缺失 | 400 `validation` |
| etag 不匹配 | 409，payload `{current_etag, updated_at}` |
| 草稿状态不是 `draft` | 409，message `only draft menu configs can be updated`（沿用） |
| SQLite backfill 出错 | 启动时 warn 但不致命；下次 PUT 仍会要求 etag —— 但 backfill 失败的行 etag 还是 ""，会让所有 PUT 拿到 409。**缓解**：backfill 失败时记 error 日志并 panic，确保 schema 一致 |

---

## 7. 测试策略

### 单元
- `etag::compute` 同一 `Value` 多次结果一致；对象 key 重排顺序后结果**相同**（用 canonical JSON：`serde_json::to_value` + sorted keys 序列化）
- `sanitize::strip_internal_fields`：
  - 单层 `meta._menu` 被剥
  - 嵌套 `children[].meta._menu` 被剥
  - 无 `meta` 节点保持原样
  - `_menu` 同级的 `title` 等不受影响

### 集成（sqlite, testcontainers 不必要 — 用 in-memory）
- create → routes_etag 非空
- update(if_match_etag = old_etag) → 200，新 etag ≠ old
- update(if_match_etag = "garbage") → 409
- update(无 if_match_etag) → 400
- get-async-routes 输出不含 `_menu`
- get-detail 输出仍含 `_menu`

### 验收 Demo
```bash
node scripts/run-tests-docker.js prue_backend     # 全 PASS
# 启动 prue_backend 后：
curl -X PUT /pure/menu-configs/<id> -d '{"if_match_etag":"stale"}'   # 期望 409
curl /get-async-routes | jq '..|.meta?._menu? // empty'              # 期望空输出
```

---

## 8. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 旧前端不传 `if_match_etag` → 全部 PUT 400 | 前端同 PR 改造；后端预留 1 个版本的兼容期 |
| MD5 性能：100KB routes_json × 多请求 | MD5 在 Rust 极快（~1µs/100KB），可忽略 |
| canonical JSON 排序不一致 → 同 payload 不同 etag | 用 `serde_json::Value` → `serde_json::to_value(BTreeMap<String, Value>)` 显式按 key 排序 |
| pure-admin UI 依赖 `_menu` 出现在 active routes | 已确认菜单管理 UI 读 detail 接口，不读 get-async-routes |
| 同时改两件事 → blast radius 扩大 | 拆 2 个 Task：Task A 加 etag 锁；Task B 加净化；分两个 commit，便于回滚 |

---

## 9. 不在范围

- 节点级 patch 替代整体 PUT（未来菜单编辑器二期）
- ETag HTTP header / `If-Match` HTTP header（仅用 JSON body 字段，规避 axum 中间件改动）
- 前端冲突合并 UI（仅简单提示）
- 其它服务的并发控制审计（专题）

---

## 10. 验收清单

- [ ] migration V002 在 MySQL / SQLite 双方都跑过一次幂等
- [ ] PUT 接受 `if_match_etag`、拒绝 stale、空 etag 拿 400
- [ ] Conflict payload 含 `current_etag` + `updated_at`
- [ ] active routes 输出 0 处 `_menu`
- [ ] detail / list 仍含 `_menu`（菜单管理 UI 不受影响）
- [ ] 集成测试 4 条全过
- [ ] `node scripts/run-tests-docker.js prue_backend` PASS
- [ ] TODO.md 中对应两条 移除（任务已闭环）
