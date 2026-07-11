# config-service list 端点 tenant_id filter SQL 拼装修复 — 设计

日期：2026-07-11
状态：approved
服务：services/config-service
影响：`GET /v1/config/namespace`（list）、`cfg_namespaces` 表
触发：用户反馈 frontend /config/namespace 页面"添加 namespace 出错"，实为 list 接口 500

## 背景

`services/config-service/src/services/config.rs:59-75` 中 `query_namespaces` 用
`sqlx::QueryBuilder` 构造 list 查询，并以 `libs/common-db::WithTenantScope::and_tenant`
追加 `AND tenant_id = ?`。`and_tenant` 不负责前置 `WHERE`，而 `query_namespaces` 的
初始 SQL 是 `SELECT * FROM cfg_namespaces` —— 没有 `WHERE` 子句。最终被拼成：

```sql
SELECT * FROM cfg_namespaces  AND tenant_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?
```

MySQL 报语法错误 → service 返回 `Err` → 路由 `.map_err(|_| (500, "query failed"))`
吞错成字面量 `"query failed"`。任何 tenant-scoped 调用方（绝大多数用户）都会命中。

## 影响与诊断特征

- 现象：`GET /api/v1/config/namespace` 返回 HTTP 500 + body `"query failed"`。
- 前端体感：`onCreate()` 成功后调 `onSearch()`，列表刷新失败 → toast 显示"创建失败"。
  实际 POST 是 200，DB 已写入，只是列表看不到新增。
- 不命中：JWT `cross_tenant=true` 时 `and_tenant` 是 no-op，sqlx 不会拼 `AND`，
  故 **platform_admin 不会触发**。
- 回归测试盲区：`tenant_isolation_test.rs` 只覆盖 `GET /config/namespace/:id`
  单条端点，从未命中 list 端点，故 CI 自 V002 加 `tenant_id` 以来未拦下。

## 修复

最小改动，将 `query_namespaces` 的初始 SQL 改为 `WHERE 1=1`，与同文件 sibling
`query_config_items` / `query_versions` 保持一致：

```rust
// before
let mut qb = QueryBuilder::new("SELECT * FROM cfg_namespaces");

// after
let mut qb = QueryBuilder::new("SELECT * FROM cfg_namespaces WHERE 1=1");
```

后续 `qb.and_tenant(scope)` 仍然追加 ` AND tenant_id = ?`，最终 SQL 合法。

不动：

- `and_tenant` helper 本体（已有 `query_config_items` 走 `WHERE 1=1 + AND` 模式，
  helper 设计假设调用方自管前置，本修不破坏假设）。
- `create_namespace`（INSERT 不需要 tenant_id 过滤，由 INSERT 显式绑定）。
- 单条端点 `get_namespace` / `update_namespace` / `delete_namespace`（用
  `where_id` + `and_tenant`，`where_id` 内部自己拼 `WHERE id = ?`，是合法的）。

## 测试

在 `services/config-service/tests/tenant_isolation_test.rs` 补一条 list 端点的
multi-tenant 隔离断言：

1. seed 两条 namespace 分别属于 tenant 1 / tenant 2。
2. tenant-1 non-cross 用户 GET `/config/namespace?page=1&size=20`：
   - 状态 200。
   - 返回的 `items` 全部 `tenant_id=1`，不出现 tenant=2 的那条。
   - `total` 是 tenant-1 namespace 的实际计数（修复前该请求 500）。
3. tenant-1 用户若没有 namespace：返回 `items=[]` 200。

跑测：

```bash
node scripts/run-tests-docker.js config-service
```

## 验收标准

- `cargo test -p config-service` 全绿，新加测试通过。
- 手动复现原始 curl：返回 200 + 包含 tenant-1 namespace 的 JSON 数组。
- 现有 `tenant_isolation_test::platform_admin_cross_tenant_namespace` 仍然通过。

## 不做的事

- 不修改 `and_tenant` helper 接口。
- 不改任何前端代码（前端写的是对的，问题在后端）。
- 不引入新依赖。
- 不写 migration（schema 已正确，问题在 SQL 拼装）。
- 不调整 `cfg_namespaces` 表上的索引（`idx_cfg_namespaces_tenant` 已存在）。

## 风险

- 修复纯字符串常量，无运行时风险。
- `WHERE 1=1` 是项目内已有惯例（`query_config_items` / `query_versions`），
  MySQL 优化器会当作无关条件丢弃，对查询计划无影响。
