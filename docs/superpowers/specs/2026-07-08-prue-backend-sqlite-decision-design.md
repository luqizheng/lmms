# prue_backend SQLite 保留/去除决策 — 设计

**日期**：2026-07-08
**状态**：草案 · 待评审
**关联**：TODO.md §Phase 10 `prue_backend 评估是否去掉 SQLite 分支，统一 MySQL`

---

## 0. 一句话

> prue_backend 同时跑 MySQL + SQLite 双 dialect（sqlx feature flag + 双 migrations 目录）。
> 仅 1 个 migration（`V001__init.sql`）。本文档评估"砍掉 SQLite"的成本/收益，给出决策建议。

---

## 1. 现状

| 维度 | 现状 |
|------|------|
| sqlx features | `mysql` + `sqlite`（`Cargo.toml` line ~30） |
| migrations 目录 | `migrations/{mysql,sqlite}/V001__init.sql`，文件名一致 |
| migrations 数量 | **仅 1 个**（`V001__init.sql`） |
| DATABASE_URL 默认 | `sqlite::memory:`（`config.rs::default_database_url`） |
| 部署 | dev 用 SQLite 内存/文件；prod（docker compose）用 MySQL |
| 业务边界 | "只管理 pure-admin 菜单配置和动态路由" — 与 iam-service 完全解耦（`AGENTS.md` line 35） |
| 依赖关系 | prue_backend 独立服务，不连 iam / token / Redis / RabbitMQ |

### 1.1 V001 双方言实际差异

```
diff mysql vs sqlite:
  VARCHAR(64)       → TEXT
  BIGINT            → INTEGER
  JSON              → TEXT            (sqlite 无原生 JSON, 业务侧走 serde_json)
  CHAR(64)          → TEXT
  BOOLEAN           → INTEGER (0/1)
  TIMESTAMP DEFAULT → TEXT DEFAULT
  ON UPDATE CURRENT_TIMESTAMP 子句 → 移除（sqlite 需 trigger）
  UNIQUE/INDEX 子句 → CREATE INDEX IF NOT EXISTS 拆分语句
```

**约 12 处差异**，但**每行是 1:1 机械翻译**，不是"重新设计"。

### 1.2 维护成本实测

每个新 migration 的额外工作量：
- 写完 MySQL 版后，对 sqlite 翻译 → ~5 min（熟练后 ~2 min）
- CI 校验两版等价 → 现**无此 CI gate**

---

## 2. 砍掉 SQLite 的代价

| # | 代价 | 影响范围 |
|---|------|----------|
| 1 | dev 启动不再零依赖 | 新人 onboarding：必须先起 docker compose；当前 `cargo run -p prue_backend` 即跑 |
| 2 | CI 复杂度上升 | `scripts/run-tests-docker.js prue_backend` 必须挂 MySQL testcontainer；当前可用 `sqlite::memory:` in-memory |
| 3 | pure-admin 嵌入式场景丧失 | 如未来需把菜单配置嵌进客户侧单机部署 → SQLite 是唯一选择 |
| 4 | 砍 sqlx `sqlite` feature → binary size 微减 | `-~200KB`，无实质收益 |
| 5 | 删 `migrations/sqlite/` 目录 + `Cargo.toml` feature | ~10 行改动 |

---

## 3. 砍掉 SQLite 的收益

| # | 收益 | 量化 |
|---|------|------|
| 1 | 减少 schema 翻译负担 | 每 migration ~5 min × 未来 N 个 migration |
| 2 | 单一真相源 | MySQL 方言即 spec |
| 3 | 杜绝两版 schema drift 风险 | 现状：sync by humans，无 CI gate |

---

## 4. 决策建议

### 4.1 短期（1.0 内）：**保留 SQLite**

理由：
1. **成本不对称**：砍 SQLite 收益是"未来潜在 ~5min/migration"；代价是"立即 dev 体验 + CI 复杂度 + 嵌入式能力丧失"。
2. **当前仅 1 个 migration**：翻译负担实际为 0。
3. **business boundary**：prue_backend 与 iam 解耦，本质就是 standalone 工具服务，SQLite 是合理选择。

### 4.2 配套（立刻可做，~2 小时）：**加 CI gate 防 drift**

```bash
# 新增 scripts/check-prue-migrations-parity.sh
# 比较 mysql vs sqlite 的 schema 抽象等价：
#   - 表名集合相同
#   - 列名集合相同
#   - 主键列相同
#   - 索引名集合相同（容忍语法差异）
```

工具实现思路：
- 用 `sqlx-cli` 分别 `database create && sqlx migrate run` 到临时 sqlite + 临时 mysql
- dump schema → 比对抽象结构
- 不要求字节级一致，只要求"表/列/索引/外键"集合相同

### 4.3 长期（1.0+ / Roadmap 2.0）：**重新评估**

触发条件（任一即重开）：
1. migration 总数 ≥ 10，翻译成本开始可感
2. 出现 schema drift bug（两版行为不一致）
3. prue_backend 业务范围扩大（如增加 RBAC 联表），sqlite 方言短板暴露
4. pure-admin 嵌入式需求彻底无望

---

## 5. 不在本文范围

- prue_backend 是否需要引入 `pure-admin-backend` 原版 Java 实现（已废弃）
- prue_backend 是否拆分为 frontend-config-server + admin-config-server（当前不必要）
- 其他服务（iam / notification 等）的 SQLite 评估（它们都 MySQL-only）

---

## 6. 验收

- [x] 现状摸清（1 个 migration，sqlx feature flag 开）
- [x] 双方言差异实测（~12 处）
- [x] 砍/留决策给出（短期保留 + 加 CI gate）
- [x] 长期重开条件列出

---

## 7. 关联

- TODO.md §Phase 10 `prue_backend 评估是否去掉 SQLite 分支`
- `services/prue_backend/Cargo.toml` — sqlx feature flags
- `services/prue_backend/migrations/{mysql,sqlite}/V001__init.sql`
- `services/prue_backend/AGENTS.md` — "服务不关联 iam-service" 边界声明