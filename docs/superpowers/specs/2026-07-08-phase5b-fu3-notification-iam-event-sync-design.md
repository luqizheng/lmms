# Phase 5B-fu3 — notification → iam 事件化（本地 user membership 投影）

> 日期：2026-07-08
> 来源：`TODO.md` Phase 5-B 后续项「5B-fu3 notification → iam 事件化」，对应
> phase5b-jwt spec §0.5.6 #2 的 `phase5b-event-sync` follow-up；phase5a-2
> deprecate-consul-connect spec §0.5.6 已显式 defer 「否，本 spec 范围外」。
> 前置：Phase 4 事件总线（iam.user.created / iam.permission.changed 已闭环）、
> Phase 4.4（iam.permission.changed 已经有 tx-in outbox + consumer 兜底模式）、
> Phase 5B（service-jwt + JWKSCache）— iam `/iam/internal/*` 鉴权已就位。
> 范围：notification-service 移除同步 HTTP `by-{role,dept}` 调用，改 RabbitMQ
> 事件驱动本地投影；iam-service 新增 3 类 membership 事件并在所有写点同事务
> outbox emit；保留旧的 `/iam/internal/users/by-{role,dept}` 端点（deprecated，
> 不删，避免破坏现有调用方 / 灰度）。
>
> **已锁定的设计决策**（brainstorm 2026-07-08）：
> - **D1** 三个新事件：`iam.user.role.assigned` / `iam.user.role.unassigned` /
>   `iam.user.dept.changed`；不复用 `iam.permission.changed`（后者只携带
>   user_ids 不带 membership 实际 diff，consumer 还得回 iam 查 — 没解决问题）。
> - **D2** notification 本地投影单表 `ntf_user_projection`（user_id PK，
>   role_ids JSON，dept_id，tenant_id），不引入 Redis；publish_announcement
>   走本地 SQL 解析 role/dept → user_id 集合，与 4.3-fu1 batch insert 路径衔接。
> - **D3** Bootstrap：notification 启动一次性 `GET /iam/internal/membership-bootstrap`
>   新端点（一次性返回全量 user→role/dept 映射，server-side 已合 multi-page），
>   替换投影表内容；之后纯靠事件增量更新。
> - **D4** 事件 emit 复用 `perm_change.rs::enqueue_permission_changed` 模式
>   （tx-in outbox + commit 后无副作用），emit 点集中加在现有 user_roles /
>   role_svc / org_svc / tenant_svc 写函数尾。
> - **D5** Consumer durable queue `notification.membership-events`，绑
>   routing-key `iam.user.role.#` + `iam.user.dept.#`；幂等键 = event_id
>   （已由 common-event envelope 自带），重复投递无副作用。
> - **D6** Out-of-order 容错：事件 payload 带 server-side `ts`（Unix 秒），
>   consumer 比对 `ntf_user_projection.last_event_at`，旧事件不覆盖新值。
> - **D7** HTTP 端点 `/iam/internal/users/by-{role,dept}` **保留 deprecated**
>   （不立即删）：bootstrap 端点是新增；现有调用方（仅 notification）切完后，
>   下一 phase（Roadmap 2.0 或独立清理 phase）再删。

---

## 1. 背景与问题

notification-service 当前 `publish_announcement` 解析 role/dept 接收人范围走同步 HTTP：

```
publish_announcement(ann_id, iam_client, scope)
  → iam_client.expand_scopes(scopes, scope)
  → GET http://iam-service:8081/iam/internal/users/by-role/{role_id}?size=1000&tenant_id={tid}
  → GET http://iam-service:8081/iam/internal/users/by-dept/{dept_id}?size=1000&tenant_id={tid}
  → HashSet<user_id>  → 批量写 ntf_announcement_deliveries
```

每个 role/dept scope 一次 HTTP 调用。规模问题 + 一致性问题 + 依赖问题：

- **依赖**：notification 强依赖 iam 在线；iam 短暂抖动 → publish 失败（已通过
  ResilientClient + Phase 5-A 网关减轻，但 RTT 与失败窗口仍在）。
- **一致**：fan-out 时如果 user→role 在 publish 期间被改，HTTP 拿到的是当前
  snapshot（事件可能后到），产生已发出公告但 user 不在 role 的「迟到」情况；
  反过来 publish 后才改 role，事件先到 HTTP 后到，announcement 漏送。
- **规模**：100 个 role/dept scope → 100 次 HTTP；4.3-fu1 已优化 DB 写入端，
  HTTP 仍是瓶颈。
- **拓扑**：phase5a-2 撤 Consul Connect 后服务间直连 + 应用层 auth；同步 HTTP
  → 事件化的迁移是 phase5a-2 §0.5.6 #2 显式 defer 的 follow-up。

---

## 2. 目标 / 非目标

### 2.1 目标

1. **去掉 notification → iam 同步 HTTP**：publish_announcement 解析 role/dept
   完全走本地 SQL 投影表，无任何跨服务调用。
2. **iam 新增 3 类 membership 事件**：`iam.user.role.assigned` /
   `iam.user.role.unassigned` / `iam.user.dept.changed`，覆盖所有写点。
3. **Bootstrap 端点**：iam 新增 `GET /iam/internal/membership-bootstrap`，
   一次性返回当前 tenant 的 user→{role_ids, dept_id} 全量映射。
4. **本地投影 + 增量事件**：notification 启动一次 bootstrap，之后消费
   `notification.membership-events` durable queue 增量更新。
5. **保留旧 HTTP 端点**（deprecated）：不破坏其他潜在调用方；下个 phase
   单独清理。

### 2.2 非目标

- 其他服务（audit / config / file）订阅同样事件 — 本期只服务 notification
- Redis 缓存层 — 本期不引入；本地 SQL `WHERE role_ids JSON CONTAINS ? OR dept_id = ?`
  对 ≤10K user 规模足够；性能不达标时再加 Redis（独立 phase）
- 多副本 notification 投影一致性 — 每副本独立消费 RabbitMQ 同 feed，最终一致；
  不做主备选举
- 跨租户广播 — `tenant_id` 是 PK 一部分，notification 启动时锁 tenant，
  跨租户事件被 consumer 直接拒收
- 删 `/iam/internal/users/by-{role,dept}` — 留 deprecated；下 phase 单独
  sweeper

---

## 3. 设计

### 3.1 数据流

```
   ┌──────────────────┐                   ┌──────────────────┐
   │  iam-service     │                   │ notification-svc │
   │                  │                   │                  │
   │ 写路径           │  outbox event     │ consumer         │
   │ assign_user_roles├──────────────────▶│ notification.    │
   │ user_roles.rs    │  iam_outbox_events│  membership-events│
   │ role_svc.rs:271  │  tx-in enqueue    │                  │
   │ role_svc.rs:334  │                   │ idempotent       │
   │ tenant_svc.rs:89 │                   │ apply: UPSERT    │
   │ org_svc.rs       │                   │ role_ids JSON    │
   │ update_org_unit  │                   │                  │
   └──────────────────┘                   │ publish_         │
            │                              │ announcement     │
            │                              │  ↓ local SQL     │
            │  bootstrap                   │  ntf_user_       │
            │  GET /membership-bootstrap  │  projection      │
            │  (启动一次)                  │                  │
            ▼                              └──────────────────┘
   GET /iam/internal/membership-bootstrap
```

### 3.2 新事件（`contracts/events.yaml`）

```yaml
  iam.user.role.assigned:
    schema_version: 1
    description: "用户被授予角色（覆盖：创建用户时 role seed、assign_user_roles、tenant admin seed、role 重启用 iam_user_roles 重建）"
    payload:
      user_id:   { type: string, required: true }
      role_id:   { type: string, required: true }
      tenant_id: { type: int,    required: true }
      actor_id:  { type: string, required: false }
      ts:        { type: int,    required: true }   # Unix 秒，server-side

  iam.user.role.unassigned:
    schema_version: 1
    description: "用户被撤销角色（覆盖：assign_user_roles 替换、role 禁删 cascades DELETE、tenant 注销）"
    payload:
      user_id:   { type: string, required: true }
      role_id:   { type: string, required: true }
      tenant_id: { type: int,    required: true }
      actor_id:  { type: string, required: false }
      ts:        { type: int,    required: true }

  iam.user.dept.changed:
    schema_version: 1
    description: "用户所属组织单元变更（PUT /iam/org-units/:id 或 create user 改 org_unit_id）"
    payload:
      user_id:   { type: string, required: true }
      dept_id:   { type: string, required: false }  # null = 移除
      tenant_id: { type: int,    required: true }
      actor_id:  { type: string, required: false }
      ts:        { type: int,    required: true }
```

跑 `node scripts/contracts-gen.js` 生成 `common_event::events::{IamUserRoleAssigned, IamUserRoleUnassigned, IamUserDeptChanged}` struct + routing-key 常量。

### 3.3 投影表（`services/notification-service/migrations/mysql/V003__ntf_membership_projection.sql`）

```sql
CREATE TABLE IF NOT EXISTS ntf_user_projection (
  user_id        VARCHAR(64) NOT NULL,
  tenant_id      BIGINT      NOT NULL,
  role_ids       JSON        NOT NULL,   -- ["r1","r2"]
  dept_id        VARCHAR(64) NULL,
  last_event_at  BIGINT      NOT NULL,   -- Unix 秒，单调非减
  PRIMARY KEY (user_id),
  KEY idx_tenant_dept (tenant_id, dept_id),
  KEY idx_tenant_role (tenant_id, (CAST(role_ids AS CHAR(64) ARRAY)))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- marker for ensure_schema
CREATE TABLE IF NOT EXISTS ntf_migration_v003_applied (
  applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

注：MySQL JSON index 限制大，role_ids 查询本期走 `JSON_CONTAINS` 全表扫（10K
user 规模够用）；超规模再换独立 `ntf_user_role_index (user_id, role_id)` 拆表。

### 3.4 iam-service emit 点（6 处）

复用 `service/perm_change.rs::enqueue_permission_changed` 模式（tx-in outbox
enqueue），新增 `service/membership_event.rs::enqueue_membership` helper：

| # | 文件 / 函数 | 触发场景 | 事件 |
|---|---|---|---|
| 1 | `service/user_roles.rs:21` (helper) | 用户创建时 role seed | `iam.user.role.assigned` × N |
| 2 | `service/role_svc.rs:271` `assign_user_roles` | PUT /iam/users/:id/roles 增量 | `assigned` × 新增; `unassigned` × 移除 |
| 3 | `service/role_svc.rs:334` `DELETE FROM iam_user_roles WHERE user_id=?` | role 禁删 / 删除 cascade | `iam.user.role.unassigned` × affected |
| 4 | `service/tenant_svc.rs:89` | 租户自助注册 admin seed | `iam.user.role.assigned` × N |
| 5 | `service/org_svc.rs:216/230/238` | org unit 成员变更 | `iam.user.dept.changed` × affected |
| 6 | `routes/org_units.rs:98` `update_org_unit` | PUT /iam/org-units/:id 改 dept 字段 | `iam.user.dept.changed` × affected |

每个 emit 在原 SQL 同事务内调用 `enqueue_membership(tx, repo, &payload)`；
`tx` 是 `&mut Transaction<'_, MySql>`，`repo` 是 `state.outbox_repo.as_ref()`。

`enqueue_membership` helper 形态：

```rust
pub async fn enqueue_role_assigned(
    tx: &mut Transaction<'_, MySql>,
    repo: &MySqlOutboxRepo,
    user_id: &str, role_id: &str, tenant_id: i64,
    actor_id: Option<&str>, ts: i64,
) -> Result<(), IamError> { /* wrap in Envelope + enqueue */ }

// 类似 enqueue_role_unassigned, enqueue_dept_changed
```

### 3.5 iam 新增 bootstrap 端点（`routes/internal_membership.rs`）

```rust
// GET /iam/internal/membership-bootstrap
// service-jwt 鉴权（aud=iam-service），tenant_id 来自 JWT tid claim
// 返回：{ user_id: { role_ids: [...], dept_id: "..." }, ... }
//       全量单租户映射；server-side 合多 page
async fn membership_bootstrap(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> AppResult<Json<MembershipBootstrap>>
```

`MembershipBootstrap { tenant_id: i64, mappings: HashMap<String, UserMembership> }`
`UserMembership { role_ids: Vec<String>, dept_id: Option<String> }`

实现：单次 SQL `SELECT u.id, u.org_unit_id, GROUP_CONCAT(ur.role_id) FROM iam_users u LEFT JOIN iam_user_roles ur ON ur.user_id=u.id WHERE u.tenant_id=? AND u.deleted_at IS NULL GROUP BY u.id`，
server-side 拼 HashMap。10K user 量级，单次查询 < 100ms。

### 3.6 notification consumer（`services/notification-service/src/consumer/membership.rs`）

```rust
const QUEUE_NAME: &str = "notification.membership-events";
// durable queue, 绑 routing-key: iam.user.role.assigned, iam.user.role.unassigned, iam.user.dept.changed
// arguments: x-dead-letter-exchange = events.dlx (复用 Phase 6-A T3 错误分级)

pub async fn run(state: AppState, shutdown: ShutdownRx) -> AppResult<()> {
    let conn = state.event_bus.connect().await?;
    let consumer = subscribe(
        &conn, QUEUE_NAME,
        &["iam.user.role.assigned", "iam.user.role.unassigned", "iam.user.dept.changed"],
        HandlerErrorClass::Transient,  // 走 republish-and-ack 阈值 3
    ).await?;
    while let Some(delivery) = consumer.next().await {
        let envelope: Envelope<serde_json::Value> = parse(&delivery)?;
        apply_membership_event(&state.db, &envelope).await
            .map_err(|e| HandlerError::Transient(e.to_string()))?;
        delivery.ack().await?;
    }
    Ok(())
}

async fn apply_membership_event(
    db: &Db, env: &Envelope<serde_json::Value>,
) -> AppResult<()> {
    let tenant_id: i64 = env.tenant_id()?.parse()?;
    let ts: i64 = env.payload()["ts"].as_i64().unwrap_or(0);
    match env.routing_key() {
        "iam.user.role.assigned" => db.add_role(env.payload()["user_id"], env.payload()["role_id"], tenant_id, ts).await,
        "iam.user.role.unassigned" => db.remove_role(env.payload()["user_id"], env.payload()["role_id"], tenant_id, ts).await,
        "iam.user.dept.changed" => db.set_dept(env.payload()["user_id"], env.payload()["dept_id"], tenant_id, ts).await,
        _ => unreachable!(),
    }
}
```

`Db::add_role/remove_role/set_dept` 三方法：单事务内
`SELECT last_event_at FOR UPDATE → if ts > current then UPSERT/DELETE/INSERT → COMMIT`。
Out-of-order 自动丢弃（D6 决策）。

### 3.7 Bootstrap（`services/notification-service/src/service/projection.rs`）

```rust
pub async fn bootstrap(db: &Db, iam_client: &IamBootstrapClient) -> AppResult<()> {
    let snap = iam_client.fetch_membership_bootstrap().await?; // 一次性 HTTP
    let mut tx = db.pool.begin().await?;
    // 全表 DELETE 然后批量 INSERT（10K 行级别）
    sqlx::query("DELETE FROM ntf_user_projection WHERE tenant_id = ?")
        .bind(snap.tenant_id).execute(&mut *tx).await?;
    for (uid, m) in snap.mappings {
        sqlx::query("INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) VALUES (?,?,?,?,?)")
            .bind(&uid).bind(snap.tenant_id)
            .bind(serde_json::to_string(&m.role_ids)?)
            .bind(&m.dept_id).bind(0_i64)
            .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
```

启动时 `main.rs::main()` 在事件 consumer spawn 之前调用一次（顺序：
bootstrap 完 → consumer start → consumer 从此刻之后的 event 开始消费，
可能漏「bootstrap 中途发生的 event」；mit: bootstrap 后再做一次短窗 grace
period 30s，事件正常 apply；后续如有 conflict last_event_at 兜底）。

### 3.8 `publish_announcement` 改造

`db.rs::publish_announcement` 当前：

```rust
let user_ids: HashSet<String> = if scopes.is_empty() {
    HashSet::new()
} else {
    iam_client.expand_scopes(&scopes, scope).await?  // ← 同步 HTTP
};
```

改为：

```rust
let user_ids: HashSet<String> = if scopes.is_empty() {
    HashSet::new()
} else {
    db.resolve_users_by_scopes(&scopes, scope).await?  // ← 本地 SQL
};
```

`Db::resolve_users_by_scopes` 实现：

```rust
pub async fn resolve_users_by_scopes(
    &self, scopes: &[(ScopeType, String)], scope: &TenantScope,
) -> AppResult<HashSet<String>> {
    let mut out = HashSet::new();
    for (t, id) in scopes {
        match t {
            ScopeType::User => { out.insert(id.clone()); }
            ScopeType::Role => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT user_id FROM ntf_user_projection \
                     WHERE tenant_id = ? \
                       AND JSON_CONTAINS(role_ids, JSON_QUOTE(?))",
                ).bind(scope.tenant_id().unwrap_or(1)).bind(id)
                .fetch_all(&self.pool).await?;
                out.extend(rows.into_iter().map(|(s,)| s));
            }
            ScopeType::Dept => {
                let rows: Vec<(String,)> = sqlx::query_as(
                    "SELECT user_id FROM ntf_user_projection \
                     WHERE tenant_id = ? AND dept_id = ?",
                ).bind(scope.tenant_id().unwrap_or(1)).bind(id)
                .fetch_all(&self.pool).await?;
                out.extend(rows.into_iter().map(|(s,)| s));
            }
        }
    }
    Ok(out)
}
```

**TenantScope**：仅 `Single(tid)` 路径走本地投影；`CrossTenant`（platform admin）
保留旧的 HTTP `expand_scopes`（platform admin 看跨租户，本地投影只覆盖当前
tenant）。

### 3.9 灰度与回滚

- **Phase 1**（本期实施）：本地投影就绪 + consumer 启动 + bootstrap 完成；
  publish_announcement 走本地投影。**HTTP 端点保留 deprecated**（spec 不删，
  bootstrap 用到，但 `expand_scopes` 不再被 publish_announcement 调用）。
- **Phase 2**（下个清理 phase）：HTTP 端点加 `#[deprecated]` + 24h warn log
  无调用 → 删 endpoint + 删 IamMemberClient HTTP 方法。

回滚：本期不删 HTTP endpoint，回滚只需 git revert；consumer / projection 表
保留但不消费（`publish_announcement` 临时改回 expand_scopes）。

---

## 4. 错误处理

| 失败 | 处理 |
|---|---|
| bootstrap HTTP 失败 | 启动失败（fail-fast）—— 没投影不能 publish；rediness 探针不通过 |
| consumer transient（DB 抖动） | Phase 6-A T3 republish-and-ack 阈值 3 → DLQ |
| consumer permanent（事件 schema 错） | ack 后 warn + drop（事件无法解析就不影响投影） |
| out-of-order event | 静默丢弃（D6 `ts` 比对），warn log |
| 投影表 schema 不一致（V003 未跑） | bootstrap 启动检查 `ntf_migration_v003_applied` 表存在；缺失则 fail-fast |
| 跨 tenant event | consumer 启动时锁 tenant_id；payload tenant_id 不匹配 → ack + warn drop |

---

## 5. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 启动期间 event 丢失（bootstrap 30s 间隙） | bootstrap 完成 → grace 30s 双消费（consumer + 短窗重放 outbox） ；last_event_at 兜底后续乱序 |
| 多副本 notification 同时 bootstrap | 每副本独立 bootstrap → 临时可能数据不一致；consumer 启动后增量事件收敛到稳态 |
| iam 删用户未发 event | `iam.user.deleted` 已存在（Phase 4）；consumer 增加：删除事件也清 `ntf_user_projection` 该 user_id 行 |
| iam 删角色未发 event | role 禁删/删除路径加 `enqueue_role_unassigned` × affected user_ids（第 #3 emit 点已覆盖） |
| 投影表膨胀（user 全历史留存） | 删 user 事件同步清行；dept 改空不变 NULL；不需 cron 清理 |
| JSON_CONTAINS 性能 | 10K user 量级实测 < 50ms；超规模独立 phase 拆表 + 加索引 |
| 测试覆盖不足 | 5 个 integration test（bootstrap / apply assigned / apply unassigned / apply dept / out-of-order）+ live e2e `test-case/5b-fu3-event-sync.md` |

---

## 6. 不在本期范围（follow-up，文档记录）

- 删 `/iam/internal/users/by-{role,dept}` 端点（灰度观察期后）
- 删 `IamMemberClient::{by_role, by_dept, expand_scopes}`（通知下游全切完）
- 其他服务（audit / config）订阅 membership 事件做本地视图
- Redis 缓存层（投影 SQL 不够快时）
- 多副本投影合并（CRDT / leader election）

---

## 7. 实现路线（Task 列表，→ writing-plans 出详细 plan）

| T# | 内容 | 涉及文件 |
|---|---|---|
| T1 | contracts/events.yaml 加 3 事件 + codegen + common-event 类型 | contracts/events.yaml, libs/common-event |
| T2 | iam 新增 `enqueue_role_assigned/unassigned, enqueue_dept_changed` helper + 6 emit 点 | services/iam-service/src/service/membership_event.rs（新） + 6 处 call site |
| T3 | iam 新增 `GET /iam/internal/membership-bootstrap` 端点 | services/iam-service/src/routes/internal_membership.rs（新）+ openapi.rs + 测试 |
| T4 | notification migration V003 + Db::projection 方法 | services/notification-service/migrations/mysql/V003__*.sql + src/db.rs（projection 方法） |
| T5 | notification consumer `consumer/membership.rs` + durable queue 声明 + 启动 wiring | services/notification-service/src/consumer/membership.rs（新）+ main.rs + infra definitions.json |
| T6 | notification bootstrap service + `IamBootstrapClient` + 启动顺序 | services/notification-service/src/service/projection.rs（新）+ iam_client.rs（新增 fetch_membership_bootstrap 方法） |
| T7 | `publish_announcement` 改走 `Db::resolve_users_by_scopes` | services/notification-service/src/db.rs |
| T8 | tests（unit + integration testcontainers）+ live e2e `test-case/5b-fu3-event-sync.md` | services/notification-service/tests/membership_projection_test.rs（新） + test-case/ |
| T9 | deprecate `IamMemberClient::{by_role, by_dept, expand_scopes}`（加 `#[deprecated]` 注释，不删） | services/notification-service/src/iam_client.rs |
| T10 | docs：USER_MANUAL / ai-context / observability 更新 | docs/ |

预计 10-12 个 commit，每个 commit 独立可测；CI gate 全过；lib tests + integration tests 都覆盖。

---

## 8. spec review checklist

- [x] 决策点 D1-D7 有答案 + 理由
- [x] 否决选项（复用 iam.permission.changed vs 新事件）有反对论据
- [x] 「不做什么」显式列出（§2.2 / §6）
- [x] 改动影响面按文件列（§3.4 / §3.6 / §3.7）
- [x] 验证步骤可执行（§3.7 bootstrap + §3.8 publish_announcement 改造 + §7 T8）
- [x] 错误处理覆盖（§4）
- [x] 风险与缓解成对（§5）

---

## 9. 关联

- 上游 spec：`2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md`
  §0.5.6 #2 显式 follow-up；`2026-07-07-deprecate-consul-connect-use-registrator-design.md`
  §0.5.6 决策「否，本 spec 范围外」
- 同模式参考：Phase 4.4 `iam.permission.changed`（tx-in outbox + consumer）、
  Phase 4.1 `notification.iam-events`（durable queue + 幂等）
- 灰度落点：下个 phase（Roadmap 2.0 或独立清理 phase）删 HTTP endpoint +
  IamMemberClient HTTP 方法