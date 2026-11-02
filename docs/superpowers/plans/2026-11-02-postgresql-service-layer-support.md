# 服务层全量 PostgreSQL 支持 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 7 个服务 + `iam-outbox-publisher` 在同一份业务 SQL 源码下同时支持 MySQL 与 PostgreSQL，按 S0~S6 切片提交（一切片一 commit）。

**Architecture:** 方言差异收敛在两层——`libs/common-db` 的**构造期方言化**（`sql::render` + `db_*!` 宏，SQL 单份源码）+ **`DbPool` 枚举**决定后端。DDL 差异走新增的 `migrations/postgres/` 目录。运行时无法统一的少数点（`ON CONFLICT`/`GROUP_CONCAT`/`GET_LOCK`/`NOW(3)`/`INTERVAL`）走**应用层显式分支并注释**。

**Tech Stack:** sqlx 0.8.6（`mysql`+`postgres`+`sqlite` feature gate）、Rust workspace、GitHub Actions dialect-matrix、testcontainers-rs 0.8（`postgres` 模块）、Node 门禁脚本。

**Spec:** `docs/superpowers/specs/2026-11-02-postgresql-service-layer-support-design.md`
**PRD:** `docs/superpowers/specs/2026-11-02-postgresql-service-layer-support-prd.md`（已裁决 Q1~Q8）
**验证环境:** `docs/superpowers/specs/2026-11-02-postgresql-verification-env.md`

> **技术方案已被运行时实测确认（主理人独立探针，sqlx 0.8.6 + 真实 PG 16）**：
> `PG + $1` ✅ / `PG + ?` ❌ / `QueryBuilder` 自动产出 `$1` / 自研 `rewrite()` 等价可执行 / 多占位符 `$1,$2` 正确。
> 本计划与设计文档 §1.3.0 记录该实测原文。**不再需要推测性论证**。

## Global Constraints

- **`postgres` 不进任何 `default features`**。`libs/common-db/Cargo.toml` 的 `default = ["sqlite"]` 保持不变；
  各服务 `default = ["mysql"]`（`prue_backend` 为 `["mysql","sqlite"]`，与现状一致）。
  ✅ **默认构建产物零变化可严格保证**（方案 3′）：方言 feature **只**经服务自己的 `[features]` 转译；
  服务依赖声明**不硬写**方言 feature → 默认构建（`default = ["mysql"]`）**不带** `sqlx-postgres` 驱动。
  **可执行证明**：`cargo tree -e features -i digua_common_db | grep -c 'digua_common_db feature "postgres"'` == 0；
  `cargo tree -i sqlx-postgres` 在默认构建中无输出。
- **MySQL/SQLite 既有构建零回归**：默认构建**行为**不变、`cargo test --workspace` 不回归
  （注意 D-6 需带 feature，见 S1 风险 1b）。
- **不编辑任何已发布 MySQL 迁移**（forward-only）。PG 侧是**新增目录**。
- **PG 迁移禁 `$$` / `DO` / `CREATE FUNCTION` / `CREATE TRIGGER`**（执行器是 `script.split(';')`，会切坏 dollar-quoted）。
- **PG 时间列一律 `TIMESTAMPTZ` / `TIMESTAMPTZ(3)`**，不用 `TIMESTAMP`（Rust `DateTime<Utc>` 只解 `TIMESTAMPTZ`，实测）。
- **表名带服务前缀**（`iam_`/`audit_`/`token_`/`ntf_`/`cfg_`/`file_`/`prue_`），`check-db-naming.js` 守门。
- **OpenAPI 零变更**：PG 支持不改 API 形状；`services/*/openapi.yaml` 禁止手改。
- **一任务一 commit，禁止 amend / force-push**；分支 `feat/postgres-service-support`（从 `main` 新建）。
- **禁用 `let DbPool::Xxx(p) = pool;`**（E0005）与**穷尽 `match`**（E0004）。
- **`render` 只返回 `RewrittenQuery`，不返回裸 `String`**；未过 `expect_binds` 校验不得取 `sql()`。
  **MySQL 分支同样校验 bind 个数**——这是"MySQL 静默接受编号错位"的唯一封堵点。
- **业务 SQL 源码中只允许 `?` 占位符**。禁止 `$1`/`:name`。
  门禁：`grep -rnE '\\$[0-9]' services/*/src --include=*.rs` 应为空。
- **方言分叉白名单**：`match ... dialect()` 仅允许归属表 #6/#7/#8/#11/#21/#22 六类（约 30 处），且每处必须带注释。

## 验证环境（每条验收命令都按此执行）

```bash
export CARGO_TARGET_DIR="D:/kudi-project/templates/low-memory-micro-services/.target/pgtest"
export SWAGGER_UI_DOWNLOAD_URL="file:///D:/kudi-project/templates/low-memory-micro-services/infrastructure/vendor/swagger-ui-v5.17.14.zip"
# PG: 容器 coder-test-pg（postgres:16, 端口 5434），或 pgct（端口 5432）
export TEST_PG_URL="postgres://postgres:coder@127.0.0.1:5434/coder_test"
```

> **五类验证环境标注**：🟢 沙箱内（静态检查/脚本）｜🔵 需可编译环境（`cargo check`/`test`）｜🟣 需 CI｜⬜ 需真实 PG 容器｜📄 仅文档评审。

## Review Focus

1. **顺序契约（本任务最高风险）**：`sql::render` 的第 k 个 `?` ↔ 宏展开的第 k 个 `.bind()`。
   - 评审时搜 `render(` 的调用点，确认**只在宏内部**；任何"先 render、再在别处 bind"的两步式都是错的。
   - 必须使用 `RewrittenQuery::expect_binds`（消耗式）→ 未校验拿不到 `sql()`。检查是否存在 `#[allow(unused_must_use)]` 之类绕过。
2. **字面量内 `?` 断言**：`render` 必须在字符串字面量/标识符/注释/dollar-quote 内发现 `?` 时返回 `Err`，未闭合也要 `Err`。单测须覆盖设计文档 §6.5 的 18 条。
   - 特别检查 `''` 转义是否**显式 lookahead**（不是朴素 toggle）。
3. **MySQL 侧也必须计数校验**：`expected_binds` 在 MySQL/SQLite 分支同样生效。若发现只在 PG 分支校验 → 打回（这是"MySQL 静默接受错位"缺陷的唯一封堵点）。
4. **`TIMESTAMPTZ` 而非 `TIMESTAMP`**：所有 PG 迁移里映射到 `DateTime<Utc>` 的列必须是 `TIMESTAMPTZ`。
   - 门禁：`grep -rnE '\bTIMESTAMP\b' services/*/migrations/postgres/` 必须零命中（`\b` 不会匹配 `TIMESTAMPTZ`，已验证）。
5. **无穷尽 match**：`grep -rn "match pool" services/ libs/` 的每处都必须有 `_ =>`。
6. **feature 隔离**：`grep -rn "postgres" services/*/Cargo.toml | grep default` 必须无输出。
7. **迁移文件数对齐**：`ls services/<svc>/migrations/postgres | wc -l` == `ls services/<svc>/migrations/mysql | wc -l`。
8. **方言分叉白名单**：`match ... dialect()` 只允许出现在归属表 #6/#7/#8/#11/#21/#22 这 6 类（约 30 处），且每处必须带注释说明为何无法统一。
9. **⚠️ 宏内 cfg 与服务 feature 转译必须成对出现（S1 阻塞）**：宏内 `#[cfg(feature = "...")]` 按**调用方 crate** 求值 →
   调用方**必须**定义同名 feature 并**转译**到库与 `sqlx/*`，两套 cfg 才同步。
   - 少了转译 → 分支被全剔除 → `E0004` / 展开成 `()` → **S1 编译失败**
   - 依赖声明里硬写 `features = ["postgres"]` → 默认构建被迫带 `sqlx-postgres` → 破坏「默认构建产物零变化」
   评审检查（两条都要）：`grep -n 'cfg(feature' libs/common-db/src/query.rs` **有命中**（不是零命中！）
   且 `grep -n 'features = \["mysql"\|features = \["postgres"' services/*/Cargo.toml` 应为空（依赖声明不硬写方言 feature）。
10. **宏的四条形态约束**（可直接对照检查的清单）：

    | # | 约束 | 违反后果 | 检查命令 |
    |---|---|---|---|
    | 1 | `match &$pool`（**不是** `match $pool`） | `Pool<DB>: Executor` 不满足 → **E0277** | `grep -c 'match \&$pool' libs/common-db/src/query.rs` ≥ 宏数 |
    | 2 | 分支内**直接 `.await`**（**不用** `async move` 包裹） | inline `format!()` 临时量提前 drop → **E0716** | `grep -c 'async move' libs/common-db/src/query.rs` == 0 |
    | 3 | 错误用 `map_err(...)?`（**不用**裸 `return Err(...)`） | 类型不匹配（`?` 有 `From` 转换，裸 return 没有） | `grep -c 'return Err(sqlx::Error' libs/common-db/src/query.rs` == 0 |
    | 4 | 宏产物是 `Result<T, sqlx::Error>`（已 await 完毕） | 调用点多加一次 `.await` → 类型错误 | 调用点不应出现 `db_fetch_*(...).await` |

11. **🔴 兜底臂是强制项**（设计 §1.4.9）：每个宏的 `match` **必须**带 `_ => Err(...)`，
    错误信息要指名道姓（方言 + 原因）。**不得**返回空结果 / panic / 静默降级。
    **当前方案 3 下不可达**（三变体恒存）——正因如此容易被当成冗余代码删掉，故标强制。
    检查：`grep -c 'unreachable_patterns' libs/common-db/src/query.rs` ≥ 宏数量。
12. **方言 feature 一致性门禁**（设计 §1.4.7 / S4 Step 1b）：`node scripts/check-dialect-features.js`
    必须 PASS —— 它校验「库侧 feature 常开」「服务 feature 转发完整」「default 不含 postgres」「宏内无 cfg」。

---

## 切片总览与并行关系

| 切片 | 内容 | 前置 | 可并行 | commit 前缀 |
|---|---|---|---|---|
| **S0** | 门禁脚本修复 + PG 迁移门禁脚本 | 无 | 与 S1 **可并行** | `chore(gate)` |
| **S1** | 库层：SQL 渲染 + 查询宏 + 会话 TZ + PG marker + health + `common` 门面 | 无（与 S0 并行） | 与 S0 **可并行** | `feat(common-db)` |
| **S2** | `common-event` outbox 泛型化 | S1 | 与 S3 **可并行**（S3 的 prue/config/file 不经 outbox） | `feat(common-event)` |
| **S3** | P0 三服务（prue/config/file）迁移集 + 类型改造 + fixture | S1 | 与 S2 **可并行** | `feat(p0-services)` |
| **S4** | CI job + compose PG + `shared-db.js` | S3 | — | `ci(dialect)` |
| **S5** | 其余 5 单元（audit/token/notification/iam/publisher） | S2 + S3 | S5a（audit）/ S5b（token）/ S5c（notification）**可并行**；S5d（iam）+ S5e（publisher）互依 | `feat(p1-services)` |
| **S6** | 文档同步 | S5 | — | `docs(pg)` |

```mermaid
graph TD
    S0[S0 门禁脚本] --> S4
    S1[S1 库层抽象] --> S2[S2 outbox 泛型化]
    S1 --> S3[S3 P0 三服务]
    S2 --> S5[S5 P1 五单元]
    S3 --> S5
    S3 --> S4[S4 CI/compose]
    S5 --> S6[S6 文档同步]
    S0 -.可并行.-> S1
    S2 -.可并行.-> S3
```

---

### Task S0: 门禁脚本修复 + PG 迁移门禁（**可立即开工，与 S1 并行**）

**目标**：让 `check-db-naming.js` 对 PG 语法零误报，并新增 `check-pg-migrations.js` 把 B-4 变成门禁。**必须先于任何 PG 迁移落地**（R-3）。

**Files:**
- Modify: `scripts/check-db-naming.js`
- Create: `scripts/check-pg-migrations.js`
- Modify: `package.json`（`check` 聚合脚本里加 `node scripts/check-pg-migrations.js`）

**Interfaces:**
- `check-db-naming.js`：`VALID_PREFIXES` 去空洞；`TABLE_PATTERNS` 的 `UPDATE` 分支增加 `DO UPDATE SET` 前置排除；`SKIP_IN_SOURCE` 增 `pg_catalog` / `current_schema` / `pg_indexes`。
- `check-pg-migrations.js`：退出码 0/1；扫描 `services/*/migrations/postgres/**/*.sql`；禁止模式见下。

- [ ] **Step 1: 写失败测试（PG 语料）**

Create `scripts/fixtures/pg-sql-corpus.sql`（新建 `scripts/fixtures/` 目录）：
```sql
-- 以下都是合法 PG 语法，check-db-naming.js 必须零误报
INSERT INTO cfg_namespaces (id, tenant_id) VALUES ('a', 1)
ON CONFLICT (id) DO UPDATE SET tenant_id = EXCLUDED.tenant_id;
INSERT INTO iam_roles (id) VALUES ('r') ON CONFLICT DO NOTHING;
SELECT COUNT(*) FROM pg_catalog.pg_indexes WHERE schemaname = current_schema();
SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = current_schema();
```

- [ ] **Step 2: 确认 RED**

Run: `node scripts/check-db-naming.js`
Expected: FAIL，报 `SET` 与 `pg_catalog`（已用脚本模拟复现：`VIOLATION table=SET` / `VIOLATION table=pg_catalog`）。

- [ ] **Step 3: 修 `check-db-naming.js`**

三处改动：
1. `VALID_PREFIXES` 去掉末尾多逗号：`['iam_','audit_','token_','ntf_','cfg_','file_','prue_']`。
2. `isOnUpdateClause` 扩展（现有已处理 `ON UPDATE` / `KEY`，补 `DO UPDATE`）：
```js
function isOnUpdateClause(text, matchIndex) {
  const before = text.slice(Math.max(0, matchIndex - 24), matchIndex).toUpperCase();
  return /\bON\s*$/.test(before) || /\bKEY\s*$/.test(before) || /\bDO\s+UPDATE\s*$/.test(before);
}
```
3. `SKIP_IN_SOURCE` 增 `'pg_catalog'`, `'current_schema'`, `'pg_indexes'`, `'pg_constraint'`。

- [ ] **Step 4: 新增 `scripts/check-pg-migrations.js`**

```js
#!/usr/bin/env node
// PG 迁移门禁：禁止 MySQL 残留语法 + 禁止 $$/DO/触发器（split(';') 不兼容）
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');

const FORBIDDEN = [
  [/\bAUTO_INCREMENT\b/i,            'AUTO_INCREMENT → GENERATED ALWAYS AS IDENTITY'],
  [/\bON\s+DUPLICATE\s+KEY\b/i,      'ON DUPLICATE KEY → ON CONFLICT'],
  [/\bON\s+UPDATE\s+CURRENT_TIMESTAMP/i, 'ON UPDATE CURRENT_TIMESTAMP → 应用层维护'],
  [/\bUNSIGNED\b/i,                  'UNSIGNED → BIGINT'],
  [/\bTINYINT\s*\(\s*1\s*\)/i,       'TINYINT(1) → BOOLEAN'],
  [/\bDATETIME\b/i,                  'DATETIME → TIMESTAMPTZ'],
  [/\bENGINE\s*=/i,                  'ENGINE= → 删除'],
  [/\bDEFAULT\s+CHARSET\b/i,         'DEFAULT CHARSET → 删除'],
  [/\bCOLLATE\s*=\s*\w+\./i,         'MySQL COLLATE → 删除'],
  [/\bUNIQUE\s+KEY\b/i,              'UNIQUE KEY → CREATE UNIQUE INDEX'],
  [/\bINSERT\s+IGNORE\b/i,           'INSERT IGNORE → ON CONFLICT DO NOTHING'],
  [/\bGROUP_CONCAT\s*\(/i,           'GROUP_CONCAT → STRING_AGG'],
  [/\bDATABASE\s*\(\s*\)/i,          'DATABASE() → current_schema()'],
  [/\$\$/,                           '$$ dollar-quoted 块（split(\';\') 不兼容）'],
  [/^\s*DO\s+\$/im,                  'DO 块（split(\';\') 不兼容）'],
  [/\bCREATE\s+(OR\s+REPLACE\s+)?(FUNCTION|TRIGGER)\b/i, 'FUNCTION/TRIGGER → 应用层维护'],
  [/COMMENT\s+'/i,                   "列/表内联 COMMENT '...' → 丢弃或 COMMENT ON"],
];
const WARN = [
  [/\bTIMESTAMP\s*\(?/i, '注意：请确认是 TIMESTAMPTZ（唯一能解 DateTime<Utc> 的类型）'],
];

function walk(dir, acc = []) { /* 递归收集 .sql */ }
const errors = [];
for (const f of walk(path.join(ROOT, 'services'))) {
  if (!f.includes(`${path.sep}migrations${path.sep}postgres${path.sep}`)) continue;
  const src = fs.readFileSync(f, 'utf8');
  const rel = path.relative(ROOT, f);
  src.split('\n').forEach((line, i) => {
    if (/^\s*--/.test(line)) return;                    // 跳过行注释
    for (const [re, msg] of FORBIDDEN) if (re.test(line)) errors.push(`${rel}:${i + 1} — ${msg}`);
  });
}
if (errors.length) { console.error(`FAIL: ${errors.length} 处 PG 迁移残留/不兼容语法`); errors.forEach(e => console.error('  ' + e)); process.exit(1); }
console.log('PASS: PG 迁移集无 MySQL 残留语法');
```

- [ ] **Step 5: 验证 GREEN**

Run:
```bash
node scripts/check-db-naming.js          # 期望 PASS
node scripts/check-pg-migrations.js      # 期望 PASS（此时还没有 postgres 目录，walk 返回空）
node scripts/check-spec.js               # 期望零 error
```

- [ ] **Step 6: 提交**

```bash
git add scripts/check-db-naming.js scripts/check-pg-migrations.js scripts/fixtures/pg-sql-corpus.sql package.json
git commit -m "chore(gate): teach db-naming gate PG syntax and add pg-migration lint"
```

**风险与回退**
- 风险：`DO UPDATE SET` 的排除规则若写宽，会漏掉真正的裸表名 `SET`。缓解：PG 语料回归（C-1b）固定 4 条用例。
- 回退：`git revert` 该 commit 即可（纯脚本，无编译依赖）。

---

### Task S1: 库层抽象（SQL 渲染 + 查询宏 + 会话 TZ + PG marker + health + 门面）**（与 S0 并行）**

**目标**：把"同一份 SQL 源码产出两种方言 + bind 顺序严格对应"做成库能力；补齐 PG 连接/marker/health/门面。

**Files:**
- Create: `libs/common-db/src/sql.rs`
- Create: `libs/common-db/src/query.rs`
- Create: `libs/common-db/tests/sql_render_test.rs`
- Modify: `libs/common-db/src/lib.rs`（`pub mod sql; pub mod query;`、`DbPool::as_*`/`ping`/`PlaceholderStyle`、`init_pool` PG `after_connect`）
- Modify: `libs/common-db/src/migration_state.rs`（`probe_marker_postgres` + `probe_marker`）
- Modify: `libs/common-db/src/scope.rs`（`WithTenantScope` 三方言 impl）
- Modify: `libs/common-db/Cargo.toml`（`[dev-dependencies]` 视需要；`default` **不动**）
- Modify: `libs/common-health/src/lib.rs`（`DbProbe::Postgres` + `with_postgres`）
- Modify: `libs/common-health/Cargo.toml`（sqlx features 加 `postgres`）
- Modify: `libs/common/src/db.rs`（`connect` 返回 `DbPool`）
- Modify: `libs/common/Cargo.toml`（feature 化）
- Modify: `services/{audit-service,config-service,file-service,iam-service,notification-service,prue_backend,token-service}/Cargo.toml`（**新增 `[features]` 段** + **移除依赖声明里硬写的方言 feature**——见 Step 3b，**S1 阻塞项**）
- Modify: `services/iam-outbox-publisher/Cargo.toml`（同上）
- Modify: `.github/workflows/dialect-matrix.yml`（**修正既有 `test-services` job 的包名**——见 Step 3c）

**Interfaces:**
- `digua_common_db::sql::{render, count_placeholders, RewrittenQuery, SqlRenderError, LiteralKind}`
  - `render(sql, dialect) -> Result<RewrittenQuery<'_>, SqlRenderError>`
  - `RewrittenQuery::{sql(&self) -> &str, expected_binds(&self) -> usize, expect_binds(self, actual: usize) -> Result<Self, SqlRenderError>}`
- `digua_common_db::{db_fetch_all!, db_fetch_one!, db_fetch_optional!, db_fetch_scalar!, db_execute!, db_qb_fetch_all!, db_qb_fetch_optional!, db_qb_fetch_one!, db_qb_execute!}`
- `DbPool::{dialect, placeholder_style, ping, as_mysql, as_postgres, as_sqlite}`
- `migration_state::{probe_marker_postgres, probe_marker}`
- `HealthState::with_postgres`
- `digua_common::db::connect(url, max) -> anyhow::Result<DiguaDbPool>`

**顺序契约（本任务的核心，必须逐条实现；设计文档 §1.3.1~§1.3.3 为权威）**
1. `render` 单遍扫描，跳过 `'...'`（**含 `''` 显式 lookahead**）、`"..."`、`` `...` ``、`--`、`/* */`、`$$...$$`。
2. 第 k 个裸 `?` → `$k`（PG）；MySQL/SQLite/Dameng **SQL 原样 `Cow::Borrowed`，但仍统计个数**。
3. 字面量/注释内发现 `?` → `Err(PlaceholderInLiteral{...})`；字面量未闭合 → `Err(UnterminatedLiteral{...})`；两者都回显原始 SQL。
4. `render` **返回 `RewrittenQuery`（非裸 String）**，携带 `expected_binds`；`sql()` 只在校验后可得。
5. 宏的 `$($bind:expr)*` 在每个 `match` 分支内**按同一顺序**展开，并调用
   `expect_binds(0usize $(+ { let _ = &$bind; 1usize })*)` —— **构造期**拦下个数错位（含 MySQL 侧）。
   > 若 `0usize $(+ ...)*` 这种"数 token 个数"的写法编译器不接受，退化为**先 `let __n = 0usize; $( let __n = __n + 1; )*`** 的显式计数；
   > 若仍不行，则降级为"不做 debug 断言，只保留 `render` 的字面量断言 + 集成测试"，并在实施日志记录（不阻塞）。

- [ ] **Step 1: 写失败测试 `tests/sql_render_test.rs`（18 条，见设计文档 §6.5 表）**

```rust
use digua_common_db::dialect::DialectKind;
use digua_common_db::sql::{count_placeholders, render, LiteralKind, SqlRenderError};

fn pg(sql: &str) -> Result<String, SqlRenderError> {
    render(sql, DialectKind::Postgres).map(|r| r.expect_binds(r.expected_binds()).unwrap().sql().to_string())
}

#[test] fn 01_pg_numbered_in_order() {
    let rq = render("SELECT * FROM t WHERE a = ? AND b = ? AND c = ?", DialectKind::Postgres).unwrap();
    assert_eq!(rq.expected_binds(), 3);
    assert_eq!(rq.sql(), "SELECT * FROM t WHERE a = $1 AND b = $2 AND c = $3");
}
#[test] fn 02_mysql_borrowed_and_still_counted() {
    let rq = render("SELECT * FROM t WHERE a = ? AND b = ?", DialectKind::Mysql).unwrap();
    assert_eq!(rq.sql(), "SELECT * FROM t WHERE a = ? AND b = ?");
    assert!(matches!(rq.sql_cow(), std::borrow::Cow::Borrowed(_)));   // 零分配
    assert_eq!(rq.expected_binds(), 2);                               // MySQL 也计数 → 封堵静默错位
}
#[test] fn 03_sqlite_unchanged() {
    assert_eq!(pg("SELECT ?").unwrap(), "SELECT $1");
    assert_eq!(render("SELECT ?", DialectKind::Sqlite).unwrap().sql(), "SELECT ?");
}
#[test] fn 04_bind_count_mismatch_at_construction() {
    let rq = render("SELECT * FROM t WHERE a = ? AND b = ?", DialectKind::Postgres).unwrap();
    let e = rq.expect_binds(1).unwrap_err();
    assert!(matches!(e, SqlRenderError::PlaceholderCountMismatch { expected: 2, actual: 1, .. }));
}
#[test] fn 05_placeholder_in_single_quote() {
    let e = render("SELECT '?' AS a", DialectKind::Postgres).unwrap_err();
    assert!(matches!(e, SqlRenderError::PlaceholderInLiteral { .. }));
}
#[test] fn 06_escaped_quote_stays_in_string() {
    // 'it''s ?' —— '' 是转义，不得提前退出字符串态
    assert!(matches!(render("SELECT 'it''s ?' AS a", DialectKind::Postgres).unwrap_err(),
                     SqlRenderError::PlaceholderInLiteral { .. }));
}
#[test] fn 07_string_closes_then_placeholder_counts() {
    assert_eq!(pg("SELECT 'a' , ? ").unwrap(), "SELECT 'a' , $1 ");
}
#[test] fn 08_double_quoted_ident_skipped() {
    assert_eq!(pg(r#"SELECT "a?b" FROM t WHERE id = ?"#).unwrap(),
               r#"SELECT "a?b" FROM t WHERE id = $1"#);
}
#[test] fn 09_backtick_ident_skipped_mysql() {
    let rq = render("SELECT `a?b` FROM t WHERE id = ?", DialectKind::Mysql).unwrap();
    assert_eq!(rq.expected_binds(), 1);
}
#[test] fn 10_line_comment()      { assert!(matches!(render("-- ?\nSELECT 1", DialectKind::Postgres).unwrap_err(), SqlRenderError::PlaceholderInLiteral { .. })); }
#[test] fn 11_block_comment()     { assert!(matches!(render("/* ? */ SELECT 1", DialectKind::Postgres).unwrap_err(), SqlRenderError::PlaceholderInLiteral { .. })); }
#[test] fn 12_dollar_quoted()     { assert!(matches!(render("SELECT $$ ? $$ , ?", DialectKind::Postgres).unwrap_err(), SqlRenderError::PlaceholderInLiteral { .. })); }
#[test] fn 13_unterminated_string(){ assert!(matches!(render("SELECT 1 WHERE x = 'unclosed", DialectKind::Postgres).unwrap_err(),
                                                 SqlRenderError::UnterminatedLiteral { kind: LiteralKind::SingleQuote, .. })); }
#[test] fn 14_unterminated_comment(){ assert!(matches!(render("SELECT 1 /* unclosed", DialectKind::Postgres).unwrap_err(),
                                                  SqlRenderError::UnterminatedLiteral { kind: LiteralKind::BlockComment, .. })); }
#[test] fn 15_existing_dollar_n_not_recounted() {
    let rq = render("SELECT 1 WHERE a = ? AND b = $1", DialectKind::Postgres).unwrap();
    assert_eq!(rq.sql(), "SELECT 1 WHERE a = $1 AND b = $1");
    assert_eq!(rq.expected_binds(), 1);
}
#[test] fn 16_like_placeholder_regression_guard() {
    assert!(matches!(render("SELECT * FROM t WHERE name LIKE '?%' AND id = ?", DialectKind::Postgres).unwrap_err(),
                     SqlRenderError::PlaceholderInLiteral { .. }));
}
#[test] fn 17_compact_multi_pg() {
    assert_eq!(pg("INSERT INTO t VALUES (?,?,?,?)").unwrap(), "INSERT INTO t VALUES ($1,$2,$3,$4)");
}
#[test] fn 18_compact_multi_mysql_counts() {
    assert_eq!(render("INSERT INTO t VALUES (?,?,?,?)", DialectKind::Mysql).unwrap().expected_binds(), 4);
}
#[test] fn 19_error_echoes_raw_sql_and_dialect() {
    let e = render("SELECT '?'", DialectKind::Postgres).unwrap_err().to_string();
    assert!(e.contains("SELECT '?'"));
    assert!(e.contains("postgres"));
}
```

- [ ] **Step 2: RED**

Run: `cargo test -p digua_common_db --features postgres --test sql_render_test`
Expected: FAIL（`sql` 模块不存在）。

- [ ] **Step 3: 实现 `src/sql.rs`**

按 Interfaces + 顺序契约 1~3 实现。要点：
- 扫描器状态机：`Normal` / `InSingle` / `InDouble` / `InBacktick` / `InLineComment` / `InBlockComment`。
- `InSingle` 遇 `''` 继续留在 `InSingle`（转义），遇单个 `'` 回 `Normal`。
- PG 分支用 `String::with_capacity(sql.len() + 8)`；MySQL 分支返回 `Cow::Borrowed`（**但仍统计个数**）。
- `SqlRenderError` 实现 `Display`（回显 `raw_sql` + `dialect` + `offset` + `reason`）与 `std::error::Error`。

- [ ] **Step 3b: 给 7+1 个服务 crate 加 `[features]` 段（**S1 阻塞项**）**

> **为什么必须做**：宏内的 `#[cfg(feature = "...")]` 按**调用方 crate** 求值。
> 调用方必须定义与库**同名**的 feature 并**转译**过去，两套 cfg 才天然同步：
> 不开 postgres → 宏的 PG 分支被剔除 → 不引用 `DbPool::Postgres` → 库侧也不需要它；
> 开 postgres → 宏的 PG 分支保留 → 同时 `digua_common_db/postgres` 与 `sqlx/postgres` 被开启 → 变体与类型都存在。

**（a）服务 `[features]` 段**（8 个 crate 统一形态）：

```toml
[features]
default  = ["mysql"]
mysql    = ["sqlx/mysql",    "digua_common_db/mysql",    "digua_common/mysql",    "digua_common_event/mysql"]
postgres = ["sqlx/postgres", "digua_common_db/postgres", "digua_common/postgres", "digua_common_event/postgres"]
sqlite   = ["sqlx/sqlite",   "digua_common_db/sqlite",   "digua_common/sqlite"]
```

**（b）依赖声明：方言 feature 必须「移除硬编码」，改由上面的 `[features]` 转译**：

```toml
# 改前（硬编码方言 feature）
sqlx            = { workspace = true, features = ["mysql", "chrono", "macros"] }
digua_common_db = { path = "../../libs/common-db", features = ["mysql", "openapi"] }
# 改后（方言 feature 走 [features] 转译；非方言 feature 如 openapi/chrono/macros 保留）
sqlx            = { workspace = true, features = ["chrono", "macros"] }
digua_common_db = { path = "../../libs/common-db", features = ["openapi"] }
```

⚠️ **这一步是「默认构建产物零变化」的关键**：若依赖声明里硬写 `"postgres"`，则**默认构建也会带上
`sqlx-postgres` 驱动**（实测：干净构建多花 **189 秒**，`cargo tree -i sqlx-postgres` 在默认构建里出现）。
方言 feature **只能**经服务自己的 `[features]` 转译。

**（c）各服务的差异**：

| 服务 | `default` | 额外需要的 feature | 备注 |
|---|---|---|---|
| `config_service` / `file_service` | `["mysql"]` | — | 零占位符，最简单 |
| `audit_service` | `["mysql"]` | — | 依赖 `digua_common`（门面）+ `digua_common_event` |
| `prue_backend` | `["mysql","sqlite"]` | — | **三方言**：默认同时开 mysql+sqlite（与现状一致） |
| `token_service` | `["mysql"]` | — | 需删自建 `DbPool`（P1-12） |
| `notification_service` | `["mysql"]` | — | 含 `ntf_outbox_publisher` bin |
| `iam_service` | `["mysql"]` | — | |
| `iam_outbox_publisher` | `["mysql"]` | — | 独立二进制 |

**（d）库自身的宏用法测试必须显式带 feature**（设计 §1.4.4 约束 2）：

```rust
// libs/common-db/tests/*_test.rs —— 用了宏的文件加门控
#![cfg(all(feature = "mysql", feature = "postgres"))]
```
否则 `cargo test -p digua_common_db`（default = sqlite）会报
`no variant, associated function, or constant named 'Mysql' found for enum 'DbPool'`。

**验收命令**：

```bash
cargo check -p config_service                              # 默认（mysql）通过
cargo check -p config_service --features postgres          # ← 验证「服务 crate 加 [features] 后能用 PG feature 编译」
cargo check -p config_service --features mysql,postgres    # 显式组合通过
cargo check -p prue_backend --features sqlite              # prue 默认已含 sqlite
cargo test  -p digua_common_db --features "mysql,postgres" # 库的宏测试（需带 feature，见 (d)）

# ★ 直接证明「默认构建产物零变化」：
cargo tree -e features -i digua_common_db | grep -c 'digua_common_db feature "postgres"'   # 期望 0
cargo tree -i sqlx-postgres 2>/dev/null | grep -q sqlx-postgres && echo "FAIL 默认构建带了 PG 驱动"  # 期望无输出
grep -n 'default = ' libs/common-db/Cargo.toml              # 必须仍是 default = ["sqlite"]
```

- [ ] **Step 3c: 修正既有 CI `test-services` job 的包名（顺手修既有缺陷）**

既有 job **今天就跑不起来**（两处都错，我已实测）：
```bash
$ cargo check -p audit-service        # error: package ID specification `audit-service` did not match any packages
$ cargo check -p config_service --features mysql   # error: the package 'config_service' does not contain this feature: mysql
```
`.github/workflows/dialect-matrix.yml` 现状：
```yaml
cargo test --package audit-service --features mysql     # ← 包名 dash（应 audit_service）
cargo test --package config-service --features mysql    # ← 包名 dash + 服务原无 mysql feature
cargo test --package file-service --features mysql      # ← 同上
```
**Step 3b 加了 `[features]` 后，`--features mysql` 变为合法**；再把包名改成下划线：

**既有 `test-services` job 的三条命令必须一并修正**（两层原因，实测见设计 §1.4.6）：
```yaml
# 改前（两层都错：dash 包名 + 服务无 mysql feature）
cargo test --package audit-service  --features mysql
cargo test --package config-service --features mysql
cargo test --package file-service   --features mysql
# 改后
cargo test --package audit_service  --features mysql
cargo test --package config_service --features mysql
cargo test --package file_service   --features mysql
```
> 这不是"改变 CI 语义"，而是**修复一个从未成功过的 job**。
> S4 新增的 PG job 必须用**下划线包名**（`config_service` 等），否则同样跑不起来。

⚠️ **该 job 修正后会第一次真正运行** —— 它此前从未执行过，**可能暴露既有失败**
（测试数据/时序/环境依赖等与 PG 无关的问题）。
**处置**：Step 3c 先在本地跑一遍这三条命令，把**既有失败**与**PG 引入的失败分开归因**；
既有失败单独记 issue，**不阻塞** PG 交付，但必须在 S6 的文档里说明该 job 的**真实基线状态**。

- [ ] **Step 4: 实现 `src/query.rs`（宏）— 已编译验证的最终形态（方案 3′）**

> ⚠️ **抄这段之前先读设计文档 §1.4**。
>
> **最终形态（方案 3′）：宏内保留 `#[cfg(feature = "...")]`**，按**调用方 crate** 求值；
> 调用方 feature 经 Cargo 的 feature 转译与库 feature **天然同步**（Step 3b(a)(b)）。
>
> **两条必须分清的失败模式**：
> - 调用方**没有同名 feature**（或没做转译）→ 宏的分支被**全部剔除** → `E0004` / 展开成 `()` → **S1 编译失败**。
> - 依赖声明里**硬写** `features = ["postgres"]` → 默认构建被迫带 `sqlx-postgres` 驱动 → 破坏「默认构建产物零变化」。
>
> → **保留 cfg + 服务 feature 转译 + 依赖声明不硬写方言 feature**，三者缺一不可。

```rust
#[macro_export]
macro_rules! db_fetch_all {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        let __sql_tpl: &str = &$sql;
        // 构造期计数：宏能「看到」bind 实参的个数
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {                                 // ← 注意是 &$pool（见下方细节 1）
            #[cfg(feature = "mysql")]
            $crate::DbPool::Mysql(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::MySql)
                    .and_then(|rq| rq.expect_binds(__n_binds))    // ← 构造期断言（MySQL 同样校验）
                    .map_err($crate::sql::to_sqlx_error)?;         // ← 用 ?，不要裸 return（细节 2）
                let mut __q = sqlx::query_as::<sqlx::MySql, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*                      // ← 顺序 = 源码顺序
                __q.fetch_all(p).await
            }
            #[cfg(feature = "postgres")]
            $crate::DbPool::Postgres(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Postgres)
                    .and_then(|rq| rq.expect_binds(__n_binds))
                    .map_err($crate::sql::to_sqlx_error)?;
                let mut __q = sqlx::query_as::<sqlx::Postgres, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*
                __q.fetch_all(p).await
            }
            #[cfg(feature = "sqlite")]
            $crate::DbPool::Sqlite(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Sqlite)
                    .and_then(|rq| rq.expect_binds(__n_binds))
                    .map_err($crate::sql::to_sqlx_error)?;
                let mut __q = sqlx::query_as::<sqlx::Sqlite, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*
                __q.fetch_all(p).await
            }
            // 🔴 兜底臂是强制项（设计 §1.4.9）：Cargo 的 feature 统一会让「库有 PG 变体、
            // 而本调用方的 PG 分支被剔除」→ 没有兜底臂就是 E0004 编译失败（已实测）。
            #[allow(unreachable_patterns)]
            _ => Err($crate::sql::to_sqlx_error($crate::sql::SqlRenderError::DialectFeatureDisabled {
                raw_sql: __sql_tpl.to_string(),
                dialect: $crate::DbPool::dialect_of(&$pool),
            })),
        }
    }};
}
```

> **宏的产物类型 = `Result<T, sqlx::Error>`**（内部**已 await 完毕**）。调用方直接 `?`，**不要**再加 `.await`。
> 错误类型统一 `sqlx::Error`；`libs/common-error/src/lib.rs:79` 已有 `impl From<sqlx::Error> for AppError`，
> 所以调用点的 `?` 照旧可用，**错误处理代码零改动**。
>
> 其余 4 个 `db_*` 宏同构（`fetch_one`/`fetch_optional`/`fetch_scalar`/`execute`）。
> `db_execute!` 无 `$ty`，用 `sqlx::query`，**直接返回 `Result<u64>`（rows_affected）**；
> `db_fetch_scalar!` 用 `sqlx::query_scalar`。
>
> **`db_qb_*!`（4 个）用 QueryBuilder 块粘贴**，不走 `render`（`push_bind` 已自动方言化，实测 [3]）：
> ```rust
> #[macro_export]
> macro_rules! db_qb_fetch_all {
>     ($pool:expr, $ty:ty, |$qb:ident| $body:block) => {{
>         match &$pool {
>             #[cfg(feature = "mysql")]
>             $crate::DbPool::Mysql(p) => {
>                 let mut $qb = sqlx::QueryBuilder::<sqlx::MySql>::new("");
>                 $body
>                 $qb.build_query_as::<$ty>().fetch_all(p).await
>             }
>             #[cfg(feature = "postgres")]
>             $crate::DbPool::Postgres(p) => {
>                 let mut $qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("");
>                 $body
>                 $qb.build_query_as::<$ty>().fetch_all(p).await
>             }
>             #[cfg(feature = "sqlite")]
>             $crate::DbPool::Sqlite(p) => {
>                 let mut $qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("");
>                 $body
>                 $qb.build_query_as::<$ty>().fetch_all(p).await
>             }
>             #[allow(unreachable_patterns)]
>             _ => Err(sqlx::Error::Configuration("db dialect feature not enabled".into())),
>         }
>     }};
> }
> ```

**四条不可省略的实现细节**（我实测踩到，设计 §1.4.8）：

| # | 细节 | 写错的后果 |
|---|---|---|
| 1 | `match &$pool`（**不是** `match $pool`） | `$pool` 为拥有值时绑定出 `Pool<DB>`，`Executor` 只为 `&Pool<DB>` 实现 → `E0271 the trait bound Pool<Postgres>: Executor<'_> is not satisfied` |
| 2 | 错误用 `map_err(...)?`（**不要**裸 `return Err(sqlx::Error::...)`） | 裸 return 不做 `From` 转换；调用方返回 `AppError` 时类型不匹配。`?` 形态已实测可行（自定义错误类型 + `From<sqlx::Error>` 亦可） |
| 3 | **不要**用 `async move { ... }` 包住 | `.await` 发生在宏块之外 → inline `format!()` 临时量提前 drop → `E0716 temporary value dropped while borrowed` |
| 4 | 兜底臂**必须**存在 | Cargo 的 feature 统一 → 库有 PG 变体而本调用方 PG 分支被剔除 → 无兜底臂即 `E0004` 编译失败（**已实测**，设计 §1.4.9） |

- [ ] **Step 4b: 与 `QueryBuilder` 对拍（把正确性锚定在 sqlx 自身行为上）**

主理人实测 [3][4][5] 已证明 `QueryBuilder` 在 PG 下自动产出 `$1..$N`，且自研 rewrite 与之等价。
因此**不要靠人肉审阅确认 `render` 正确**，而是加一条对拍测试（放在 `tests/sql_render_test.rs`）：

```rust
/// 对同一批纯占位符 SQL，我方 render 的产物必须与 sqlx QueryBuilder 的产物逐字节一致。
#[test]
fn render_matches_sqlx_query_builder() {
    // 主理人实测 [5] 的原始用例
    let ours = render("SELECT ?::text AS a, ?::text AS b", DialectKind::Postgres).unwrap();
    assert_eq!(ours.sql(), "SELECT $1::text AS a, $2::text AS b");
    assert_eq!(ours.expected_binds(), 2);

    // 与 QueryBuilder 对拍：QueryBuilder::push_bind 内部就是 format_placeholder
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("");
    qb.push("SELECT ").push_bind("a").push("::text AS a, ").push_bind("b").push("::text AS b");
    let built = qb.build();
    assert_eq!(built.sql(), ours.sql(), "我方 render 必须与 sqlx QueryBuilder 产物一致");
}
```

> 这条测试的价值：把「我方重写逻辑是否正确」的判定权交给 sqlx 自己，而不是靠审阅猜测。
> 若未来 sqlx 升级改变了 `format_placeholder` 行为，这条测试会**第一个红**。

- [ ] **Step 5: 实现 `DbPool` 扩展 + `init_pool` PG `after_connect` + `migration_state` + `scope`**

按设计文档 §2.1.3/§2.1.4/§2.1.5/§2.1.6 实现。

- [ ] **Step 6: `common-health` + `common` 门面**

`DbProbe::Postgres` + `with_postgres`；`connect()` 返回 `DbPool`；两个 `Cargo.toml` feature 化（`default = ["mysql"]`）。

- [ ] **Step 7: GREEN + feature 矩阵三连**

```bash
cargo test -p digua_common_db --test sql_render_test --features postgres
cargo test -p digua_common_db --features postgres        # 既有测试不回归
cargo test -p digua_common_db --features mysql           # MySQL 不回归
cargo test -p digua_common_db                            # default(sqlite) 不回归
cargo check -p digua_common_health
cargo check -p digua_common
grep -n 'default = ' libs/common-db/Cargo.toml           # 必须仍是 default = ["sqlite"]
```

- [ ] **Step 8: 真实 PG 上跑 `render` 产物（⬜ 需容器）**

```bash
docker exec coder-test-pg psql -U postgres -c "PREPARE p(text) AS SELECT \$1::text AS a; EXECUTE p('ok');"
```

- [ ] **Step 8b: `u64` → `i64` 清单核对（编译器强制的硬约束，漏一个就编译不过）**

> `sqlx-postgres` **没有任何** unsigned 类型的 `Type`/`Decode` 实现 → 含 `u64` 的字段在 `--features postgres` 下**编译失败**
> （实测：`error[E0277]: the trait bound u64: sqlx::Type<Postgres> is not satisfied`；且 `bind` 与 `try_get` **双向**都不可用）。
> 下表是**全部需要改**的点（已全量 grep 生产代码 + 测试代码）。

**必须改（DB 绑定 / DB 解码）**：

| # | 文件 | 位置 | 现状 | 改为 | 关联 MySQL 列 |
|---|---|---|---|---|---|
| 1 | `services/config-service/src/models/namespace.rs` | `:9` | `pub tenant_id: u64`（`FromRow`） | `pub tenant_id: i64` | `cfg_namespaces.tenant_id BIGINT UNSIGNED`（V002） |
| 2 | `services/file-service/src/models/file_info.rs` | `:14` | `pub tenant_id: u64`（`FromRow`） | `pub tenant_id: i64` | `file_files.tenant_id BIGINT UNSIGNED`（V002） |
| 3 | `services/file-service/src/models/file_info.rs` | `:37` | `pub tenant_id: Option<u64>` | `pub tenant_id: Option<i64>` | 同上（查询入参） |
| 4 | `services/iam-service/src/models/user.rs` | `:9` | `pub tenant_id: u64`（`FromRow`） | `pub tenant_id: i64` | `iam_users.tenant_id` |
| 5 | `services/notification-service/src/models.rs` | `:76` | `pub id: u64`（`FromRow`） | `pub id: i64` | `ntf_announcements.id BIGINT UNSIGNED`（V001） |
| 6 | `services/iam-service/src/routes/internal_membership.rs` | `:62` | `.bind(tenant_id as u64)` | `.bind(tenant_id)`（`i64`） | — |
| 7 | `services/iam-service/tests/tenant_register_test.rs` | `:128`、`:301` | `.bind(resp.tenant_id as u64)` | `.bind(resp.tenant_id)` | — |
| 8 | `services/config-service/tests/tenant_isolation_test.rs` | `:52` | `tenant_id: u64`（测试函数参数） | `tenant_id: i64` | — |

**不需要改（纯 Rust 侧，不进 DB）**（列出来是为了避免误改）：
`file-service/src/config.rs:10 max_file_size`、`iam-service/src/config.rs:68,77,78`（TTL 配置）、
`iam-service/src/state.rs:17`、`notification-service/src/config.rs:43`、
`iam-service/src/dto/auth.rs:44,53 expires_in`、`token-service/src/routes/user_jwt.rs:22 expires_in`、
`iam-service/src/dto/{permission,role,user}.rs` 的 `total: u64`（分页计数，来自 `COUNT(*)` 的 `i64` 再转换）、
`prue_backend/src/dto/menu_config.rs:74 total`、`api-gateway/**`（无 DB 依赖）。

> `total: u64` 是**读**路径：`COUNT(*)` 在 MySQL/PG 都解码为 `i64`，Rust 侧再 `as u64` → **无需改动**。

- [ ] **Step 9: 提交**

```bash
git add libs/common-db libs/common-health libs/common
git commit -m "feat(common-db): dialect-aware SQL renderer, query macros, PG session TZ and marker probe"
```

**风险与回退**
- 风险 1（🔴 **最高，S1 阻塞**）：**宏内 `#[cfg(feature=...)]` 按调用方 crate 求值** → 调用方无同名 feature 时
  分支被全剔除 → `E0004` / 展开成 `()`。**已由双方独立复现**。规避：宏内**不写任何 cfg** + 库侧 feature 常开（Step 3b）。
  评审：`grep -n 'cfg(feature' libs/common-db/src/query.rs` 必须零命中。
- 风险 1b（🟠 **新增**）：**库自身不能用这个宏** —— `libs/common-db` 的 `default = ["sqlite"]` 时，
  其测试里用宏会报 `no variant named 'Mysql' found for enum 'DbPool'`。
  规避：库的宏用法测试加 `#![cfg(all(feature = "mysql", feature = "postgres"))]`；
  且 `cargo test --workspace`（D-6，default features）会因库的测试而失败 → D-6 需改用带 feature 的命令。
- 风险 1c（🟠 **语义变化**）：**`cargo test -p <svc> --features mysql` 的含义变了** —— 命令形式不变，
  但从「给依赖开 feature」变成「给服务 crate 开 feature」。由于库侧 feature 已常开，**开关不改变编译产物**。
  对 `.github/workflows/dialect-matrix.yml` 的 `test-services` job：
  该 job 现状**本来就跑不起来**（包名用 dash + 服务无 `mysql` feature，见 Step 3c），
  Step 3b/3c 之后命令变合法且语义正确 → **不破坏，反而修复**。
  唯一需注意：`cargo test --workspace`（default）会带上每个服务的 `default = ["mysql"]` → 只含 MySQL，行为与现状一致。
- 风险 2（中）：宏里"数 bind 个数"的写法（`0usize $(+ { let _ = &$bind; 1usize })*`）较取巧。若不接受，退化为**显式计数** `let mut __n = 0usize; $( __n += 1; )*`；若仍不行，降级为"只保留 `render` 的字面量断言 + 集成测试"，并在实施日志记录（**不阻塞**，因为 `render` 的 `PlaceholderInLiteral` 与 `expect_binds` 仍是主力防线）。**此点必须在 Step 4 当场验证。**
- 回退：`git revert` S1 commit。S1 不改任何服务，回退无下游影响。

---

### Task S2: `libs/common-event` outbox 泛型化（**与 S3 并行**）

**目标**：`OutboxRepo` 不再绑死 `MySqlPool`/`Transaction<'_, MySql>`；outbox 三条 SQL 在 PG 下正确。

**Files:**
- Modify: `libs/common-event/src/outbox.rs`
- Modify: `libs/common-event/Cargo.toml`（feature 化；`default = ["mysql"]`）
- Modify: `libs/common-event/src/lib.rs`（导出 `AnyTx`/`SqlOutboxRepo`）
- Create: `libs/common-event/tests/outbox_sql_test.rs`（纯 SQL 渲染断言，不需容器）

**Interfaces:**
- `pub enum AnyTx<'a> { #[cfg(feature="mysql")] Mysql(&'a mut sqlx::Transaction<'a, sqlx::MySql>), #[cfg(feature="postgres")] Postgres(&'a mut sqlx::Transaction<'a, sqlx::Postgres>) }`
- `pub trait OutboxRepo { async fn enqueue<P: Event>(&self, tx: &mut AnyTx<'_>, env: &mut Envelope<P>) -> Result<(), OutboxError>; ... }`（其余两方法签名不变）
- `pub struct SqlOutboxRepo { pool: DiguaDbPool, table: String }` + `new`/`with_table`
- `#[deprecated] pub type MySqlOutboxRepo = SqlOutboxRepo;`（保 5 个服务 `use` 不破）

**三条 SQL 的处置（对照设计 §2.2）**

| 方法 | MySQL | Postgres |
|---|---|---|
| `enqueue` | `... VALUES (?,?,?,?) ON DUPLICATE KEY UPDATE id = id` | `... VALUES (?,?,?,?) ON CONFLICT (event_id) DO NOTHING` |
| `claim_batch` | `... LIMIT ? FOR UPDATE SKIP LOCKED` | **同一份**（`render` 处理 `?`→`$1`） |
| `mark_published` | `... = CURRENT_TIMESTAMP(3)` | **同一份**（PG 16 接受） |

- [ ] **Step 1: 写失败测试 `tests/outbox_sql_test.rs`**

断言 `enqueue_sql(DialectKind::Postgres)` 含 `ON CONFLICT (event_id) DO NOTHING` 且不含 `ON DUPLICATE KEY`；
`enqueue_sql(DialectKind::Mysql)` 含 `ON DUPLICATE KEY UPDATE`；
`claim_batch_sql(Postgres)` 经 `render` 后含 `$1` 且含 `FOR UPDATE SKIP LOCKED`；
`mark_published_sql(Postgres)` 含 `CURRENT_TIMESTAMP(3)`（**不得**被改写成 `now()`）。

- [ ] **Step 2: RED** → `cargo test -p digua_common_event --features postgres --test outbox_sql_test` 失败。

- [ ] **Step 3: 实现 `AnyTx` + `SqlOutboxRepo` + 三条 SQL 的方言化**

`enqueue` 的方言分叉：在 `match tx { AnyTx::Mysql(..) => ENQUEUE_MYSQL, AnyTx::Postgres(..) => ENQUEUE_PG }` 选常量，再 `render` 处理 `?`。

- [ ] **Step 4: GREEN + 双 feature 校验**

```bash
cargo test -p digua_common_event --features postgres
cargo test -p digua_common_event --features mysql
cargo check -p digua_common_event            # default
```

- [ ] **Step 5: 更新 5 个消费方的最小编译适配**（不改变行为）

`iam-service` / `notification-service` / `audit-service` / `iam-outbox-publisher` 中把 `MySqlOutboxRepo::…` 的 `enqueue` 调用改为传 `&mut AnyTx::Mysql(tx)`。
（`api-gateway` 只用 event 类型不用 outbox，已核实无需改。）

- [ ] **Step 6: 提交**

```bash
git add libs/common-event services/iam-service services/notification-service services/audit-service services/iam-outbox-publisher
git commit -m "feat(common-event): genericize OutboxRepo over dialects with AnyTx"
```

**风险与回退**
- 风险（高，R-8）：横跨 5 个服务。缓解：`MySqlOutboxRepo` 用 `type` 别名 + `#[deprecated]` 保兼容，行为不变。
- 回退：`git revert`。若别名方案在某些调用点不兼容，退化为**新增 `PgOutboxRepo` 平行实现**（PRD Q5-B），代价是两份 SQL。

---

### Task S3: P0 三服务（prue / config / file）**（与 S2 并行，形态验证切片）**

**目标**：用 3 个低风险服务验证抽象形态；覆盖三种情况——prue（已有双方言骨架）、config/file（`QueryBuilder` 为主 + 少量 `?`）。

**Files:**
- Create: `services/prue_backend/migrations/postgres/V001__init.sql`
- Create: `services/config-service/migrations/postgres/V001__init.sql`, `V002__tenant_cfg.sql`
- Create: `services/file-service/migrations/postgres/V001__init.sql`, `V002__file_tenant_reconcile.sql`
- Modify: `services/prue_backend/src/db.rs`, `service/menu_config_svc.rs`, `Cargo.toml`, `main.rs`
- Modify: `services/config-service/src/db.rs`, `services/config.rs`, `main.rs`, `Cargo.toml`
- Modify: `services/file-service/src/db.rs`, `services/file.rs`, `main.rs`, `Cargo.toml`
- Modify: `services/{prue_backend,config-service,file-service}/tests/common/mod.rs`
- Modify: `services/config-service/tests/tenant_isolation_test.rs`, `services/file-service/tests/tenant_isolation_test.rs`（`u64`→`i64`、`?`→宏）

**Interfaces:**
- 各服务 `pub async fn ensure_schema(pool: &DiguaDbPool) -> anyhow::Result<()>`（签名从 `&MySqlPool` 改）
- `tests/common::{Backend, TestDb, spawn, spawn_all}`

- [ ] **Step 0: `V003__align_bigint_signed.sql` 的双侧动作（S3 前置，与 P0-7 同批做）**

> 背景：`u64` → `i64` 要求 MySQL 侧 `BIGINT UNSIGNED` 列去掉 `UNSIGNED`
> （`sqlx-mysql::int_compatible` 显式拒绝 `UNSIGNED` 标志 → 解不到 `i64`）。
> 项目**无生产数据**（AGENTS.md 明示），且迁移 **forward-only** → **新增** V003，不改 V001/V002。

**MySQL 侧**（`services/<svc>/migrations/mysql/V003__align_bigint_signed.sql`）：
```sql
-- 去掉 UNSIGNED：PG 侧 BIGINT 是有符号的，Rust 侧统一用 i64
ALTER TABLE cfg_namespaces MODIFY COLUMN tenant_id BIGINT NOT NULL DEFAULT 1;
```

**PG 侧**（`services/<svc>/migrations/postgres/V003__align_bigint_signed.sql`）：
**必须存在**（版本号与 MySQL 侧一一对应，PRD B-6），内容是**明确的空操作**：
```sql
-- 空操作迁移：PG 无 UNSIGNED 概念，BIGINT 天然有符号，列定义在 V001/V002 即为 BIGINT。
-- 本文件仅为与 MySQL 侧的 V003 保持版本号/marker 语义对齐而存在。
SELECT 1;
```
**空操作机制（不是靠报错忽略）**：
- PG 侧 V003 的 marker 表名与 MySQL 侧**同名**（如 `cfg_migration_v003_applied`）；
- PG 侧同样创建该哨兵表（`applied_at TIMESTAMPTZ(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3)`）；
- `ensure_schema` 的既有机制（marker 探测 → 未应用则执行 → 建哨兵表）**天然幂等**，
  重复执行 `SELECT 1;` 无副作用 → **不依赖任何"忽略错误"逻辑**。

- [ ] **Step 1: PG 迁移（三服务共 5 个文件）**

按设计 §4.4 对照表逐条翻译。**关键约束**：
- 所有时间列 → `TIMESTAMPTZ` / `TIMESTAMPTZ(3)`（**不是** `TIMESTAMP`）。
- `TINYINT(1)` → `BOOLEAN`。
- `BIGINT UNSIGNED` → `BIGINT`。
- 内联 `INDEX`/`UNIQUE KEY` → 独立 `CREATE INDEX` / `CREATE UNIQUE INDEX`。
- 列 `COMMENT '...'` → **丢弃**。
- 表尾 `ENGINE=/CHARSET=/COLLATE=` → 删除。
- `DEFAULT CURRENT_TIMESTAMP(3)` → **保持原样**。
- `prue_backend` 的 `ON UPDATE CURRENT_TIMESTAMP` → 去掉，改 `DEFAULT CURRENT_TIMESTAMP(3)`；Rust 侧 UPDATE 已显式写 `updated_at`（已核实 `menu_config_svc.rs:164`）。

- [ ] **Step 2: `check-pg-migrations.js` 门禁先跑**

Run: `node scripts/check-pg-migrations.js`
Expected: PASS（若有 `TIMESTAMP` 残留会因 `DATETIME`/`TIMESTAMP` 规则被拦或告警）。

- [ ] **Step 3: 服务侧类型改造**

> **照抄设计文档 §2.7.B 的 4 个真实改造前后示例**（取自 `auth_svc.rs:143/155`、`notification db.rs:245`、`prue menu_config_svc.rs:45`），
> 以及 **§2.7.A 的 QueryBuilder 块粘贴示例**（`config.rs:78`）。不要自行发明形态。

三服务统一模式：
- `Cargo.toml`：加 `postgres` feature（不进 default）。
- `db.rs`：
  - `init_pool` → 删掉本地实现，改调 `digua_common_db::init_pool`（复用 PG 会话 TZ）。
  - `ensure_schema(pool: &DiguaDbPool)`：`probe_marker` + 按方言选迁移文本 + `db_execute!` 逐语句。
  - 迁移常量：`const INIT_SQL_POSTGRES: &str = include_str!("../migrations/postgres/V001__init.sql");`
- `services/*.rs`：
  - `sqlx::query("... ? ...")` → `db_execute!`（写）/ `db_fetch_*`（读）
  - `sqlx::query("... 无 ? ...")` → `db_execute!(pool, sql)` 或 `db_fetch_*`
  - `QueryBuilder::new(...)` → `db_qb_fetch_all!(pool, T, |qb| { ... })`（`and_tenant`/`where_id` 已在 S1 泛型化）
  - `self.pool: MySqlPool` → `DiguaDbPool`
  - `u64` → `i64`（`tenant_id` 字段与 `.bind(tenant_id as u64)`）
  - 方言特有语法（`ON CONFLICT`/`JSON 取文本`）→ 两条 `const` + `match pool.dialect()`，**必须写注释**
- `main.rs`：`db::connect` 返回 `DbPool`（`common::db::connect` 已在 S1 改）。

- [ ] **Step 4: 测试 fixture 参数化**

`tests/common/mod.rs` 实现 `Backend`/`TestDb`/`spawn`/`spawn_all`（设计 §6.1）。**先保留** `spawn_mysql()` 内部转调 `spawn(Backend::MySql).unwrap()`，让既有测试零改动仍绿。

- [ ] **Step 5: 双后端编译 + 测试**

```bash
# 编译（🔵）
cargo check -p prue_backend --features postgres
cargo check -p config_service --features postgres
cargo check -p file_service --features postgres
# 默认不回归（🔵）
cargo check -p prue_backend -p config_service -p file_service
# 测试（⬜ 需容器）
TEST_PG_URL=... cargo test -p config_service --features postgres
TEST_PG_URL=... cargo test -p file_service --features postgres
TEST_PG_URL=... cargo test -p prue_backend --features postgres
cargo test -p config_service --features mysql      # 不回归
```

- [ ] **Step 6: 幂等验证（B-2，⬜）**

```bash
# 重复启动 ensure_schema 两次，第二次必须无错
docker exec coder-test-pg psql -U postgres -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
TEST_PG_URL=... cargo test -p config_service --features postgres integration_test -- --test-threads=1
```

- [ ] **Step 7: 提交**

```bash
git add services/prue_backend services/config-service services/file-service
git commit -m "feat(p0-services): postgres migrations, dialect-aware pool and parameterized test fixtures"
```

**风险与回退**
- 风险 1（高）：`TIMESTAMPTZ` 若漏改某列，运行期才炸（`mismatched types`）。缓解：B-3 逐列 schema 对比 + 每个 model 的集成测试至少读一次该列。
- 风险 2（中，U-6）：`prue_backend` 的 `mysql_sql`/`sqlite_sql` 成对写法 → 三轨后可能与"SQL 单份"冲突。缓解：PG 复用 MySQL 分支的字段列表，仅把 `CAST(routes_json AS CHAR)` 在 PG 下改 `routes_json::text`。
- 回退：`git revert` S3 commit；S1/S2 保持，P1 服务不受影响（这是「先验证抽象再铺开」的价值）。

---

### Task S4: CI job + compose PG + `shared-db.js`

**目标**：PG 服务集成测试有 CI 载体；本地有 PG 编排入口。

**Files:**
- Modify: `.github/workflows/dialect-matrix.yml`（新增 `test-services-postgres` job，**不动**既有 job）
- Modify: `infrastructure/docker-compose-infra.yml`（`postgres` service + `profiles: ["pg"]`）
- Modify: `infrastructure/docker-compose-infra.dev.yml`（端口 5434）
- Modify: `scripts/lib/shared-db.js`（`PG_CONTAINER` + `ensurePostgres`）
- Create: `scripts/check-dialect-features.js`（方言 feature 一致性门禁，见 Step 1b）
- Modify: `package.json`（`check` 聚合脚本加入 `node scripts/check-dialect-features.js`）
- Modify: `docs-site/guides/testing.md`（PG 容器段落）

- [ ] **Step 1: workflow 加 job**

按设计 §7.1 的 YAML 落地。校验语法：
```bash
node -e "const y=require('js-yaml');y.load(require('fs').readFileSync('.github/workflows/dialect-matrix.yml','utf8'));console.log('yaml ok')"
```

- [ ] **Step 1b: 新增 `scripts/check-dialect-features.js`（方言 feature 一致性门禁）**

> 设计 §1.4.7：方案 3 的「库侧 feature 常开」是**结构性前提**，漏改 → 编译期 `E0599`。
> 把它做成**门禁**比写进文档可靠。脚本与 `check-db-naming.js` 并列进 CI。

Create: `scripts/check-dialect-features.js`

| # | 校验项 | 判据 | 失败含义 |
|---|---|---|---|
| 1 | 宏内不得有 `#[cfg(feature` | `libs/common-db/src/query.rs` 零命中 | 方案 3 被破坏 → 分支被全剔除 |
| 2 | 宏必须用 `match &$pool` | `query.rs` 中 `match &$pool` 次数 == 宏数量 | 拥有值池 → `E0277 Executor not satisfied` |
| 3 | 库侧 feature 常开 | 每个服务 crate 的 `digua_common_db` 依赖 `features` 含 `mysql` **和** `postgres` | 宏引用不存在的 `DbPool::Postgres` |
| 4 | 服务 feature 转发完整 | 每个服务 `[features]` 有 `default`/`mysql`/`postgres`(/`sqlite`)，且 `postgres` 转发到 `digua_common_db/postgres` | `--features postgres` 命令不合法 → CI 跑不起来 |
| 5 | 无 default 含 postgres | `libs/common-db` 与各服务 `[features].default` 均不含 `postgres` | 违反 D-9 / A-7 / F-3 |

**脚本骨架**（Node，仿 `check-db-naming.js` 的风格；TOML 用正则子集解析，不引新依赖）：
```js
#!/usr/bin/env node
// 方言 feature 一致性门禁 —— 校验「库侧 feature 常开」+「服务 feature 转发完整」
// 设计：docs/superpowers/specs/2026-11-02-postgresql-service-layer-support-design.md §1.4.7
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..');
const SERVICES = ['audit-service','config-service','file-service','iam-service',
                  'notification-service','prue_backend','token-service','iam-outbox-publisher'];
const errors = [];

// 1) 宏内不得有 cfg；2) 必须用 match &$pool
const q = fs.readFileSync(path.join(ROOT,'libs/common-db/src/query.rs'),'utf8');
if (/cfg\s*\(\s*feature/.test(q)) errors.push('query.rs: 宏内不得出现 #[cfg(feature = ...)]');
const macros = (q.match(/macro_rules!\s+db_\w+!/g) || []).length;
const guards = (q.match(/match\s+&\$pool/g) || []).length;
if (guards < macros) errors.push(`query.rs: match &$pool 出现 ${guards} 次，少于宏数量 ${macros}`);

// 3) 库侧 feature 常开；4) 服务 feature 转发；5) default 不含 postgres
for (const svc of SERVICES) {
  const toml = fs.readFileSync(path.join(ROOT,'services',svc,'Cargo.toml'),'utf8');
  const dep = toml.match(/digua_common_db\s*=\s*\{[^}]*\}/s);
  if (!dep) { errors.push(`${svc}: 缺少 digua_common_db 依赖声明`); continue; }
  const depFeatures = dep[0].match(/features\s*=\s*\[([^\]]*)\]/s);
  const list = depFeatures ? depFeatures[1] : '';
  for (const f of ['mysql','postgres']) {
    if (!list.includes(`"${f}"`)) errors.push(`${svc}: digua_common_db 依赖必须含 "${f}"（库侧 feature 常开）`);
  }
  const featBlock = toml.match(/\[features\]([\s\S]*?)(
\[|$)/);
  if (!featBlock) { errors.push(`${svc}: 缺少 [features] 段`); continue; }
  for (const f of ['default','mysql','postgres']) {
    if (!new RegExp(`^\s*${f}\s*=`, 'm').test(featBlock[1])) errors.push(`${svc}: [features] 缺少 ${f}`);
  }
  if (!/postgres\s*=\s*\[[^\]]*"digua_common_db\/postgres"/s.test(featBlock[1]))
    errors.push(`${svc}: postgres feature 必须转发到 digua_common_db/postgres`);
  const def = featBlock[1].match(/default\s*=\s*\[([^\]]*)\]/);
  if (def && def[1].includes('postgres')) errors.push(`${svc}: default 不得含 postgres`);
}
const libToml = fs.readFileSync(path.join(ROOT,'libs/common-db/Cargo.toml'),'utf8');
const libDef = libToml.match(/default\s*=\s*\[([^\]]*)\]/);
if (!libDef || !libDef[1].includes('sqlite')) errors.push('libs/common-db: default 必须保持 ["sqlite"]');
if (libDef && libDef[1].includes('postgres')) errors.push('libs/common-db: default 不得含 postgres');

if (errors.length) { console.error(`FAIL: ${errors.length} 处方言 feature 不一致`); errors.forEach(e=>console.error('  '+e)); process.exit(1); }
console.log('PASS: 方言 feature 一致性检查通过');
```

**可选的 cargo 侧补充校验**（需 cargo，放 CI 里）：
```bash
for svc in config_service file_service audit_service token_service notification_service iam_service iam_outbox_publisher prue_backend; do
  cargo tree -p "$svc" -e features -i digua_common_db | grep -q 'feature "postgres"'     || { echo "FAIL: $svc 未启用 digua_common_db/postgres"; exit 1; }
done
```

- [ ] **Step 2: compose + shared-db.js**

按设计 §7.2 / §6.3 落地。校验：
```bash
node -e "const {PG_CONTAINER,ensurePostgres}=require('./scripts/lib/shared-db.js'); if(PG_CONTAINER!=='coder-test-pg') throw new Error('bad'); console.log('shared-db ok')"
docker compose -f infrastructure/docker-compose-infra.yml --profile pg config >/dev/null && echo "compose ok"
docker compose -f infrastructure/docker-compose-infra.yml config | grep -c postgres || echo "默认不启 PG（期望 0）"
```

- [ ] **Step 3: 本地真实拉起验证（⬜）**

```bash
docker compose -f infrastructure/docker-compose-infra.yml --profile pg up -d postgres
docker compose -f infrastructure/docker-compose-infra.yml --profile pg ps
node -e "require('./scripts/lib/shared-db.js').ensurePostgres('127.0.0.1',5434).then(ok=>console.log('pg reachable:',ok))"
docker compose -f infrastructure/docker-compose-infra.yml --profile pg down
```

- [ ] **Step 4: 提交**

```bash
git add .github/workflows/dialect-matrix.yml infrastructure/docker-compose-infra.yml infrastructure/docker-compose-infra.dev.yml scripts/lib/shared-db.js docs-site/guides/testing.md
git commit -m "ci(dialect): add postgres service test job, compose profile and shared-db helper"
```

**风险与回退**
- 风险（中，R-11）：CI 时长上升。缓解：`rust-cache` + PG job 与 MySQL job 天然并行。
- 回退：删 job / 删 compose service。

---

### Task S5: 其余 5 单元（audit / token / notification / iam / iam-outbox-publisher）

> **并行度**：S5a（audit）、S5b（token）、S5c（notification）**可并行**（互不依赖）；S5d（iam）与 S5e（publisher）共享 `outbox_archive.rs` 逻辑，建议同一人顺序做。

**Files（按单元）:**

**S5a `audit-service`**
- Create: `services/audit-service/migrations/postgres/V001__init.sql`
- Modify: `services/audit-service/src/{db.rs,consumer.rs,services/audit.rs,services/cleanup.rs,main.rs,Cargo.toml}`

**S5b `token-service`**
- Create: `services/token-service/migrations/postgres/V001__init.sql`, `V002__add_client_user_and_expiry.sql`
- Modify: `services/token-service/src/{db.rs,config.rs,service/oauth_client_svc.rs,service/refresh_svc.rs,service/revoke_svc.rs,service/auth_code_svc.rs,main.rs,Cargo.toml}`
- Modify: `services/token-service/tests/{common/mod.rs,migration_marker_test.rs,integration_test.rs}`

**S5c `notification-service`**
- Create: `services/notification-service/migrations/postgres/V001__init.sql`, `V002__ntf_tenant.sql`, `V003__ntf_membership_projection.sql`
- Modify: `services/notification-service/src/{db.rs,scheduler.rs,service/announcement_service.rs,bin/outbox_publisher.rs,main.rs,Cargo.toml}`
- Modify: `services/notification-service/tests/common/mod.rs` + 4 个测试文件

**S5d `iam-service`**
- Create: `services/iam-service/migrations/postgres/V001__init.sql` … `V008__rebind_tenant_admin_permissions.sql`（**8 个**）
- Modify: `services/iam-service/src/{db.rs,outbox_archive.rs,routes/internal_membership.rs,routes/internal_users.rs,routes/migration_state.rs,service/{auth_svc,org_svc,role_svc,user_svc,permission_svc,perm_change,tenant_svc,user_roles}.rs,main.rs,Cargo.toml}`
- Modify: `services/iam-service/tests/common/mod.rs` + 12 个测试文件

**S5e `iam-outbox-publisher`**
- Modify: `services/iam-outbox-publisher/src/{main.rs,outbox_archive.rs}`, `Cargo.toml`

**关键差异处置（逐条，见设计 §3 归属表）**

| 位置 | 差异 | 处置 |
|---|---|---|
| `audit-service/src/consumer.rs:108` | `ON DUPLICATE KEY UPDATE received_at = CURRENT_TIMESTAMP(3)` | → `ON CONFLICT (event_id) DO UPDATE SET received_at = CURRENT_TIMESTAMP(3)` |
| `token-service/src/db.rs:41` | 自建 `DbPool` 枚举 | **删除**，改用 `digua_common_db::DbPool`（P1-12） |
| `token-service/src/db.rs:80` | `let DbPool::Mysql(p) = pool;` | 改 `match pool.as_mysql()/as_postgres() { .. }`（**E0005 源**） |
| `token-service/config.rs:40` | `enum DbType { Mysql }` | 加 `Postgres` 变体，**保留"未知值忽略"行为**（R-12） |
| `token-service/service/oauth_client_svc.rs:311,348` | UPDATE 未显式写 `updated_at` | 补 `updated_at = now()`（Q2-A） |
| `notification-service/src/db.rs:156-157` | `.bind(a.pinned as i8)` | 去掉 `as i8`，改 `bool`（D-5） |
| `notification-service/src/db.rs:246,586`、`announcement_service.rs:66,127` | `INSERT IGNORE` / `ON DUPLICATE KEY` | → `ON CONFLICT DO NOTHING` / `ON CONFLICT (…列…) DO UPDATE` |
| `iam-service/src/outbox_archive.rs:50,67` | `GET_LOCK`/`RELEASE_LOCK` | → `pg_try_advisory_lock(hashtext($1))` / `pg_advisory_unlock(hashtext($1))`。**⚠️ 语义不完全等价**（session 级一致、释放时机一致；差异在 `hashtext` 是 int4 → 有碰撞面；当前只有 1 个锁名 `iam_outbox_archive`，实际风险为零）→ 详见设计 §3「分布式锁语义差异」。**未来锁名增多**时改用 `pg_try_advisory_lock($1::bigint)` + Rust 侧算稳定 64 位哈希 |
| `iam-service/src/routes/internal_membership.rs:55` | `GROUP_CONCAT(ur.role_id)` | → `STRING_AGG(ur.role_id::text, ',')`；Rust 侧 `.bind(tenant_id as u64)` → `i64` |
| `iam-service/src/outbox_archive.rs:82,102,106,118,141,159` | `NOW(3)` | → `now()`（D-6） |
| 同上 | `INTERVAL ? HOUR` / `INTERVAL ? DAY` | → `($1 * INTERVAL '1 hour')` / `($1 * INTERVAL '1 day')`（D-7） |
| `iam-service/tests/tenant_register_test.rs:267,279` | `information_schema.statistics` | → `pg_catalog.pg_indexes`（仅测试） |

- [ ] **Step 1（S5a）：audit-service** —— 迁移 + 类型改造 + `ON CONFLICT`
- [ ] **Step 2（S5b）：token-service** —— 迁移 + **删本地 `DbPool`** + `DbType::Postgres` + `updated_at`
- [ ] **Step 3（S5c）：notification-service** —— 迁移 + `bool` 修正 + `ON CONFLICT`
- [ ] **Step 3b（S5 范围微扩，主理人已批准）：抽出 `outbox_archive` 公共模块**

> `services/iam-service/src/outbox_archive.rs` 与 `services/iam-outbox-publisher/src/outbox_archive.rs`
> **内容完全重复**（各含 10 处 `NOW(3)` + `GET_LOCK`/`RELEASE_LOCK`）。
> 不抽公共模块 → PG 适配要改两遍 → 极易漂移。

**做法**：把归档逻辑抽到一处（如 `libs/common-event/src/outbox_archive.rs`，或
`iam-service` 内的一个共享模块由 publisher bin 引用）。抽取时**同时**做方言化：

| 现有写法 | PG 写法 | 说明 |
|---|---|---|
| `NOW(3)`（10 处） | **`now()`** | ⚠️ `NOW(3)` 在 PG 是 `function now(integer) does not exist`（主理人实测）。**注意与 `CURRENT_TIMESTAMP(3)` 区分**——后者在 PG **合法**（实测），但返回 `timestamptz`；列已改为 `TIMESTAMPTZ(3)`，两者语义等价。**推荐统一用 `CURRENT_TIMESTAMP(3)`**（与 MySQL 版字面量一致，且 PG 合法） |
| `NOW(3) - INTERVAL ? HOUR` | `CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 hour')` | ⚠️ `INTERVAL $1 HOUR` 在 PG 是 `syntax error at or near "$1"`（实测）。**只做占位符改写不够**，必须改成乘法式 |
| `NOW(3) - INTERVAL ? DAY` | `CURRENT_TIMESTAMP(3) - ($1 * INTERVAL '1 day')` | 同上 |
| `GET_LOCK(?, 0)` / `RELEASE_LOCK(?)` | `pg_try_advisory_lock(hashtext($1))` / `pg_advisory_unlock(hashtext($1))` | 详见设计 §3「分布式锁语义差异」 |

**抽取后**：iam-service 与 iam-outbox-publisher **共用同一份方言化代码** → 只改一次。

- [ ] **Step 4（S5d）：iam-service** —— 8 个迁移 + `GET_LOCK`/`GROUP_CONCAT`/`NOW(3)`/`INTERVAL`
- [ ] **Step 5（S5e）：iam-outbox-publisher** —— 与 S5d 同构改造
- [ ] **Step 6: 每单元双后端验证**

```bash
for pkg in audit_service token_service notification_service iam_service iam_outbox_publisher; do
  cargo check -p $pkg --features postgres || echo "FAIL $pkg"
  cargo check -p $pkg || echo "FAIL(default) $pkg"
done
TEST_PG_URL=... cargo test -p audit_service --features postgres
TEST_PG_URL=... cargo test -p token_service --features postgres
TEST_PG_URL=... cargo test -p notification_service --features postgres
TEST_PG_URL=... cargo test -p iam_service --features postgres
```

- [ ] **Step 7: iam 专项验证（⬜，R-7）**

```bash
# 1) 分布式锁语义：两个并发 run_once 只有一个归档
TEST_PG_URL=... cargo test -p iam_service --features postgres outbox_archive_test -- --nocapture
# 2) membership 的 STRING_AGG 输出格式与 MySQL 侧一致（逗号分隔、无空格）
TEST_PG_URL=... cargo test -p iam_service --features postgres internal_membership_test
```

- [ ] **Step 8: 分单元提交（每单元一个 commit，便于独立回退）**

```bash
git add services/audit-service && git commit -m "feat(audit): postgres migration and dialect-aware queries"
git add services/token-service && git commit -m "feat(token): postgres support, drop local DbPool, add DbType::Postgres"
git add services/notification-service && git commit -m "feat(notification): postgres migration and ON CONFLICT upserts"
git add services/iam-service && git commit -m "feat(iam): postgres migrations, advisory lock, STRING_AGG and interval fixes"
git add services/iam-outbox-publisher && git commit -m "feat(iam-outbox-publisher): postgres support for archive task"
```

**风险与回退**
- 风险 1（高，R-7）：`pg_try_advisory_lock` 与 `GET_LOCK(name,0)` 语义差异。缓解：沿用"单连接持有锁"约束；专项测试（Step 7）。
- 风险 2（高）：iam 122 个占位符适配点 + 8 个迁移，量最大。缓解：拆成 S5d 单独 commit；先跑 `cargo check --features postgres` 做红→绿。
- 风险 3（中，R-12）：`DbType` 新增变体可能踩既有"未知值忽略"测试。缓解：保留忽略行为，仅加已知变体；同步更新测试断言。
- 回退：**按单元 `git revert`**，i 与其他单元解耦（S5 内部无跨单元依赖，除 S5e↔S5d 的 `outbox_archive.rs` 逻辑）。

---

### Task S6: 文档同步

**Files:**
- Modify: `services/{iam-service,audit-service,token-service,notification-service,config-service,file-service,prue_backend}/AGENTS.md`（「数据库方言」小节）
- Modify: `contracts/db-conventions.md`（新增 PG 类型映射章节，内容取设计 §4.4）
- Modify: `docs/superpowers/specs/dialect-matrix.md`（PG 差异列 + 服务×方言测试矩阵）
- Modify: `docs-site/guides/testing.md`（若 S4 未完成）

- [ ] **Step 1: 7 个 `AGENTS.md`** —— 「支持 MySQL 和 SQLite」→「支持 MySQL / PostgreSQL / SQLite（`migrations/{mysql,postgres,sqlite}/`）」
- [ ] **Step 2: `contracts/db-conventions.md`** —— 贴设计 §4.4 的对照表 + 明确「PG 时间列用 `TIMESTAMPTZ`」与「`u64` 用 `i64`」
- [ ] **Step 3: `dialect-matrix.md`** —— 支持表加 PG 行；新增「服务 × 方言」覆盖矩阵（✅/🟡/⚠️）
- [ ] **Step 4: 校验**

```bash
node scripts/check-doc-links.js
node scripts/check-spec.js
node scripts/check-contracts.js   # 若存在
node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'
```

- [ ] **Step 5: 提交**

```bash
git add services/*/AGENTS.md contracts/db-conventions.md docs/superpowers/specs/dialect-matrix.md docs-site/guides/testing.md
git commit -m "docs(pg): sync dialect support across service guides, db conventions and matrix"
```

**风险与回退**：低。`git revert`。

---

## 依赖包列表（新增/变更）

| 包 | 版本 | 用途 | 变更类型 |
|---|---|---|---|
| `sqlx` | 0.8.6（workspace 既有） | `postgres` / `mysql` / `sqlite` feature | **新增 feature 启用**（不加新 crate） |
| `testcontainers` | 0.20（既有 dev-dep） | PG 容器 | 无版本变更 |
| `testcontainers-modules` | 0.8（既有 dev-dep） | 加 `postgres` feature | **feature 新增** |
| `js-yaml` | root devDeps（既有） | 校验 workflow YAML | 无变更 |

> **无新增第三方 crate**。全部能力由既有依赖的 feature 开关提供。这是本设计的重要约束（避免 supply-chain 面扩大）。

---

## 共享知识（跨文件约定）

1. **`?` 是唯一允许出现在业务 SQL 源码里的占位符**。禁止在 `services/**/*.rs` 里写 `$1`/`:name`。门禁：`grep -rn '\$[0-9]' services/*/src --include=*.rs` 应为空（`$` 后接数字）。

2. **PG 时间列一律 `TIMESTAMPTZ` / `TIMESTAMPTZ(3)`**。Rust 侧保持 `DateTime<Utc>`（75 处，不改类型）。理由：sqlx 的 `type_compatible` 是严格相等，`DateTime<Utc>` 只映射 `TIMESTAMPTZ`。
   - 门禁：`grep -rnE '\bTIMESTAMP\b' services/*/migrations/postgres/` 必须零命中（`TIMESTAMPTZ` 不匹配 `\bTIMESTAMP\b`）。

3. **`u64` → `i64`**：`tenant_id` 等字段与所有 `.bind(x as u64)` 改 `i64`。PG `BIGINT` 有符号；MySQL 侧 `BIGINT UNSIGNED` 需 ALTER 成 `BIGINT`（新迁移，不改旧迁移）。

4. **布尔列**：MySQL `TINYINT(1)` ↔ PG `BOOLEAN`，Rust 侧一律 `bool`。**禁止** `.bind(x as i8)` 绑定布尔列。

5. **`DbPool` 使用规约**：
   - ❌ `let DbPool::Mysql(p) = pool;`（E0005）
   - ❌ 穷尽 `match pool { Mysql(..) => .., Postgres(..) => .. }`（E0004）
   - ✅ 业务层用 `db_*!` 宏；`match` 只出现在 `ensure_schema` 与显式方言分支处，且必带 `_ =>`。

6. **迁移文件命名与版本**：`migrations/postgres/V00N__<same-name-as-mysql>.sql`，序号与后缀**必须**与 MySQL 侧一致。marker table 名两端共用一份常量。

7. **`db_*!` 宏的错误类型**：统一为 `sqlx::Error`。`render` 的 `SqlRenderError` 在宏内被包成 `sqlx::Error::Configuration`（`sqlx::Error::Configuration(BoxDynError)`，`SqlRenderError` 满足 `Error + Send + Sync`），**调用点不需要改错误类型**。

7b. **`render` 的返回契约（不许绕过）**：`render` 返回 `#[must_use] RewrittenQuery`，
   - `sql()` 只能通过 `expect_binds(actual)` 校验后的实例取得；
   - `expect_binds` 是**消耗式**（`self`），未校验拿不到 SQL；
   - **MySQL/SQLite 分支也必须调用 `expect_binds`**（SQL 不变，但个数必须校验）——这是"编号错位在 MySQL 上静默"的唯一封堵；
   - 禁止 `#[allow(unused_must_use)]` / `let _ = render(..)` 之类绕过（Review Focus #1 会检查）。

7c. **改造形态照抄设计文档**：`?` 字面量 → **§2.7.B** 的 4 个真实示例；`QueryBuilder` → **§2.7.A** 的块粘贴示例。不要自行发明第三种写法。

7d. **服务 crate 必须有 `[features]` 段**（S1 起）：
```toml
[features]
default  = ["mysql"]
mysql    = ["digua_common_db/mysql",    "digua_common/mysql",    "digua_common_event/mysql"]
postgres = ["digua_common_db/postgres", "digua_common/postgres", "digua_common_event/postgres"]
sqlite   = ["digua_common_db/sqlite",   "digua_common/sqlite"]
```
**用途**：仅让 `cargo test -p <svc> --features postgres` 这条 CI 命令**合法**（库侧已常开，转译是幂等的，
**开关不改变编译产物**）。
**依赖声明**：库侧 feature **必须常开** —— `digua_common_db = { path = "...", features = ["mysql","postgres","sqlite","openapi"] }`。
**原因**：宏内不能写 cfg、也不能下沉到库函数（设计 §1.4.2 三方案实测）→ 库侧变体与 `sqlx::Postgres` 必须恒存在。

7f. **🔴 兜底臂强制**：每个 `db_*!` / `db_qb_*!` 宏的 `match` 必须带 `_ => Err(...)`，
错误信息指名道姓（方言 + 原因），不得返回空结果 / panic / 静默降级。
理由：Cargo 的 feature 统一会把同名 feature 合并到整个 workspace → 库的 feature 集合是
**所有成员请求的并集**，不等于任何单个成员的集合 → 未来新增第 4 个方言时可能「库有变体、宏没覆盖」。
检查：`grep -c 'unreachable_patterns' libs/common-db/src/query.rs` ≥ 宏数量。

7g. **方言 feature 一致性门禁**：`node scripts/check-dialect-features.js`（S4 落地）
必须 PASS。它是「库侧 feature 常开」这条结构性前提的**唯一自动化保障**。

7e. **CI 包名用下划线**：workspace 包名是 `config_service` / `audit_service` / `file_service` /
`token_service` / `notification_service` / `iam_service` / `iam_outbox_publisher` / `prue_backend`
（**不是** dash 形式）。`cargo -p audit-service` 会报 `package ID specification did not match any packages`。

8. **测试 fixture**：新测试用 `spawn_all()`（自动双后端）；既有测试保持 `spawn_mysql()`（内部转调）。**PG 不可用 → 跳过不失败**。

9. **环境变量**：`TEST_PG_URL` 存在 → 直连共享容器；不存在 → testcontainers；两者皆不可用 → 跳过。

10. **OpenAPI 零变更**：任何 PG 相关改动都不得触碰 `#[utoipa::path]` 与 `src/openapi.rs`。门禁：`node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'`。

---

## 完成判据

1. **S0**：`node scripts/check-db-naming.js` 对 PG 语料零误报；`node scripts/check-pg-migrations.js` 存在且 PASS。
2. **S1**：`sql_render_test` 全绿（含字面量内 `?` 断言）；`libs/common-db` 三方言 feature 组合测试不回归；`default = ["sqlite"]` 未变。
3. **S2**：`outbox_sql_test` 双后端断言通过；`common-event` 三 feature 组合编译通过。
4. **S3**：3 个 P0 服务 `cargo check --features postgres` 通过；PG 容器内 `ensure_schema` 幂等；MySQL 不回归。
5. **S4**：`dialect-matrix.yml` YAML 合法且新 job 存在；`compose --profile pg config` 通过；默认 `config` 不含 postgres。
6. **S5**：5 个单元双后端编译通过；iam 的 advisory lock 与 STRING_AGG 专项测试通过。
7. **S6**：7 个 `AGENTS.md` + `db-conventions.md` + `dialect-matrix.md` 均含 PG；openapi 零 diff。
8. **全量**：
   - `grep -rn "postgres" services/*/Cargo.toml | grep default` 无输出（无服务 default 含 postgres）；
   - `grep -n 'default = ' libs/common-db/Cargo.toml` 仍为 `default = ["sqlite"]`；
   - `grep -n 'cfg(feature' libs/common-db/src/query.rs` **零命中**（宏内不得有 cfg）；
   - `cargo check -p config_service --features postgres` 通过（服务 crate 的 PG feature 可用）。
