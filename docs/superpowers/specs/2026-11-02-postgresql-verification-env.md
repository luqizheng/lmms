# PG 支持实施 — 验证环境速查（主理人实测）

> 本文件是本次任务的环境事实记录，供实施者（工程师/QA）直接采信，**不必重新摸索**。
> 实测时间：2026-11-02。全部结论均在本机实际执行验证，非文档推断。

## 1. 编译环境：本机 MSVC **可用**（推翻 AGENTS.md 的旧表述）

AGENTS.md 写「Windows 原生 MSVC 编译不可用」，**该结论只对依赖 `ring` / `aws-lc-rs`
（`tls-rustls-aws-lc-rs`）的 crate 成立**。走 `tls-rustls`（纯 Rust）的 crate 在 MSVC 上可正常编译。

| crate | 命令 | 实测结果 |
|---|---|---|
| `digua_common_db` | `cargo check -p digua_common_db --features postgres` | ✅ 通过（9.01s，2 个既有 unused warning） |
| `digua_common_event` | `cargo check -p digua_common_event` | ✅ 通过（14.30s） |
| `config_service` | `cargo check -p config_service` | ✅ 通过（8.37s，需设 SWAGGER_UI_DOWNLOAD_URL） |
| `iam_service` | `cargo check -p iam_service` | 见 §5（依赖 aws-lc-rs，耗时长） |

### 必设的两个环境变量

```bash
# 1. 独立的 target 目录，避免与 docker volume 抢仓库根的 target/
export CARGO_TARGET_DIR="D:/kudi-project/templates/low-memory-micro-services/.target/pgtest"

# 2. Swagger UI zip 走本地 file://，否则 build.rs 联网下载会超时
#    （infrastructure/vendor/swagger-ui-v5.17.14.zip 本地已存在，4.4MB）
export SWAGGER_UI_DOWNLOAD_URL="file:///D:/kudi-project/templates/low-memory-micro-services/infrastructure/vendor/swagger-ui-v5.17.14.zip"
```

> 不设 `SWAGGER_UI_DOWNLOAD_URL` 时的典型失败：
> `utoipa-swagger-ui` build.rs → `curl: (28) Connection timed out after 300899 milliseconds`
> → `panicked at .../utoipa-swagger-ui-9.0.2/build.rs:216:50: failed to download Swagger UI`
> 这是**网络问题，不是代码问题**。

### 沙箱限制

- **`wsl.exe` 被程序黑名单硬拦截**，不可绕过、不接受审批。走 WSL2 的路径（`node scripts/run-tests-host.js`）在本会话不可用。
- 因此依赖 `tls-rustls-aws-lc-rs` 的 crate 若在本机编译失败，需在能跑 WSL2 的环境复验。

## 2. 数据库环境：Docker + PG 16 容器**现成可用**

| 项 | 值 |
|---|---|
| Docker Server | 29.8.2 ✅ |
| 容器 `pgct` | `postgres:16`（16.15），`POSTGRES_PASSWORD=test`，`POSTGRES_DB=test_db` |
| 容器 `aipm-pg-dev` | `postgres:16`（另有用途，勿动） |
| 连接方式 | `docker exec pgct psql -U postgres -d test_db -c "..."` |

**PG 集成测试有真实环境可跑**，不依赖 Postgres.app。

## 3. PG 方言差异 —— 真实 PG 16.15 实测结论（权威）

| # | 断言 | 实测 | 结论 |
|---|---|---|---|
| 1 | `SELECT CURRENT_TIMESTAMP(3)` | ✅ 合法，返回 `2026-11-02 11:11:48.681+00` | **可用，9 处无需改写** |
| 2 | `CREATE TABLE t (a JSON, b JSONB)` | ✅ 合法 | **JSON 列保持 `JSON` 原样**，不必改 JSONB |
| 3 | `CREATE TABLE t (a DATETIME)` | ❌ `type "datetime" does not exist` | 57 处 → `TIMESTAMP` |
| 4 | `SELECT 1 WHERE 1 = ?` | ❌ `syntax error at or near ";"` | `?` 占位符在 PG 失效 |
| 5 | `SELECT '{"a":1}'::jsonb ? 'a'` | ✅ `t` | `?` 是 JSONB 存在性操作符 |
| 6 | `... ON DUPLICATE KEY UPDATE id=id` | ❌ `syntax error at or near "DUPLICATE"` | 约 12 处 → `ON CONFLICT` |
| 7 | `CREATE TABLE t (a TINYINT(1))` | ❌ `type "tinyint" does not exist` | 5 处 → `BOOLEAN` |
| 8 | `CREATE TABLE t (id INT COMMENT 'x')` | ❌ `syntax error at or near "COMMENT"` | 57 处内联注释 → `COMMENT ON COLUMN` 或丢弃 |
| 9 | `CREATE TABLE t (id INT, INDEX idx_x (id))` | ❌ `type "idx_x" does not exist` | 26 处 → 独立 `CREATE INDEX` |
| 10 | `CREATE TABLE t (id INT) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4` | ❌ `syntax error at or near "ENGINE"` | 18 处表尾选项删除 |

### PG 迁移必须改写的 DDL 差异（收敛后）

| MySQL 写法 | 次数 | PG 写法 |
|---|---|---|
| `DATETIME` / `DATETIME(3)` | 57 | `TIMESTAMP` / `TIMESTAMP(3)` |
| `TINYINT(1)` | 5 | `BOOLEAN` |
| 列内联 `COMMENT '...'` | 57 | `COMMENT ON COLUMN t.c IS '...'` 或丢弃 |
| 内联 `INDEX idx_x (...)` | 26 | 独立 `CREATE INDEX idx_x ON t (...)` |
| 内联 `UNIQUE KEY uk_x (...)` | 10 | 独立 `CREATE UNIQUE INDEX uk_x ON t (...)` |
| `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=...` | 18 | 删除 |
| `AUTO_INCREMENT` | 9 | `GENERATED ALWAYS AS IDENTITY` |

### PG 迁移**保持原样**的项（已实测确认合法）

- `CURRENT_TIMESTAMP` 与 `CURRENT_TIMESTAMP(3)`（PG 接受精度参数）
- `JSON` 列型（PG 原生支持；JSONB 属 P2 增强）
- `NOW()`（PG 有；注意返回 timestamptz，与 MySQL naive datetime 语义不同）

### 运行时 SQL 差异

| MySQL | 次数 | PG |
|---|---|---|
| `?` 占位符 | 102 处调用点 | `$1..$N` |
| `ON DUPLICATE KEY UPDATE` | 约 12 | `ON CONFLICT (冲突目标列) DO UPDATE SET` |
| `information_schema.tables` + `DATABASE()` | 探测多处 | `current_schema()` |
| `GROUP_CONCAT` | 2 | `STRING_AGG` |
| `GET_LOCK` / `RELEASE_LOCK` | iam-service | `pg_advisory_lock`（语义敏感） |
| `information_schema.statistics` | iam-service | PG 无此视图 |

## 4. 门禁脚本对 PG 语法的误报 —— 已独立复现

`scripts/check-db-naming.js` 用 PG 语料跑会误报两个表名。用脚本内的同一套正则复现：

输入（PG 语法）：
```sql
INSERT INTO iam_users (id, name) VALUES (1, 'a')
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

SELECT COUNT(*) FROM pg_catalog.pg_indexes WHERE schemaname = current_schema();
```
输出：
```
误报表名: ["pg_catalog","SET"]
```

| 误报 | 触发正则 | 根因 | 修法 |
|---|---|---|---|
| `SET` | `/(?<![`\w])UPDATE\s+([a-z_][a-z0-9_]*)/gi` | `DO UPDATE SET name` 中 `UPDATE SET` 被当作 `UPDATE <表名>`。现有 `isOnUpdateClause` 只挡 `ON UPDATE` / `ON DUPLICATE KEY UPDATE`，挡不住 PG 的 `DO UPDATE SET` | 在 `isOnUpdateClause`（或新增判断）中识别 `DO UPDATE SET`；或对 `UPDATE` 后紧跟 `SET` 关键字的匹配跳过 |
| `pg_catalog` | `/(?<![`\w])(?:FROM|JOIN)\s+([a-z_][a-z0-9_]*)/gi` | PG 系统目录 `pg_catalog` 不在 `SKIP_IN_SOURCE` 白名单 | `SKIP_IN_SOURCE` 增加 `pg_catalog`、`current_schema`、`pg_indexes`、`pg_tables`、`pg_class`、`pg_namespace` 等 |

**结论**：S0（先修门禁再写 PG 迁移）是**硬前提**，不是优化项。未修则首个 PG 迁移提交即被 `node scripts/check-db-naming.js` 拦截。

### 既有瑕疵（S0 顺手修）

`scripts/check-db-naming.js` 第 7 行：
```js
const VALID_PREFIXES = ['iam_', 'audit_', 'token_', 'ntf_', 'cfg_', 'file_', 'prue_', ,];
```
末尾 `'prue_', ,` 是 **JS 稀疏数组空洞**（`undefined` 元素）。`isValidTableName` 中
`VALID_PREFIXES.some(p => name === p || name.startsWith(p))` 遇到 `p === undefined` 时，
`name.startsWith(undefined)` 会把 undefined 转成字符串 `"undefined"` 比较——**不崩**，
但 `name === p` 分支永不匹配 `undefined`。属既有瑕疵，改为无空洞写法即可。

## 5. 占位符行为 —— sqlx 0.8.6 运行时实测（决定性）

> 探针：`.target/placeholder-probe/`（独立 crate，不属工作区，已在 `.gitignore` 的 `/.target/` 覆盖内）
> 目标容器：`coder-test-pg`（`postgres:16`，`postgres://postgres:coder@127.0.0.1:5434/coder_test`）

| # | 验证项 | 实测输出 | 结论 |
|---|---|---|---|
| 1 | PG + `$1` 占位符 | `OK v=hello` | ✅ 工作 |
| 2 | PG + `?` 占位符 | `FAIL syntax error at or near "::"` | ❌ **确认失效** |
| 3 | `QueryBuilder::push_bind` | SQL 产出 `SELECT $1::text AS v`，执行 `OK` | ✅ **自动方言化，范式依据成立** |
| 4 | 手写重写 `?`→`$1` + `.bind()` | `OK v=hello` | ✅ **方言感知层技术上可行** |
| 5 | 多重写 `?`→`$1,$2` + 两次 `.bind()` | `OK a=x b=y` | ✅ **顺序契约可实现** |

### 对架构设计的三条硬结论

1. **Q1 裁决（方言感知 SQL 构造层）技术上成立**，且 `QueryBuilder::push_bind` 是同一机制的现成实现——
   辅助层应与它同构（构造期产出「改写后 SQL + 参数序列」），而非两步式。
2. **重写层必须处理单引号字符串**：探针的 `rewrite_placeholders()` 做了 `in_str` 状态机，
   保证 `'a?b'` 这类字面量内的 `?` 不被改写。实施时需保留该逻辑并加单测。
3. **`?`→`$N` 的编号必须与 `.bind()` 调用顺序严格一致**（探针 #5 证实编号与绑定顺序需对应）。
   若 SQL 改写与参数绑定分离在两个调用点，PG 上会出现 `$N` 错位而 MySQL 上不暴露——这是最危险的静默错误。

## 6. 类型映射与表达式 —— sqlx 运行时实测（推翻 PRD 的三项）

> 探针：`.target/placeholder-probe/src/bin/probe2.rs`。目标：`coder-test-pg`（5434）。

### 7.1 时间类型：PRD 说的 `TIMESTAMP` 是**错的**，必须 `TIMESTAMPTZ`

| # | 验证 | 实测输出 | 结论 |
|---|---|---|---|
| A1 | `TIMESTAMP` → `Option<DateTime<Utc>>` | ❌ `mismatched types; Rust type Option<DateTime<Utc>> (as SQL type TIMESTAMPTZ) is not compatible with SQL type TIMESTAMP` | **PRD 的 `DATETIME`→`TIMESTAMP` 映射会编译/解码失败** |
| A2 | `TIMESTAMPTZ` → `Option<DateTime<Utc>>` | ✅ `Some(2026-11-02T11:50:34.009817Z)` | **正解** |
| A3 | `TIMESTAMPTZ` → `Option<NaiveDateTime>` | ❌ 不兼容 | 不可混用 |
| A4 | `SET TIME ZONE 'Asia/Shanghai'` 后读 TIMESTAMPTZ | ✅ 返回同一绝对时刻 `2026-11-02T11:50:34.009817Z` | **TIMESTAMPTZ 免疫会话时区偏移** |

**结论**：
- PG 迁移中 `DATETIME` / `DATETIME(3)` 应映射为 **`TIMESTAMPTZ` / `TIMESTAMPTZ(3)`**（非 `TIMESTAMP`），与 Rust 侧 `DateTime<Utc>` 对齐。
- 选 `TIMESTAMPTZ` **同时解决**了会话时区问题（A4）——无需在 6 个服务里补 `SET TIME ZONE 'UTC'`，消除了一整类漂移风险。
- 仅当 Rust 侧用 `NaiveDateTime` 时才对应 `TIMESTAMP`（A3 实测不兼容 `TIMESTAMPTZ`）。

### 7.2 整数类型：`u64` 在 PG **编译期双向不可用**

| # | 验证 | 实测 | 结论 |
|---|---|---|---|
| B1 | `u64` bind / `try_get` | ❌ `error[E0277]: the trait bound u64: sqlx::Type<Postgres> is not satisfied`；`note: but trait Type<MySql> is implemented for it`（`sqlx-mysql/src/types/uint.rs:60`） | **`bind` 与 `try_get` 双向都不可用**（`Decode<Postgres>` 亦未实现） |
| B2 | `BIGINT` → `i64` | ✅ `42` | 正解 |
| B3 | `bind(i64)` | ✅ `7` | 正解 |

**结论**：PG 无 `u64` 绑定，含 `u64` 的字段在 `--features postgres` 下**无法编译**。必须在改造中统一改 `i64`
（这是编译器强制的硬约束，不是风格选择）。

### 7.3 时间表达式：`NOW(3)` 与 `INTERVAL $1 HOUR` 在 PG 非法

| # | 验证 | 实测 | 结论 |
|---|---|---|---|
| C1 | `NOW(3)` | ❌ `function now(integer) does not exist` | **`ON DUPLICATE KEY ... NOW(3)` 这类写法必须改 `CURRENT_TIMESTAMP(3)`** |
| C2 | `NOW()` | ✅ OK | 可用 |
| C3 | `INTERVAL $1 HOUR` | ❌ `syntax error at or near "$1"` | **参数不能直接放进 INTERVAL 字面量** |
| C4 | `($1 * INTERVAL '1 hour')` | ✅ OK | 替代方案 1 |
| C5 | `make_interval(hours => $1)` | ✅ OK | 替代方案 2（推荐，语义清晰） |

**注意与 §3 #1 的区别**：`CURRENT_TIMESTAMP(3)` 在 PG **合法**（实测通过），
但 `NOW(3)` **不合法**（`now()` 无重载）。二者不可混为一谈。

### 7.4 多方言共存的编译期陷阱

同时启用 `mysql` + `postgres` feature 时，`sqlx::query("...")` 的 `DB` 类型参数
**被推断为 `MySql`**（sqlx 的默认 Database），编译报
`expected MySql, found Postgres`。必须显式标注：
```rust
sqlx::query::<sqlx::Postgres>("...")          // 而非 sqlx::query("...")
sqlx::query_as::<sqlx::Postgres, Row>("...")  // 两个泛型参数：DB, O
```
另：`#[derive(sqlx::FromRow)]` 需要 `sqlx` 的 **`macros`** feature。

这是服务层双方言改造时**每一处调用点都要面对**的问题——是 Q1 辅助层存在的根本理由：
把 `::<DB>` 的类型标注收敛到辅助层内部，业务代码不再需要写。

## 7. 数据库容器

| 容器 | 镜像 | 端口 | 凭据 | 用途 |
|---|---|---|---|---|
| `coder-test-pg` | `postgres:16` | `5434→5432` | `postgres` / `coder`，库 `coder_test` | **本次任务专用**（按 2026-10-30 基座方案约定，常开） |
| `aipm-pg-dev` | `postgres:16` | `5433→5432` | — | 其他项目占用，**勿动** |

`TEST_PG_URL='postgres://postgres:coder@127.0.0.1:5434/coder_test'`

> 注意：Windows + Docker Desktop 下 `localhost` 可能解析到 IPv6 `::1` 而转发不通，**请用 `127.0.0.1`**。

## 8. 待补实测结果

`iam_service` 的编译结果（依赖 `aws-lc-rs`，耗时较长，待回填）。
