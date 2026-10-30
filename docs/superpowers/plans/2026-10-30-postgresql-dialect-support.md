# PostgreSQL 基础方言支持 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 libs 基座层为 PostgreSQL 建立与 MySQL/SQLite 同级的基础方言支持：`DialectKind::Postgres` 完整方言、common-db `postgres` feature（`DbPool::Postgres`）、CI 方言矩阵 PR 级测试、文档同步。

**Architecture:** 完全沿用达梦/MSSQL 的预留先例——方言差异进 `dialect/pg.rs`（纯函数、无 IO），连接能力进 `DbPool` 的 cfg feature 臂，`postgresql://` URL 从「静默回退 Mysql」改为显式判定（行为变更，测试钉死）。

**Tech Stack:** sqlx 0.8（workspace，`postgres` 仅作 feature gate）、Rust workspace libs/common-db、GitHub Actions dialect-matrix workflow。

**Spec:** `docs/superpowers/specs/2026-10-30-postgresql-dialect-support-design.md`

## Global Constraints

- `postgres` **不进 default features**；default（sqlite）构建产物零变化。
- sqlx 0.8 workspace 统一；不新增任何原生 C 依赖。
- 方言实现风格照抄 `dameng.rs`（中文注释、差异说明、保留字表、辅助函数模块）。
- 版本号不动——随下一次 libs 统一发布（`coder release publish rust`）。
- lmms 服务侧零改动：token-service `DbType` 不加 postgres；其「未知 db_type 忽略」测试必须保持原样。
- 测试容器常开（用户指示）：coder-test-pg 起后不停。

## Review Focus

1. **`from_url` 大小写**：现有实现用 `starts_with` 精确小写前缀（`sqlite:`/`dameng:`）；PG 的 `postgres://` 与 `postgresql://` 同样精确小写，不做大小写归一（保持全矩阵行为一致）。
2. **穷尽 match**：`run_migrations` 的 `dialect_str` match 覆盖全部变体，新增 `Postgres` 后编译器会强制补臂——漏补即编译失败，属预期护栏。
3. **feature 组合矩阵**：`--features postgres` 单开、`postgres,mysql` 组合、default 三种都必须编译且测试通过（DbPool/Transaction 的 cfg 臂互相独立）。
4. **PG 集成测试的可跳过性**：无 `TEST_PG_URL` 时测试必须**通过并打印 skip 说明**，不得失败——CI 无 service 的 job、本地无容器场景都依赖这一点。
5. **保留字来源**：PG 保留字表必须取自 PostgreSQL 官方文档 Appendix C「reserved」列，禁止凭记忆手写。

---

### Task 1: `DialectKind::Postgres` 方言层

**Files:**
- Create: `libs/common-db/src/dialect/pg.rs`
- Modify: `libs/common-db/src/dialect/mod.rs`（变体、from_url、now_expr、escape_identifier、is_reserved_keyword、`pub mod pg;`）
- Modify: `libs/common-db/src/dialect/mod.rs` 的 `run_migrations` 所在文件 `libs/common-db/src/lib.rs`（仅 `dialect_str` 补 `DialectKind::Postgres => "postgres"` 臂）
- Modify: `libs/common-db/tests/dbpool_test.rs`（postgresql:// 断言改写 + 新增断言）

**Interfaces:**
- Produces（`dialect::pg`）:
  - `pub const PG_RESERVED_WORDS: &[&str]`——PostgreSQL 官方 Appendix C「reserved」列全表。
  - `pub mod identity { pub fn identity_column(column: &str, column_type: &str) -> String }` → `"{column} {column_type} GENERATED ALWAYS AS IDENTITY"`。
- `DialectKind::Postgres` 行为：
  - `from_url`：`postgres://`、`postgresql://` → `Postgres`；未知前缀仍回退 `Mysql`。
  - `now_expr()` → `"now() AT TIME ZONE 'UTC'"`（与其余方言的显式 UTC 语义对齐）。
  - `escape_identifier` → `"name"` 双引号双写（同 Sqlite/Dameng 形态）。
  - `is_reserved_keyword` → 查 `pg::PG_RESERVED_WORDS`。
  - `paginate` → 通用 `LIMIT {} OFFSET {}`（现有默认臂覆盖，无新代码）。

- [ ] **Step 1: 改写与新增失败测试（dbpool_test.rs）**

改写既有断言：
```rust
#[test]
fn dialect_pg_urls_are_explicit() {
    assert_eq!(DialectKind::from_url("postgresql://localhost/db"), DialectKind::Postgres);
    assert_eq!(DialectKind::from_url("postgres://localhost/db"), DialectKind::Postgres);
}
```
`dialect_unknown_url_defaults_mysql` 中删除 `postgresql://` 行（mssql://、oracle:// 回退断言保留）。新增：
```rust
#[test]
fn dialect_pg_has_explicit_utc_and_quoting() {
    let d = DialectKind::Postgres;
    assert_eq!(d.now_expr(), "now() AT TIME ZONE 'UTC'");
    assert_eq!(d.escape_identifier("user"), "\"user\"");
    assert!(d.is_reserved_keyword("SELECT"));
    assert!(!d.is_reserved_keyword("coder_not_reserved"));
}
```

- [ ] **Step 2: 编译失败即 RED**

Run: `cargo test -p common-db --test dbpool_test`
Expected: FAIL——`DialectKind::Postgres` 变体不存在（编译错误）。

- [ ] **Step 3: 实现**

`pg.rs` 按 Interfaces 实现（保留字表从 PostgreSQL 官方 Appendix C「reserved」列转录，来源写进文件头注释）；`mod.rs` 加 `pub mod pg;`、变体与四个 match 臂；`lib.rs` 的 `run_migrations` 补 `DialectKind::Postgres => "postgres"`。

- [ ] **Step 4: 测试通过**

Run: `cargo test -p common-db --test dbpool_test && cargo test -p common-db`（default features）
Expected: PASS，default 无回归。

- [ ] **Step 5: 提交**

```bash
git add libs/common-db
git commit -m "feat(dialect): add PostgreSQL as an explicit base dialect"
```

---

### Task 2: common-db `postgres` feature 与连接层

**Files:**
- Modify: `libs/common-db/Cargo.toml`（features 加 `postgres = ["sqlx/postgres"]`）
- Modify: `libs/common-db/src/lib.rs`（`use sqlx::postgres::Postgres`（cfg）、`DbPool::Postgres` 臂、`dialect()` 臂、`init_pool` 分支、`Transaction` pg 字段与 commit/rollback 臂）
- Create: `libs/common-db/tests/pg_test.rs`

**Interfaces:**
- `DbPool::Postgres(sqlx::Pool<Postgres>)`（`#[cfg(feature = "postgres")]`），`dialect()` → `DialectKind::Postgres`。
- `init_pool`：`DialectKind::Postgres` 分支（cfg postgres），`PgPoolOptions::new().max_connections(10)`，错误映射与现有臂一致（`AppError::internal("DB_CONNECT", …)`）。
- `Transaction`：`#[cfg(feature = "postgres")] pg: Option<sqlx::Transaction<'a, Postgres>>`，commit/rollback 各补对应臂。
- 无 `postgres` feature 时 `init_pool("postgres://…")` 返回既有 `DB_FEATURE` 错误（`_ =>` 臂自然覆盖，无新代码）。
- 集成测试环境变量：`TEST_PG_URL`（如 `postgres://postgres:coder@localhost:5434/coder_test`）；缺失时打印 `[pg_test] TEST_PG_URL not set — skipped` 并 return。

- [ ] **Step 1: 写失败测试（pg_test.rs）**

三个测试，全部以「无 `TEST_PG_URL` 即跳过」开头：
1. `pg_dialect_url_maps_to_postgres`——纯枚举断言（不需要 DB）。
2. `pg_pool_connects_and_reports_dialect`——`init_pool(TEST_PG_URL)` → `dialect() == Postgres`，`sqlx::query("SELECT 1")` 取回 1。
3. `pg_now_expr_is_valid_sql`——连接后 `format!("SELECT {}", d.now_expr())` 执行成功。

- [ ] **Step 2: RED**

Run: `cargo test -p common-db --features postgres --test pg_test`
Expected: FAIL——`postgres` feature 不存在（Cargo 编译错误）。

- [ ] **Step 3: 实现**

按 Interfaces 补 Cargo.toml feature、lib.rs 各 cfg 臂。Feature 组合检查三连：
```bash
cargo test -p common-db --features postgres
cargo test -p common-db --features "postgres,mysql"
cargo test -p common-db
```

- [ ] **Step 4: 本地容器实测（一次性）**

```bash
docker run -d --name coder-test-pg -p 5434:5432 -e POSTGRES_PASSWORD=coder -e POSTGRES_DB=coder_test postgres:16
TEST_PG_URL='postgres://postgres:coder@localhost:5434/coder_test' cargo test -p common-db --features postgres --test pg_test
```
Expected: 3/3 PASS（真实连接）。容器保留运行（常开规则）。

- [ ] **Step 5: 提交**

```bash
git add libs/common-db
git commit -m "feat(common-db): postgres feature with pg pool and transaction arms"
```

---

### Task 3: CI 方言矩阵、文档与全量验证

**Files:**
- Modify: `.github/workflows/dialect-matrix.yml`（加 `test-postgres` PR 级 job）
- Modify: `docs/superpowers/specs/dialect-matrix.md`（支持表加 PG 行；分页/自增/当前时间/标识符转义四节补 PG 列）
- Modify: `docs-site/guides/testing.md`（补 coder-test-pg 段落：端口 5434、常开规则、TEST_PG_URL 用法）

**Interfaces:**
- CI job 复刻 test-mysql 形态：`services: postgres: image postgres:16, env POSTGRES_PASSWORD: coder / POSTGRES_DB: coder_test, options: --health-cmd pg_isready`；steps 跑 `TEST_PG_URL: postgres://postgres:coder@postgres:5432/coder_test cargo test --package common-db --features postgres`。

- [ ] **Step 1: workflow 加 job**

照 test-mysql job 结构写 `test-postgres`（PR 级，随现有 `on.pull_request` 触发）。校验：`node -e "const y=require('js-yaml');y.load(require('fs').readFileSync('.github/workflows/dialect-matrix.yml','utf8'))"`（root devDeps 有 js-yaml）不抛错。

- [ ] **Step 2: 文档同步**

matrix 文档支持表加行：`| PostgreSQL | sqlx-postgres | postgres | ai-pm 生产 + 通用基础 | PR 级别 |`；四节差异表各补 PG 行（分页 `LIMIT ? OFFSET ?`、自增 `GENERATED ALWAYS AS IDENTITY`、当前时间 `now() AT TIME ZONE 'UTC'`、标识符 `"name"` 双写）；文档头部「当前项目仅使用 MySQL+SQLite」表述更新为含 PG。testing.md 补容器段落。

- [ ] **Step 3: 全量验证**

```bash
cargo test -p common-db && cargo test -p common-db --features postgres && cargo test -p common-db --features "postgres,mysql" && cargo test -p common-db --features mysql
```
Expected: 全绿（postgres 组合需 coder-test-pg 在跑 + TEST_PG_URL）。

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/dialect-matrix.yml docs/superpowers/specs/dialect-matrix.md docs-site/guides/testing.md
git commit -m "ci(dialect): add PR-level postgres matrix job and sync docs"
```

---

## 完成判据

1. 三个 feature 组合测试全绿；`postgresql://` 显式判定有测试钉死。
2. CI workflow YAML 语法有效且 job 与既有四档风格一致。
3. 文档与实现一致；coder-test-pg 容器常开并在本机真实跑过集成测试。
4. lmms 服务侧零 diff（`git diff --stat services/` 为空）。
