# ADR-003: Multi-tenancy Architecture

> 日期：2026-07-23
> 状态：已采纳（追认 Phase 8.1–8.3 决策）
> 关联：
> - [Phase 8.1 skeleton](../superpowers/specs/2026-07-01-phase8.1-multi-tenancy-skeleton-design.md)
> - [Phase 8.2 tenant business tables](../superpowers/specs/2026-07-03-phase8.2-tenant-business-tables-design.md)
> - [Phase 8.3 tenant hardening](../superpowers/specs/2026-07-04-phase8.3-tenant-hardening-design.md)

## 上下文

从单租户 SaaS 骨骼线转向多租户 SaaS 平台时，需要一次性决定：租户识别粒度、SQL 隔离策略、编译期 vs 运行期检查、admin 越权模型。7 月 Phase 8.1–8.3 完成多租户改造，本 ADR 沉淀关键决策以指导后续新表 / 新服务接入。

## 决议

### 1. 租户身份来源：JWT `tid` claim

- **`tid` 必填**（缺失硬拒 401，无 fallback）。承载于 access token / refresh token / user JWT / service JWT。
- **`ct: true` claim** 标识跨租户能力（platform_admin 用户拥有）。
- Gateway 只做 `tid` 存在性检查，**不解析** request body / path params —— 跨租户过滤由下游 SQL `WHERE tenant_id = ?` 兜底。

### 2. 表 tenant_id 列的三档策略

| 类别 | 规则 | 例子 |
|------|------|------|
| **租户拥有资源** | `tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1` + 索引 | `iam_users`, `iam_roles`, `iam_org_units`, `ntf_announcements`, `cfg_namespaces`, `file_files`, `audit_operation_logs` |
| **全局资源** | 不加 `tenant_id`（跨租户共享） | `iam_permissions`, `iam_menus`, `prue_menu_configs` |
| **事件流** | `tenant_id BIGINT UNSIGNED NULL`（consumer 容错缺失租户的历史事件） | `audit_events` |

- 唯一键必须包含 `tenant_id`（如 `UNIQUE KEY (tenant_id, name)`）。
- **跨服务不建 FK**：仅 `iam_*` 表 FK → `iam_tenants(id)`；其他服务用裸列 + 索引（微服务边界）。

### 3. 编译期 tenant 保护：`TenantScope` enum

`libs/jwt-ctx/src/scope.rs`：

```rust
pub enum TenantScope {
    Single(i64),      // 普通用户
    CrossTenant,      // platform_admin（Principal.cross_tenant=true）
}
```

`libs/common-db/src/scope.rs` 提供 `QueryBuilder::and_tenant(&scope)`：
- `Single(tid)` → 追加 `AND tenant_id = <tid>`
- `CrossTenant` → no-op（不追加过滤）

Handler 拿 `TenantScope` extractor → 强制传给 service 层。CI lint（`scripts/check-tenant-scope.js`）扫描 handler 拿了 `TenantScope` 但 SQL 无 `tenant_id` 的情况，exit 1 阻断 push。

### 4. Roles 拆分：全局 permissions + 租户内 roles

- `iam_permissions`：**全局**（权限码定义全平台统一，如 `iam.user.create`）
- `iam_roles`：**租户内**（每租户可自定义 role → permission 映射）
- 注册流程：新租户注册时同事务创建 per-tenant `tenant_admin` 角色 + 绑定全部全局权限 + 赋给注册用户
- `platform_admin` 角色是**全局角色**，非租户内，签发时 JWT 带 `ct: true`

### 5. 权限查询链带 tenant 过滤

`get_claims` 端点（token-service 刷新时调）3 条 SQL 全部 `AND tenant_id = ?` + Query 参数 `?tenant_id=`。防御性深度：即使 mesh intentions 鉴权可信，业务层也拒绝跨租户 user_id lookup。

## 后果

### 好处

- **数据泄露防线三层**：JWT `tid` → `TenantScope` extractor → SQL `AND tenant_id = ?`。任一层挡住即无泄露。
- **编译期保证**：忘调用 `and_tenant` → 编译不过（因为 `TenantScope` 必须传给 service）。
- **平台运营能力**：`platform_admin` 可跨租户操作（客户支持、审计）。

### 代价

- 所有新表默认要加 `tenant_id`，否则会被 `check-tenant-scope.js` / code review 拦
- Backfill 老数据到 `DEFAULT 1` 无历史数据可保留（**开发期决定，无生产数据**）
- 跨租户日志（`audit_events`）用 `NULL tenant_id` 特例；消费侧要容错
- `check-tenant-scope.js` 是启发式扫描，可能误报（需 `check-tenant-scope: delegated` 豁免注释）

### 已知未做

- gateway 路由感知的租户检测（放弃：D1 已决策由 SQL 兜底）
- 跨租户审计（记录 platform_admin 访问了哪租户）—— 未来安全增强
- 租户计费 / quota / 生命周期 —— 产品形态未定

## 变更时机

- **新增租户拥有的业务表**：必须遵循 §2 分类，并加索引 + tenant-included unique key
- **新 handler**：必须 extractor 拿 `TenantScope`，service 层用 `and_tenant`
- **新 JWT 签发路径**：必须携带 `tid`（缺失即 401）
- **新 event envelope**：必须携带 `tenant_id`
