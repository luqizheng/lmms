# 数据库 Schema State 端点 — 设计

**日期**：2026-07-08
**状态**：草案 · 待评审
**关联**：TODO.md §Phase 10 `数据库 schema 当前版本/历史查询端点（/internal/migrations/state）`

---

## 0. 一句话

> 给所有 7 个 Rust 服务统一暴露 `GET /<svc>/internal/migrations/state` 端点，返回该服务的 DB schema 状态（applied / available / in_sync / missing / extra）。统一在 `libs/common-db` 抽 `MigrationState` trait，避免每个服务重复实现 marker-table 探测。

---

## 1. 现状

### 1.1 各服务迁移机制（全 marker-table 模式，未用 sqlx 内置）

| 服务 | migration 目录 | 数量 | marker 模式 | 位置 |
|------|----------------|------|-------------|------|
| iam | `migrations/mysql/V00X__*.sql` | 5 (V001-V005) | `iam_users` / `iam_outbox_events_archive` / `iam_tenants` / `iam_migration_v00{4,5}_applied` | `db.rs::ensure_schema` |
| token | `migrations/mysql/V001__init.sql` | 1 | `token_oauth_clients` | `db.rs::ensure_schema` |
| notification | `migrations/mysql/V00X__*.sql` | 3 (V001-V003) | `ntf_announcements` / `ntf_migration_v00{2,3}_applied` | `db.rs::ensure_schema` |
| audit | `migrations/mysql/V001__init.sql` | 1 | `audit_operation_logs` | `main.rs::ensure_schema` (inlined) |
| config | `migrations/mysql/V00X__*.sql` | 2 (V001-V002) | `cfg_namespaces` / `cfg_migration_v002_applied` | `db.rs::ensure_schema` |
| file | `migrations/mysql/V00X__*.sql` | 2 (V001-V002) | `file_files` / `file_migration_v002_applied` | `db.rs::ensure_schema` |
| prue_backend | `migrations/{mysql,sqlite}/V001__init.sql` | 1×2 双 dialect | `prue_menu_configs` | `db.rs::ensure_schema` |

### 1.2 痛点

- **没有 schema 版本可见性**：运维 / 排障时不知道"某个服务现在跑在哪个 schema 版本"
- **schema drift 不可观测**：dev / staging / prod 三套环境的 schema 可能漂移，没人能集中查询
- **回滚决策无依据**：rollback 一个 migration 前需要知道目标 schema 是哪个版本
- **CI 不能校验 schema 一致性**：prue_backend 双方言同步靠人眼

---

## 2. 目标 API

### 2.1 端点契约

```
GET /<svc>/internal/migrations/state
  Headers:
    Authorization: Bearer <service-jwt>   # 必须, Phase 5-B 标准
  Response 200:
    {
      "service": "iam-service",
      "dialect": "mysql",                   # "mysql" | "sqlite" | "mysql+sqlite"
      "available": [                         # migrations/ 目录里有
        { "version": "V001", "name": "init" },
        { "version": "V002", "name": "outbox_archive" },
        ...
      ],
      "applied": [                           # 通过 marker table 探测出
        "V001", "V002", "V003"
      ],
      "current_version": "V003",
      "in_sync": true,                       # applied == available
      "missing": [],                         # available - applied
      "extra": []                            # applied - available (理想空)
    }

  Response 503 (DB 不可达):
    { "service": "iam-service", "error": "db_unreachable" }
```

### 2.2 路由前缀

按现有 `/internal/*` 约定（`iam/internal/*` 已在用）：
- `GET /iam/internal/migrations/state`
- `GET /token/internal/migrations/state`
- `GET /ntf/internal/migrations/state`
- `GET /audit/internal/migrations/state`
- `GET /cfg/internal/migrations/state`
- `GET /file/internal/migrations/state`
- `GET /prue/internal/migrations/state`

各服务的根路径前缀见各服务 AGENTS.md。

### 2.3 鉴权

- 必须 service-jwt（Phase 5-B 标准），与 `iam/internal/*` 一致
- 不接受 user-jwt
- 走 `verify_aud_principal_middleware`

---

## 3. 架构设计

### 3.1 `libs/common-db::MigrationState` trait

```rust
// libs/common-db/src/migration_state.rs

#[async_trait]
pub trait MigrationState: Send + Sync {
    fn service_name(&self) -> &'static str;
    fn dialect(&self) -> &'static str;     // "mysql" / "sqlite" / "mysql+sqlite"

    /// (version, name, marker_table) 元组列表
    fn migrations(&self) -> &[MigrationDescriptor];

    /// 探测 applied 状态：返回已应用的 version 列表
    async fn probe_applied(&self, pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error>;
}

pub struct MigrationDescriptor {
    pub version: &'static str,
    pub name: &'static str,
    pub marker_table: &'static str,
}

pub async fn migration_state_handler(
    State(state): State<Arc<dyn MigrationState>>,
    Extension(pool): Extension<MySqlPool>,
) -> Result<Json<MigrationStateResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 1. 调 state.probe_applied(&pool).await
    // 2. 比对 state.migrations() 列表
    // 3. 计算 missing / extra
    // 4. 返回 JSON
}
```

### 3.2 使用模式（以 iam-service 为例）

```rust
// services/iam-service/src/migration_state.rs (新建)
pub struct IamMigrationState;

impl MigrationState for IamMigrationState {
    fn service_name(&self) -> &'static str { "iam-service" }
    fn dialect(&self) -> &'static str { "mysql" }
    fn migrations(&self) -> &[MigrationDescriptor] {
        &[
            MigrationDescriptor { version: "V001", name: "init", marker_table: "iam_users" },
            MigrationDescriptor { version: "V002", name: "outbox_archive", marker_table: "iam_outbox_events_archive" },
            MigrationDescriptor { version: "V003", name: "tenants", marker_table: "iam_tenants" },
            MigrationDescriptor { version: "V004", name: "tenant_business_tables", marker_table: "iam_migration_v004_applied" },
            MigrationDescriptor { version: "V005", name: "platform_admin_role", marker_table: "iam_migration_v005_applied" },
        ]
    }
    async fn probe_applied(&self, pool: &MySqlPool) -> Result<Vec<String>, sqlx::Error> {
        // 对每个 migration: SELECT COUNT(*) FROM information_schema.tables WHERE table_name = ?
        // 返回存在的 version 列表
    }
}

// services/iam-service/src/routes/mod.rs
.route("/iam/internal/migrations/state",
       get(common_db::migration_state::migration_state_handler)
           .with_state(Arc::new(IamMigrationState))
           .layer(Extension(pool.clone()))
           .layer(verify_aud_principal_middleware(...)))
```

### 3.3 prue_backend 双 dialect 特例

```rust
impl MigrationState for PrueMigrationState {
    fn service_name(&self) -> &'static str { "prue_backend" }
    fn dialect(&self) -> &'static str { "mysql+sqlite" }
    // migrations() 返回 1 个 V001 (marker_table = prue_menu_configs)
    // probe_applied 根据 pool 类型分发:
    //   - MySqlPool → SELECT FROM information_schema.tables
    //   - SqlitePool → SELECT FROM sqlite_master WHERE type='table'
}
```

---

## 4. 验收

### 4.1 功能
- [ ] 7 个服务都暴露 `/<svc>/internal/migrations/state`
- [ ] 鉴权：必须 service-jwt
- [ ] DB 不可达返回 503
- [ ] prue_backend 双 dialect 都返回正确状态

### 4.2 测试
- [ ] 每个服务集成测试 1 case：调用端点，验证 JSON 结构
- [ ] "missing" 测试：手动删一张 marker table，验证端点报 missing
- [ ] "extra" 测试：手动建一张 orphan table（不可能但 API 需支持）

### 4.3 文档
- [ ] `docs-site/guides/operations.md` 加 "如何检查 schema 一致性" 一节
- [ ] 每个服务 AGENTS.md 在端点域加 `/<svc>/internal/migrations/state`

### 4.4 不在范围
- 自动 schema drift 告警（后续）
- 跨服务 schema 联邦视图（`GET /internal/migrations/state/all` 聚合）（后续）
- 回滚 / 修复 migration 的端点（不在本 spec）
- audit / config / file 服务的双 dialect 支持（当前都是 MySQL-only，不为 SQLite 适配）

---

## 5. 实施工作量估算

| 子任务 | 估算 |
|--------|------|
| `libs/common-db/src/migration_state.rs` + trait + handler | 半天 |
| iam-service 接入（含集成测） | 1 小时 |
| notification-service 接入 | 30 min（已有 3 个 marker） |
| token / audit / config / file 各接入 | 30 min × 4 = 2 小时 |
| prue_backend 双 dialect 接入 | 1 小时 |
| 集成测试 + docs 同步 | 1 小时 |
| **合计** | **~1 天** |

---

## 6. 风险

| 风险 | 缓解 |
|------|------|
| marker table 命名可能不直观（`iam_users` 不明显表示 V001） | 端点返回 name + version 双标识 |
| 多个 V00X 用同一 marker table | 当前没这情况；如有，迁移机制需重构 |
| service-jwt 鉴权与现有内部端点行为不一致 | 复用 `verify_aud_principal_middleware` |
| audit-service 没有 db.rs（main.rs inlined） | 新建 `audit-service/src/db.rs`，把 inlined 函数挪过去 |

---

## 7. 关联

- TODO.md §Phase 10 line 417
- `libs/common-db` — 现有 db 连接抽象 + retry / pool 配置
- 各服务 `db.rs::ensure_schema` — 现有 marker-table 模式
- Phase 5-B service-jwt + `verify_aud_principal_middleware` — 鉴权基础
- AGENTS.md 各服务端点域 — 文档更新位置