# 服务层全量 PostgreSQL 支持 — 系统设计

> 日期：2026-11-02
> 上游 PRD：`docs/superpowers/specs/2026-11-02-postgresql-service-layer-support-prd.md`（**已裁决 Q1~Q8**）
> 验证环境：`docs/superpowers/specs/2026-11-02-postgresql-verification-env.md`（**实测权威，本文所有"实测"标注均指该文档 + 本设计阶段的补充探针**）
> 基座层：`docs/superpowers/specs/2026-10-30-postgresql-dialect-support-design.md`（已完成，不重做）
> 范围：7 个服务 + `iam-outbox-publisher`、4 个共享库、门禁脚本、CI、compose、文档
> 任务分解：`docs/superpowers/plans/2026-11-02-postgresql-service-layer-support.md`

---

## 0. 结论先行

| # | 结论 |
|---|---|
| **D-1** | **占位符问题不是"改写 SQL"，而是"SQL 构造期选择方言"**。**主理人独立探针在 sqlx 0.8.6 运行时实测（权威）**：`PG + $1` ✅ / `PG + ?` ❌ `syntax error at or near "::"` / `QueryBuilder` 自动产出 `SELECT $1::text AS v` / 自研 `rewrite()` 等价可执行。基线层（`sqlx-core`）`format_placeholder` 的**唯一调用方是 `QueryBuilder::push_bind`**，MySQL/SQLite 用默认实现输出 `?`，**仅 PG 覆写**为 `$N`。二者**互斥**，无法有一份字面量同时正确。→ 库层提供**构造期方言化**（Q1-A）+ **`DbPool` 枚举分派**（Q6-C），业务 SQL 保持**单份 `?` 字面量源码**。见 §1.3.0 实测原文。 |
| **D-1b** | **顺序错位必须用类型约束拦在构造期，不能等执行期**。主理人实测 [5] 证实多占位符编号必须与 `.bind()` 顺序严格对应；错位在 PG 上报 `$N` 越界、**在 MySQL 上完全不暴露** —— 这是本任务最危险的缺陷类型。→ `render` **不返回裸 `String`**，而返回 `#[must_use] RewrittenQuery { sql, expected_binds }`；`sql()` 只能在校验后的实例上取得（消耗式 `expect_binds`）。**MySQL 侧同样计数校验**，从而让两端行为一致。见 §1.3.3。 |
| **D-2** | **辅助层的落地形态必须是"宏"而非"trait 方法"**。本设计已用真实编译验证：sqlx 的 `QueryAs`/`Query` 是 **DB 泛型类型**，`DbPool` 是 enum，无法在 trait 方法里返回一个"两种 DB 通用的 query 对象"；且 `sqlx::query_as::<DB, T>` 要求 `T: for<'r> FromRow<'r, DB::Row>`，在 `&str` 等 bind 参数上会因 `Encode<'_, DB>` 无法泛化而**编译失败**（已实测报错 `the trait bound str: Sized is not implemented`）。→ 采用 **`macro_rules!` 生成 `match` 分派**：一份宏调用 → 展开为各后端各自的强类型 query。**这仍然满足 Q1-bis-L2「库层 trait 上的 query/query_as」的实质（调用点不暴露方言），但实现载体是宏**。 |
| **D-3** | **`DateTime<Utc>` 与 PG `TIMESTAMP` 不兼容 → 时间列必须 `TIMESTAMPTZ`**（主理人实测 A1~A4，权威原文见 verification-env.md §7.1）：<br>`A1` `TIMESTAMP   -> Option<DateTime<Utc>>` = ❌ `mismatched types; Rust type Option<DateTime<Utc>> (as SQL type TIMESTAMPTZ) is not compatible with SQL type TIMESTAMP`<br>`A2` `TIMESTAMPTZ -> Option<DateTime<Utc>>` = ✅ `Some(2026-11-02T11:50:34.009817Z)`<br>`A3` `TIMESTAMPTZ -> Option<NaiveDateTime>` = ❌ 不兼容<br>`A4` `SET TIME ZONE 'Asia/Shanghai'` 后读 TIMESTAMPTZ = ✅ **同一绝对时刻** `2026-11-02T11:50:34.009817Z`<br>根因：sqlx 的 `type_compatible` 是**严格相等**（`sqlx-postgres/src/type_info.rs:1057`）。仓库 **75 个 `DateTime<Utc>`、0 个 `NaiveDateTime`**。<br>→ PG 迁移里 `DATETIME`/`DATETIME(3)` → **`TIMESTAMPTZ`/`TIMESTAMPTZ(3)`**。<br>**A4 的额外价值**：`TIMESTAMPTZ` 存绝对时刻 → **天然免疫会话时区漂移**，等于把 Q2-bis 那一整类风险消掉（`SET TIME ZONE 'UTC'` 降级为兜底而非必需）。这**推翻** PRD §5.0 与 verification-env §3 的「DATETIME → TIMESTAMP」表述（后者已被其 §7.1 自我修正）。 |
| **D-4** | **`u64` 在 PG 无对应类型（新发现的硬阻断）**。`sqlx-postgres` **完全没有** `u8/u16/u32/u64/usize` 的 `Type`/`Decode` 实现（已核对源码类型清单）。仓库有 **29 个 `pub ...: u64` 字段**（`tenant_id` 等）和若干 `.bind(tenant_id as u64)`。→ **必须把受影响字段/绑定改为 `i64`**（PG `BIGINT` 为有符号，实测 `bigint out of range`）。MySQL 侧 `BIGINT UNSIGNED` 列用 `i64` 解码：`sqlx-mysql::int_compatible` **拒绝** `UNSIGNED` 标志 → 因此需**同时**把 MySQL 侧 DDL 的 `BIGINT UNSIGNED` 改为 `BIGINT`（V002 迁移在开发期可重建，见 §3.4 说明）。 |
| **D-5** | **`bool` 与 PG `BOOLEAN` 兼容，但与 MySQL `TINYINT(1)` 解码差异仍在**。实测 `BOOLEAN -> bool` ✅ / `BOOLEAN -> i8` ❌；MySQL `TINYINT(1) -> bool` ✅。现有代码有 2 处 `.bind(x as i8)`（`notification-service/src/db.rs:156-157`）→ PG 侧必须去掉 `as i8`。 |
| **D-6** | **`NOW(3)` 在 PG 不存在**（实测 `function now(integer) does not exist`）。`iam-service` 与 `iam-outbox-publisher` 的 `outbox_archive.rs` 共 **10 处** 用 `NOW(3)`。→ 统一改为 `now()`（列类型 `TIMESTAMPTZ(3)` 已保证精度截断，实测 `date_trunc('milliseconds', now())` 与 `now()::timestamp(3)` 均正确）。 |
| **D-7** | **`INTERVAL ? HOUR` 在 PG 非法**（实测 `syntax error at or near "$1"`）。→ 改为 `($N * INTERVAL '1 hour')` 或 `make_interval(hours => $N)`（两者实测通过）。 |
| **D-8** | **`CURRENT_TIMESTAMP(3)` / `JSON` 列型无需改**（Q7-A + 实测 #1/#2）。但 `CURRENT_TIMESTAMP` 返回 `timestamptz`，写入 `TIMESTAMP`（naive）列会按会话 TZ 偏移（实测 +08h 静默偏移）。→ 配合 D-3 统一用 `TIMESTAMPTZ` 后，**该风险自然消失**（`TIMESTAMPTZ` 存储绝对时刻，已实测跨 `Asia/Shanghai` 会话往返 `delta_ms = 0`）。`SET TIME ZONE 'UTC'`（Q2-bis）仍需落地，作为 `now()` 返回值展示与 `date_trunc` 等会话敏感计算的兜底。 |
| **D-9** | **默认 features 零变化可严格保证**：`libs/common-db` 的 `default = ["sqlite"]` 不动；`postgres` 只出现在各服务自己的 `postgres` feature 里，而服务 `default` 不含它。验收 A-4/A-7 用两条独立命令守住。 |
| **D-10** | **`check-db-naming.js` 必须最先修**（S0）。已复现两处误报：`DO UPDATE SET` 的 `SET`、`pg_catalog`；另有 `VALID_PREFIXES` 稀疏数组空洞（实测 `length = 8`、`index 7 = undefined`，当前 `.some()` 跳过空洞所以不阻塞，但必须顺手修正）。 |
| **D-11** | **多方言共存时 `sqlx::query()` 的 DB 类型会被推断为 `MySql`**（主理人实测：`type mismatch resolving <&Pool<Postgres> as Executor>::Database == MySql`，`expected MySql, found Postgres`）。→ **每一处调用点都必须显式写 `::<sqlx::Postgres>`**，否则编译失败。**这是 Q1 辅助层最有力的存在理由**：它把 `::<DB>` 类型标注收敛到库层内部，业务代码一处都不必写。这不是"减少重复"的审美问题，是**编译器强制**。另：`#[derive(sqlx::FromRow)]` 需 `sqlx` 的 `macros` feature。 |

### 0.2 本设计所依据的实测证据

> 全部实测原文、探针位置、容器凭据集中于 **`docs/superpowers/specs/2026-11-02-postgresql-verification-env.md`**（主理人独立执行，权威）。
> 本设计**不重复**其输出，只标注"哪条结论依赖哪项实测"，便于评审逐条回溯。

| 设计结论 | 依赖的实测编号（见 verification-env.md） | 该实测回答的问题 |
|---|---|---|
| D-1 / §1.3.0 占位符互斥 + `QueryBuilder` 范式成立 | **§5 #1~#5** | `?` 在 PG 失效、`$1` 可用、`QueryBuilder` 自动方言化、手写重写可行、多重写顺序可对齐 |
| D-11 / §1.2 显式 `::<DB>` 标注是编译器强制 | **§7.4** | 多 feature 下 `sqlx::query()` 推断为 `MySql`，必须显式标注 |
| D-3 / §4.4 时间列必须 `TIMESTAMPTZ` | **§7.1 A1~A4** | `TIMESTAMP`→`DateTime<Utc>` 不兼容；`TIMESTAMPTZ` 兼容；且**跨会话时区返回同一绝对时刻**（A4） |
| D-4 / §3 归属 #19 `u64`→`i64` 是编译器强制 | **§7.2 B1~B3** | `u64: Type<Postgres>` 未实现（`bind` 与 `try_get` **双向**皆不可用） |
| D-6 / §3 归属 #21 `NOW(3)` 非法 | **§7.3 C1~C2** | `now(integer) does not exist`；`NOW()` 可用 |
| D-7 / §3 归属 #22 `INTERVAL $1 HOUR` 非法 | **§7.3 C3~C5** | PG 要求 `($1 * INTERVAL '1 hour')` 或 `make_interval(hours => $1)` |
| D-8 / §4.4 `CURRENT_TIMESTAMP(3)` 保持原样 | **§3 #1 + §7.3 末尾注** | PG 接受精度参数；**与 `NOW(3)` 是两个不同函数，不可混谈** |
| D-10 / S0 门禁必须先修 | **§4** | 误报根因 `["pg_catalog","SET"]` 与修法 |

> **两条容易被混淆的区分**（实测已澄清，实施时务必分开）：
> 1. **`CURRENT_TIMESTAMP(3)` 合法 vs `NOW(3)` 非法** —— 前者是 SQL 标准关键字、PG 接受精度参数；后者是函数、PG 无 `now(integer)` 重载。全仓 `CURRENT_TIMESTAMP(3)` 9 处**保持原样**，`NOW(3)` 10 处**必须改**。
> 2. **`TIMESTAMP` vs `TIMESTAMPTZ`** —— 前者 naive、后者带时区。Rust `DateTime<Utc>` 只认后者；`NaiveDateTime` 只认前者（互不兼容，双向实测）。本仓 75 个 `DateTime<Utc>`、0 个 `NaiveDateTime` → 统一 `TIMESTAMPTZ`。

---

## 1. 方案选型与理由（Q1 / Q1-bis / Q6）

### 1.1 问题本质：占位符互斥

| 后端 | `SELECT ?` | `SELECT $1` |
|---|---|---|
| MySQL 8.0 | ✅ 返回 1 | ❌ `ERROR 1054 Unknown column '$1' in 'field list'` |
| PG 16.15 | ❌ `syntax error at or near "?"`（`?` 是 jsonb 存在性操作符） | ✅ 返回 42 |

sqlx 侧的机制（源码核对）：
- `Arguments::format_placeholder` 的默认实现输出 `?`（`sqlx-core/src/arguments.rs:27`）。
- **仅** `PgArguments` 覆写为 `${count}`（`sqlx-postgres/src/arguments.rs:156`）。
- `format_placeholder` 的**唯一调用方**是 `QueryBuilder::push_bind`（`sqlx-core/src/query_builder.rs:161`）。
- `sqlx::query(sql)` 走的是 `prepare(sql)`，SQL 文本**原样**发给服务端 → 不做任何重写。

**结论**：想在两种后端复用同一份 SQL 源码，方言化必须发生在**我方代码的 SQL 构造期**，且必须**同时**产出「已方言化的 SQL」与「与之严格对齐的参数序列」。这正是 `QueryBuilder::push_bind` 的语义，也是本仓库已有 44 处 `QueryBuilder` 用法天然跨方言的原因（PRD 已验证：其初始串中零 `?`）。

### 1.2 被否决的备选方案

| 方案 | 否决理由（含实测） |
|---|---|
| **B. 源码逐条改 `$N` + MySQL 另存一份** | MySQL 不认 `$N`（实测 `Unknown column '$1'`）→ 必须两套 SQL ≈ 600+ 份，与仓库「单一真相源」冲突；漂移风险极高。 |
| **C. 编译期 proc-macro** | 仓库零编译期 SQL 宏先例；仍需"一份源码→两份产物"，维护问题未解；新增 proc-macro crate 与编译期成本。 |
| **B'. sqlx `Any` 驱动做统一** | `AnyArguments::convert_to` 只支持 `Option<i8..i64/f32/f64/String/Vec<u8>>` 等**有限类型集**（`sqlx-core/src/any/arguments.rs`），**不支持 `chrono`/`serde_json`**；仓库大量使用 `DateTime<Utc>`(75 处) 与 `serde_json::Value`。另 `Any` 行类型 `AnyRow` 与 `#[derive(FromRow)]` 的泛型 impl 不匹配。→ 不可行。 |
| **Q6-B 泛型 `DB: Database` 函数** | **已实测编译失败**：`sqlx::query_as::<DB, T>(...).bind(id)` 在 `id: &str` 上要求 `&str: Encode<'_, DB>`，而 `Encode` 是对具体 DB 实现的（`impl Encode<'_, MySql> for &'_ str` 等），泛型 `DB` 下无法成立 → `the trait bound str: Sized is not implemented`。行类型 `T: for<'r> FromRow<'r, DB::Row>` 虽可写，但 bind 参数泛化不成立。→ 这条路走不通，反证了 Q6-C（枚举 + 重写层）是唯一可行解。 |
| **L1 独立函数 `q(dialect, sql)`** | 会退化为"两步式"：先算 SQL，再在另一处 `.bind()`。**顺序契约靠人工保证**，正是 PRD 明确要求避免的形态。 |
| **L3 全部改 `QueryBuilder`** | 226 个调用点需整体重写为 `push_bind` 链；跨行 SQL 改成字符串拼接会显著降低可读性，且现有 `QueryBuilder` 用法是 `MySqlPool` 专属类型，仍需先做类型泛型化。保留给"动态拼 SQL"场景，不作主路径。 |

### 1.3 采纳方案：**构造期方言化宏**（Q1-A + Q1-bis-L2 + Q6-C 的落地形态）

#### 1.3.0 实测依据（主理人独立探针，2026-11-02，权威）

sqlx 0.8.6 运行时实测原文（探针独立 crate `.target/placeholder-probe/`，不进仓库）：

```
[1] PG + $1        => OK   v=hello
[2] PG + ?         => FAIL error returned from database: syntax error at or near "::"
[3] QueryBuilder   => SQL = SELECT $1::text AS v
[3] QueryBuilder   => OK   v=hello
[4] rewrite()      => SELECT ?::text AS v  →  SELECT $1::text AS v
[4] rewrite+exec   => OK   v=hello
[5] rewrite multi  => SELECT $1::text AS a, $2::text AS b
[5] rewrite multi  => OK   a=x b=y
```

这三条实测确立了本设计的骨架：

| 结论 | 设计含义 |
|---|---|
| **[3] `QueryBuilder` 在 PG 下自动产出 `$1`** | `QueryBuilder::push_bind` 内部调 `format_placeholder` → **"构造期一次性产出 SQL + 参数"是可行且已被 sqlx 自身验证的范式**。辅助层必须与之同构，不能两步式。 |
| **[4][5] 自研 `rewrite()` 等价于 sqlx 的方言化** | 说明我方 `render` 的正确性基线可以与 `QueryBuilder` 对拍（**单测策略**：同一 SQL 分别经 `render` 与 `QueryBuilder` 处理，断言输出一致）。 |
| **[5] 多占位符编号必须与 bind 顺序严格对应** | 错位是**静默缺陷**：PG 上 `$N` 越界报错，**MySQL 上完全不暴露** → 必须在**构造期**用类型/断言拦住，不能等执行期。 |

#### 1.3.1 与 `QueryBuilder::push_bind` 同构：为什么是宏，而不是 builder

> PRD 要求「与 `QueryBuilder::push_bind` 同构（构造期一次性产出改写后 SQL + 参数），**不要**两步式」。

**"同构"的实质是：SQL 方言化与 bind 追加发生在同一个构造动作里，不产生可分离的中间态。**
本设计用 **`macro_rules!` 的 `$( ... )*` 重复**实现这一点：

```text
db_fetch_all!(pool, Row, "SELECT ... WHERE a = ? AND b = ?", a, b)
                 │                                        └────┬────┘
                 │                                     同一组 token
                 ▼                                             ▼
   match pool {                                  每个分支各自展开一遍
     Mysql(p)  => { let rq = render(Mysql);  q.bind(a).bind(b) }   ← 顺序 = 源码顺序
     Postgres(p)=> { let rq = render(Postgres); q.bind(a).bind(b) } ← 同上
   }
```

- **SQL 文本只有一份源码**（宏实参），`render` 在**构造期一次**把它转成目标方言。
- **bind 序列只有一份源码**（宏实参尾部），在**每个分支内按同一 token 顺序**展开。
- `?` 的第 k 个出现 ⇔ 第 k 个 `.bind()`：由 `render` 的**扫描顺序**（左→右）与宏的**展开顺序**（左→右）**双向锁定**。
- **不存在"先改 SQL、再另行 bind"的中间态**——两件事在同一次宏展开中完成。

**为什么不做成 `q(dialect).push(...).push_bind(v)` 这种 builder？**
不是偏好问题，是类型系统不允许：

| 尝试 | 结果 |
|---|---|
| `struct DbQuery<'a> { sql: String, binds: Vec<???> }` | bind 值类型异构（`&str`/`i64`/`bool`/`DateTime<Utc>`/`serde_json::Value`…）。要么装箱 `Box<dyn Encode<DB>>`（`Encode` 带 `DB` 参数 + `ArgumentBuffer<DB>`，非对象安全），要么自行实现 `DialectValue` trait（需为 3 个后端 × ~12 类型手写 `Encode`/`Type`，且 `Type<DB>::type_info()` 是 DB-specific，无法泛化）。 |
| 泛型函数 `fn fetch<DB: Database>(pool: &Pool<DB>, ..)` | **已实测编译失败**：`sqlx::query_as::<DB, T>(..).bind(id)` 要求 `&str: Encode<'_, DB>`，而 `Encode` 只对**具体 DB** 实现（`impl Encode<'_, MySql> for &'_ str` 等），泛型 `DB` 下不成立 → `the trait bound str: Sized is not implemented`。 |
| trait 方法 `trait Db { fn query_as<T>(&self, ..) }` | `QueryAs<'q, DB, T, DB::Arguments<'q>>` 是 **DB 泛型类型**，一个 trait 方法无法返回"两种 DB 通用"的 query 对象；参数化又会退化成上面第 2 条。 |

→ **宏分派是"同构"在 sqlx 类型体系下唯一可编译的载体**；`QueryBuilder` 的块粘贴宏（`db_qb_*!`）则直接复用了 sqlx 自己的同构实现。

**编译期验证结论（本设计阶段实测，`--features "mysql,postgres"`）**：
- `db_fetch_all!` 形态（`match` + 每分支 `.bind()` + `.await`）**编译通过**。
- `QueryBuilder` 块粘贴形态（`db_qb_fetch_all!(pool, T, |qb| { ... })`，同一个 `$body` token 块在两分支各展开一次，分别绑定 `QueryBuilder<MySql>` / `QueryBuilder<Postgres>`）**编译通过**。
- `Cow<'_, str>` 跨 `await` 的借用形态**编译通过**（PG 分支 `Owned`、MySQL/SQLite 分支 `Borrowed`，零分配）。

#### 1.3.2 重写层：必须带状态机，且必须是独立可测纯函数

主理人探针给出的是**最小实现**（只处理单引号），本设计在其基础上补齐为**完备状态机**。差距与理由：

| 场景 | 最小实现 | 本设计要求 | 理由 |
|---|---|---|---|
| 双引号标识符 `"a?b"` | ❌ 会把 `?` 当占位符 | ✅ 识别并跳过 | PG / SQLite / MSSQL 都用双引号转义标识符；列名含 `?` 虽罕见但语法合法 |
| `` `a?b` `` 反引号 | ❌ | ✅ 识别并跳过 | MySQL 标识符引用 |
| `-- ?` 行注释 | ❌ | ✅ 识别并跳过 | 迁移/调试 SQL 常带注释 |
| `/* ? */` 块注释 | ❌ | ✅ 识别并跳过 | 同上 |
| `$$...$$` dollar-quoted | ❌ | ✅ 识别并跳过 | Q2-A 已规避触发器，但**字符串/注释里仍可能出现**（如注释中写 PG 示例） |
| 转义单引号 `''` | ⚠️ 朴素 toggle 恰好不出错（两个 toggle 相互抵消，且 `''` 之间不可能夹 `?`），但**语义脆弱、不可测** | ✅ **显式 lookahead**：遇 `'` 且下一字符也是 `'` → 消耗两个、**保持** `InSingle` | 显式处理可单测、可在 `'it''s'` 这类语料上稳定通过 |
| 反斜杠转义 `\'`（MySQL 默认开启 `backslash_escapes`） | ❌ 会把 `\'` 中的 `'` 当闭合 | ✅ MySQL 方言下处理 `\` 转义；PG 侧 `standard_conforming_strings=on`（默认）下 `\` 是普通字符，**不**处理 | 两方言语义不同，按 `DialectKind` 分流 |
| 未闭合字符串 | ❌ 不报错，静默吞掉 | ✅ 扫描结束若仍在 `InSingle`/`InDouble`/`InBlockComment` → `Err(UnterminatedLiteral)` | 静默是最大风险源 |

**状态机定义**（`InNormal` / `InSingle` / `InDouble` / `InBacktick` / `InLineComment` / `InBlockComment` / `InDollarQuote(tag)`）：

| 输入 | 起始状态 | 迁移 | 输出 |
|---|---|---|---|
| `'` | Normal | → InSingle | `'` |
| `''` | InSingle | 停留 InSingle（转义） | `''` |
| `'` | InSingle | → Normal | `'` |
| `"` | Normal | → InDouble | `"` |
| `""` | InDouble | 停留（转义） | `""` |
| `` ` `` | Normal | → InBacktick | `` ` `` |
| `--` | Normal | → InLineComment | `--` |
| `\n` | InLineComment | → Normal | `\n` |
| `/*` | Normal | → InBlockComment | `/*` |
| `*/` | InBlockComment | → Normal | `*/` |
| `$$` 或 `$tag$` | Normal | → InDollarQuote(tag) | 原样 |
| 同 tag 的 `$tag$` | InDollarQuote(tag) | → Normal | 原样 |
| `?` | **Normal** | — | `$n`（PG）；原样（MySQL/SQLite） |
| `?` | 其他任何状态 | — | 原样 + **记录"字面量内 `?`"**（见下） |

**"字面量内 `?`"的处置（关键取舍）**：
- 语义上它**不是**占位符 → 若只是跳过，PG 侧行为正确、MySQL 侧行为也正确。
- 但它是**极强的作者意图信号**（大概率本意是占位符，写错了引号）→ 若静默跳过，会出现"参数个数对不上"的诡异报错。
- **裁决：报错**。`render` 返回 `Err(SqlRenderError::PlaceholderInLiteral { raw_sql, dialect, offset })`。
- 全仓实测：SQL 字面量内 `?` **0 个**（PRD 与本次两套独立方法复核一致）→ 报错**不会**误伤现有代码，是纯防回归护栏。
- 单测语料（见 §6.5 测试表）必须覆盖全部状态迁移。

**单测策略：与 `QueryBuilder` 对拍**（利用实测 [3]）：
```rust
// 对同一批 SQL，render(pg) 的输出必须与 QueryBuilder::<Postgres> 的产物一致
for sql in corpus {
    let ours = render(sql, DialectKind::Postgres).unwrap();
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("");
    // 用 push 逐段 + push_bind 模拟 → 取 qb.sql()
    assert_eq!(ours, qb_sql);
}
```
这把"我方重写是否正确"锚定在 sqlx 自身行为上，而不是靠人肉审阅。

#### 1.3.3 顺序契约的**结构性**保证：`RewrittenQuery` + 构造期断言

> 主理人明确指出：「编号与绑定顺序的静默错位是最大风险…请在设计中把它写成**类型层面的约束**」。

因此 `render` **不返回裸 `String`**，而是返回携带契约的结构体：

```rust
/// 已方言化的 SQL + 其"应绑参数个数"契约。
/// `expected_binds` 由扫描器在改写时同步计数得出，与 `sql` 一一对应。
#[must_use = "RewrittenQuery 携带 bind 个数契约，丢弃它等于放弃校验"]
pub struct RewrittenQuery<'a> {
    sql: std::borrow::Cow<'a, str>,
    expected_binds: usize,
}

impl<'a> RewrittenQuery<'a> {
    pub fn sql(&self) -> &str;
    pub fn expected_binds(&self) -> usize;

    /// 构造期校验：实际提供的 bind 个数必须等于 expected_binds。
    /// 不等 → 立即 Err（在**构造期**，不是执行期）。
    pub fn expect_binds(self, actual: usize) -> Result<Self, SqlRenderError>;
}
```

宏内部强制走这条路径（**两处**断言，双保险）：

```rust
let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Postgres)
    .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
// (1) 编译期即可知 bind 个数（宏能数字面重复次数）→ 构造期断言
let __rq = __rq.expect_binds(0usize $(+ { let _ = &$bind; 1usize })*)
    .map_err(|e| sqlx::Error::Configuration(Box::new(e)))?;
let mut __q = sqlx::query_as::<sqlx::Postgres, $ty>(__rq.sql());
$( __q = __q.bind($bind); )*
```

为什么这样就"结构性"了：
1. **同一个值**（`RewrittenQuery`）同时携带 `sql` 与 `expected_binds` → 二者**不可能分家**。
2. `expect_binds` 是**消耗性**的（`self`），未校验的 `RewrittenQuery` 拿不到 `&str` → 编译器强制你先校验。
3. `#[must_use]` 防止丢弃。
4. **失败发生在构造期**（首次请求即炸），而不是等到某个 `$N` 越界的偶发路径。
5. **覆盖 MySQL 静默缺陷**：即使 MySQL 侧不需要改写，`render` 也会统计 `?` 个数并做同样的 `expect_binds` 校验 → **两端行为一致，MySQL 不再"默默接受错位"**。这是本设计对"最危险缺陷类型"的直接封堵。

#### 1.3.4 错误信息回显原始 SQL

`SqlRenderError` 携带 `{ raw_sql: String, dialect: DialectKind, offset: usize, reason }`，`Display` 输出形如：

```
sql render failed [postgres]: placeholder '?' found inside string literal at byte 42
  raw: SELECT * FROM t WHERE name LIKE '?%' AND id = ?
```
```
sql render failed [postgres]: bind count mismatch: sql expects 3, caller supplied 2
  raw: SELECT * FROM t WHERE a = ? AND b = ? AND c = ?
```

满足 PRD「错误信息须回显**原始 SQL** 与方言」。

### 1.4 ⚠️ 宏的 `#[cfg(feature = ...)]` 求值主体错配（S1 阻塞项，已实测复现并解决）

> 主理人发现的缺陷，**已由主理人与我双方独立实测复现**。这是 S1 的阻塞项：按原设计实施会**直接编译失败**。

#### 1.4.1 缺陷机制

`macro_rules!` 展开时，宏体内的 `#[cfg(feature = "...")]` 按**展开发生的 crate（调用方）**求值，**不是**定义宏的 crate。rustc 自己会给出这条提示：

```
note: using a cfg inside a macro will use the cfgs from the destination crate
      and not the ones from the defining crate
```

**复现（独立探针，模拟"库定义宏 + 调用方无 `[features]` 段"）**：

```rust
// 库 pgdb：cfg 门控的 DbPool 变体 + 宏
pub enum DbPool {
    #[cfg(feature = "mysql")]    Mysql(Pool<sqlx::MySql>),
    #[cfg(feature = "postgres")] Postgres(Pool<sqlx::Postgres>),
}
#[macro_export]
macro_rules! db_fetch_all { /* match $pool { #[cfg(feature="mysql")] ... } */ }
```
```rust
// 调用方 app：无 [features] 段（与本仓 7 个服务 crate 的现状一致）
[dependencies]
pgdb = { path = "../pgdb", features = ["mysql"] }
```

编译结果（**原设计在此处即失败**）：
```
error[E0004]: non-exhaustive patterns: type `&DbPool` is non-empty
note: `DbPool` defined here
warning: unexpected `cfg` condition value: `postgres`
  = note: using a cfg inside a macro will use the cfgs from the destination crate
          and not the ones from the defining crate
```

根因：调用方 crate **没有** `mysql`/`postgres` feature（服务只通过依赖声明启用库 feature，**不会**在调用方创建同名 feature）→ 宏内**两个分支同时被剔除** → `match` 零臂 → E0004。

> 另一个变体的报错更具迷惑性：宏体整体被剔除后，展开成 `()`，报错是
> ``error[E0277]: `()` cannot be formatted with the default formatter`` —— 与"数据库方言"毫无关联，极难定位。

#### 1.4.2 四方案实测矩阵（双方独立实测）

| 方案 | 做法 | 实测结果 |
|---|---|---|
| **1. 宏内写 `#[cfg(feature=...)]`，调用方无同名 feature** | 分支可见性由**调用方** feature 决定 | ❌ 调用方无同名 feature → 两分支同时剔除 → `E0004` / 展开成 `()` |
| **2. cfg 下沉到库内函数，宏只分派** | 库内 `#[cfg(feature="postgres")] pub fn q_postgres(...)` | ❌ `error[E0425]: cannot find function 'q_postgres' ... found an item that was configured out` —— 库 feature 未开时该函数**不存在**，宏无条件引用它即报错 |
| **3. 宏内不写任何 cfg，库侧 feature 常开** | 库侧依赖硬写 `features = ["mysql","postgres",...]` | ⚠️ 编译通过，但**默认构建被迫带上 `sqlx-postgres` 驱动**（实测：干净构建多花 **189 秒**，且 `cargo tree -i sqlx-postgres` 在默认构建里出现）→ **突破「默认构建产物零变化」纪律** |
| **3′. 宏内保留 cfg + 调用方定义同名 feature 并转译 + 依赖声明不硬写方言 feature** ✅ | 调用方 feature 同时转译库 feature 与 `sqlx/*` feature | ✅ **默认构建不带 postgres；带 `--features postgres` 时才带上**；两套 cfg 由 Cargo 的 feature 转译**天然同步** |

**根本矛盾**：`cfg` 的求值主体**永远是「展开发生的 crate」**，无论 cfg 写在宏里还是库函数里。
→ 库 crate 与调用方 crate 的 feature 集合必须一致，否则「库有变体、宏没分支」或反之。

#### 1.4.3 采纳方案（方案 3′）

**核心思想：保留「宏内 cfg」，但让调用方 feature 与库 feature 通过 Cargo 的 feature 转译机制「锁步」。**

```toml
# 库 libs/common-db/Cargo.toml —— feature 定义不变，default 仍为 ["sqlite"]
[features]
default  = ["sqlite"]
mysql    = ["sqlx/mysql"]
postgres = ["sqlx/postgres"]
sqlite   = ["sqlx/sqlite"]
```

```toml
# 服务 services/<svc>/Cargo.toml —— 依赖声明**不硬写**方言 feature
[dependencies]
sqlx            = { workspace = true, features = ["chrono", "macros"] }   # ← 方言 feature 走下方转译
digua_common_db = { path = "../../libs/common-db", features = ["openapi"] }  # ← 同上

[features]
default  = ["mysql"]
mysql    = ["sqlx/mysql",    "digua_common_db/mysql",    "digua_common/mysql",    "digua_common_event/mysql"]
postgres = ["sqlx/postgres", "digua_common_db/postgres", "digua_common/postgres", "digua_common_event/postgres"]
sqlite   = ["sqlx/sqlite",   "digua_common_db/sqlite",   "digua_common/sqlite"]
```

```rust
// 宏内**保留** cfg —— 它按调用方求值，而调用方 feature 已转译库 feature → 两套 cfg 恒等
match &$pool {
    #[cfg(feature = "mysql")]    $crate::DbPool::Mysql(p)    => { /* sqlx::MySql    */ }
    #[cfg(feature = "postgres")] $crate::DbPool::Postgres(p) => { /* sqlx::Postgres */ }
    #[cfg(feature = "sqlite")]   $crate::DbPool::Sqlite(p)   => { /* sqlx::Sqlite   */ }
    #[allow(unreachable_patterns)]
    _ => Err(/* 受控错误，见 §1.4.9 */),
}
```

**为什么这样就同步了**：
- 调用方不开 `postgres` → 宏的 PG 分支被剔除 → **不引用** `DbPool::Postgres` / `sqlx::Postgres` → 库侧也不需要它们（`digua_common_db/postgres` 未被转译开启）→ ✅
- 调用方开 `postgres` → 宏的 PG 分支保留 → 同时 `digua_common_db/postgres` 与 `sqlx/postgres` 被转译开启 → `DbPool::Postgres` 与 `sqlx::Postgres` **都存在** → ✅

**关键点：服务依赖声明里不能硬写 `sqlx/postgres`**，否则默认构建会带上 PG 驱动。
方言 feature **必须**由服务自己的 feature 转译过去（`postgres = [... "sqlx/postgres" ...]`）。

> **为什么不用「运行期分派 + 类型擦除」**（理论上更优雅）：库内 dispatch 函数需要按运行期选定的方言泛化行类型 `T`
> （`T: for<'r> FromRow<'r, DB::Row>`）——`DB` 是运行期值，无法在编译期表达。这是 Rust 的 GAT / object-safety 限制，**不可行**（U-1 已裁决）。

#### 1.4.4 方案 3′ 的两项约束（**替代原「三项结构性后果」**）

| # | 约束 | 实测证据 | 处置 |
|---|---|---|---|
| **1** | **库侧 postgres feature 随调用方 feature 转译** → **默认构建不带 postgres** | `cargo tree -e features -i digua_common_db`（默认构建）→ `digua_common_db feature "postgres"` **计数 = 0**；`--features postgres` 时 **= 1**。<br>`cargo tree -i sqlx-postgres` 在默认构建中**不存在**，带 postgres 时存在。<br>→ **「默认构建产物零变化」的纪律得以保持** | 服务依赖声明**不硬写**方言 feature；方言 feature 全部经服务 `[features]` 转译。<br>**可执行校验**见 §1.4.7。 |
| **2** | **库自身不能用这个宏** | 库 `default = ["sqlite"]` 时，其自身测试里用宏 → `error[E0599]: no variant, associated function, or constant named 'Mysql' found for enum 'DbPool'` | 两种处置（择一）：<br>① 库内**用了宏的测试文件**顶部加 `#![cfg(all(feature = "mysql", feature = "postgres"))]`，并用 `cargo test -p digua_common_db --features "mysql,postgres"` 跑；<br>② 把库的宏用法测试放进**独立的集成测试 crate**，用 `--features` 显式驱动。 |

**已消除的旧「后果」**：
- ~~「后果 1：库侧 postgres 必须常开、默认构建被迫带 PG 驱动」~~ → **不成立**（方案 3′ 已解决；189 秒的额外构建成本不再存在）。
- ~~「后果 3：服务侧 postgres 变成名义开关」~~ → **不成立**：在方案 3′ 下，服务侧 `postgres` feature **真正控制**了
  ① 库的 `DbPool::Postgres` 变体是否存在、② 宏的 PG 分支是否可见、③ `sqlx::Postgres` 是否可用。**它是有效开关。**

#### 1.4.5 方案 3 与 3′ 的对比（决策留档）

| 维度 | 方案 3（宏内无 cfg + 库侧常开） | **方案 3′（宏内保留 cfg + 转译）** ✅ |
|---|---|---|
| 默认构建是否带 `sqlx-postgres` | ❌ **带**（实测多 189 秒） | ✅ **不带**（`cargo tree` 计数 = 0） |
| 「默认构建产物零变化」 | ❌ 突破 | ✅ **保持** |
| 宏内 cfg | 无 | **有**（按调用方求值，与库 feature 天然同步） |
| 服务侧 `postgres` feature | 名义开关 | ✅ **有效开关** |
| 维护负担 | 低 | 中（多一条「服务 feature ⇔ 库 feature 同名同义」的不变式，**由 §1.4.7 门禁自动化保障**） |

> **方案 3 何时仍然更优**：若将来要求「一个二进制同时内置所有方言、连 feature 都不需要」——
> 但那与「默认构建产物零变化」的纪律直接冲突。**本设计选方案 3′。**

#### 1.4.6 顺带发现：既有 CI `test-services` job **今天就是坏的（两层原因）**

**第一层：包名用 dash**（workspace 包名是**下划线**）：

```bash
$ cargo test --package audit-service --features mysql --no-run
error: cannot specify features for packages outside of workspace
help: a workspace member with a similar name exists: `audit_service`

$ cargo test --package config-service --features mysql --no-run
error: cannot specify features for packages outside of workspace
help: a workspace member with a similar name exists: `config_service`
```

**第二层：包名改对后，服务 crate 仍没有该 feature**：

```bash
$ cargo test --package config_service --features mysql --no-run
error: the package 'config_service' does not contain this feature: mysql
help: package with the missing feature: digua_common_db
```

`.github/workflows/dialect-matrix.yml` 的 `test-services` job **三条命令两层都错**：

```yaml
cargo test --package audit-service  --features mysql   # ← 层1 dash 包名 + 层2 无 mysql feature
cargo test --package config-service --features mysql   # ← 同上
cargo test --package file-service   --features mysql   # ← 同上
```

**真实包名（下划线）**：`config_service` / `audit_service` / `file_service` / `token_service` /
`notification_service` / `iam_service` / `iam_outbox_publisher` / `prue_backend`。

→ **给服务 crate 加 `[features]` 段不是"改变 CI 语义"，而是"修复一个从未成功过的 job"**。
→ **S1 Step 3c 必须把既有 `test-services` 的三条命令一并修正**（只改包名仍缺 feature）。
→ **S4 的 PG job 必须用下划线包名**（`config_service` 等），否则同样跑不起来。

> ⚠️ **修正后该 job 会第一次真正运行** —— 这是好事，但要**有预期**：它此前从未执行过，
> 因此**可能暴露既有失败**（测试数据/时序/环境依赖等与 PG 无关的问题）。
> 处置：S1 Step 3c 先本地跑一遍这三条命令，把既有失败与 PG 引入的失败**分开归因**；
> 既有失败单独记 issue，**不阻塞** PG 交付，但必须在 S6 文档里说明该 job 的真实基线状态。

#### 1.4.7 方言 feature 一致性的**可执行校验**（不靠人肉记忆）

> 「服务 feature ⇔ 库 feature 同名同义」是方案 3′ 的**核心不变式**（§1.4.3）。
> 它保证「宏的分支可见性」与「库变体是否存在」永远同步。
> **写成文档只是善意提醒，会漂移** → 必须做成**可执行的门禁**。

**校验脚本**：`scripts/check-dialect-features.js`（S4 落地，与 `check-db-naming.js` 并列进 CI）

| # | 校验项 | 判据 | 失败含义 |
|---|---|---|---|
| 1 | **服务依赖声明不硬写方言 feature** | 每个服务 crate 的 `sqlx` / `digua_common_db` / `digua_common` / `digua_common_event` 依赖声明里**不得**出现 `"mysql"` / `"postgres"` / `"sqlite"` | 硬写会让默认构建带上 PG 驱动（违反 §1.4.4 约束 1） |
| 2 | **服务 feature 转译完整** | 每个服务 `[features]` 必须有 `default`/`mysql`/`postgres`(/`sqlite`)，且每个方言 feature 必须**同时**转译 `sqlx/<方言>` 与 `digua_common_db/<方言>` | 漏转译 → 宏的分支与库变体不同步 → `E0004` |
| 3 | **无 default 含 postgres** | `libs/common-db` 与各服务 `[features].default` 均不含 `postgres` | 违反 D-9 / A-7 / F-3 |
| 4 | **库侧 feature 定义完整** | `libs/common-db` 的 `[features]` 必须有 `mysql`/`postgres`/`sqlite`（`default = ["sqlite"]`） | 服务转译的目标不存在 → cargo 直接报错（响亮） |
| 5 | **宏的形态** | `libs/common-db/src/query.rs`：`match &$pool` 次数 ≥ 宏数；`async move` 计数 == 0；裸 `return Err(sqlx::Error` 计数 == 0 | 见 §1.4.8 |

**第 1/2 项的补充校验（需要 cargo，**直接证明默认构建产物零变化**）**：
```bash
# 默认构建：必须为 0（证明默认构建不带 postgres，AGENTS.md 纪律保持）
cargo tree -e features -i digua_common_db | grep -c 'digua_common_db feature "postgres"'
#   期望：0
# 带 postgres：必须为 1
cargo tree -e features -i digua_common_db --features postgres | grep -c 'digua_common_db feature "postgres"'
#   期望：1

# 更强：默认构建里根本不应出现 sqlx-postgres 驱动
cargo tree -i sqlx-postgres 2>/dev/null | grep -q sqlx-postgres && echo "FAIL: 默认构建带上了 PG 驱动"
#   期望：无输出（默认构建不含 sqlx-postgres）
```

**关于 Cargo 的 feature 统一（必须让维护者知道）**：
同名 feature 会被合并到整个 workspace —— 服务 A 开启 `postgres` 会让**所有**服务对 `digua_common_db` 的依赖
都带上 postgres。此时**某个未开 postgres 的服务**，其宏的 PG 分支被剔除（调用方 cfg），但库的 `DbPool::Postgres`
变体**存在**（库 feature 是并集）→ `match` 变非穷尽 → **E0004**。
→ **这正是兜底臂必须存在的原因**（§1.4.9）。**兜底臂不是可选的，是必需的。**

#### 1.4.8 宏的形态约束（实测踩到，与方案选择无关）

| # | 细节 | 写错的后果 |
|---|---|---|
| 1 | `match &$pool`（**不是** `match $pool`） | `$pool` 是拥有值时绑定出 `Pool<DB>`，而 `Executor` 只为 `&Pool<DB>` 实现 → `E0271 the trait bound Pool<Postgres>: Executor<'_> is not satisfied`。`db::connect()` 返回拥有值 `DbPool` → 一定会踩。 |
| 2 | 错误用 `map_err(...)?`（**不要**裸 `return Err(sqlx::Error::...)`） | 裸 return 不做 `From` 转换，调用方返回 `AppError` 时类型不匹配。`?` 运算符会做转换（`impl From<sqlx::Error> for AppError` **已存在**，`libs/common-error/src/lib.rs:79`）。**`?` 形态要求宏位于返回 `Result<_, E>` 的函数内**——所有调用点本来就在 `?` 链上。 |
| 3 | **不要**用 `async move { ... }` 包住 | `.await` 发生在宏块**之外** → inline `format!()` 临时量在 `.await` 前被 drop → `E0716 temporary value dropped while borrowed`。 |
| 4 | 宏的产物是 `Result<T, sqlx::Error>`（**内部已 await 完毕**） | 调用方直接 `?`，**不要**再加 `.await`。 |

#### 1.4.9 🔴 强制项：兜底臂必须存在，且必须「返回受控错误」而非静默

> 主理人要求明确标注为**强制项**——**实施者不得省略**。

**强制要求**：
1. 每一个 `db_*!` / `db_qb_*!` 宏的 `match` **必须**带兜底臂 `_ => Err(...)`，
   错误信息必须**指名道姓**（方言 + 原因），例如：
   ```rust
   _ => Err($crate::sql::to_sqlx_error($crate::sql::SqlRenderError::DialectNotHandled {
       raw_sql: __sql_tpl.to_string(),
       dialect: $crate::DbPool::dialect_of(&$pool),
   })),
   ```
2. 兜底臂**不得**写成"返回空结果"或"panic"或"静默降级"。

**为什么是强制项（实测证明，非推测）**：

**Cargo 的 feature 统一**会把同名 feature 合并到整个 workspace —— 服务 A 开 `postgres`
会让服务 B 对 `digua_common_db` 的依赖也带上 postgres。库的 feature 集合是**所有成员请求的并集**，
**不等于**任何一个成员单独构建时的集合。

→ 于是出现这个场景：**服务 B 没开 postgres**（其宏的 PG 分支被剔除），**但库有 `DbPool::Postgres` 变体**
（因为服务 A 开了）。此时 B 的宏 `match` **少一个臂**。

**我实测了这个场景（探针 `.target/cfg-probe2/`）**：

| 兜底臂 | 结果 |
|---|---|
| **有**（`_ => Err(...)`） | ✅ 编译通过；运行期返回受控错误：`error with configuration: sql render failed [Postgres]` |
| **无** | ❌ **编译失败**：`error[E0004]: non-exhaustive patterns: '&DbPool::Postgres(_)' not covered` |

→ **兜底臂在方案 3′ 下是「必需」而非「可选」**：没有它，**只要 workspace 里有任何一个服务开启 postgres，
所有未开启 postgres 的服务都会编译失败**（E0004）。
→ 有它，则降级为**运行期一条指名道姓的错误**（安全、可观测），而不是**编译期全仓失败**。

> 补充：它同时也是「将来新增第 4 个方言（达梦/MSSQL 已在基座层预留）」的安全网。
> **正因为在单服务构建里不可达，才容易被实施者当成冗余代码删掉**——故标为强制项。

**评审检查**：
```bash
grep -c 'unreachable_patterns' libs/common-db/src/query.rs   # 应 >= 宏数量
```

---

## 2. 接口定义（Rust 签名）

### 2.1 库层：`libs/common-db`

#### 2.1.1 新增模块 `src/sql.rs` —— 方言化的 SQL 渲染器

```rust
use crate::dialect::DialectKind;
use std::borrow::Cow;

/// SQL 渲染错误：占位符出现在字面量/注释内、字面量未闭合，或参数个数与占位符不符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqlRenderError {
    /// `?` 出现在字符串字面量 / 标识符引用 / 注释 / dollar-quoted 内（防回归护栏）
    PlaceholderInLiteral { raw_sql: String, dialect: DialectKind, offset: usize },
    /// 字面量/注释未闭合（静默吞掉是最大风险源，必须报错）
    UnterminatedLiteral { raw_sql: String, dialect: DialectKind, offset: usize, kind: LiteralKind },
    /// 提供的 bind 个数与 `?` 个数不一致（构造期由 `RewrittenQuery::expect_binds` 产生）
    PlaceholderCountMismatch { raw_sql: String, dialect: DialectKind, expected: usize, actual: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind { SingleQuote, DoubleQuote, Backtick, BlockComment, DollarQuote }

impl std::fmt::Display for SqlRenderError { /* 回显 raw_sql + dialect + offset */ }
impl std::error::Error for SqlRenderError {}

/// 已方言化的 SQL + "应绑参数个数"契约（§1.3.3）。
/// `expected_binds` 由扫描器在改写时同步计数得出，与 `sql` 一一对应。
#[must_use = "RewrittenQuery 携带 bind 个数契约，丢弃它等于放弃校验"]
pub struct RewrittenQuery<'a> {
    sql: Cow<'a, str>,
    expected_binds: usize,
}

impl<'a> RewrittenQuery<'a> {
    /// 已方言化的 SQL。**只有通过 `expect_binds` 校验后才能拿到**（消耗式 API）。
    pub fn sql(&self) -> &str;
    pub fn expected_binds(&self) -> usize;
    /// 构造期校验：实际 bind 个数必须等于 expected_binds。
    pub fn expect_binds(self, actual: usize) -> Result<Self, SqlRenderError>;
}

/// 把 MySQL 风格（`?`）的 SQL 渲染为目标方言，并同时产出 bind 个数契约。
/// - Mysql / Dameng / Sqlite / Mssql：SQL 原样（`Cow::Borrowed`，零分配），但仍**统计** `?` 个数
/// - Postgres：`?` → `$1..$N`（`Cow::Owned`）
///
/// 契约：`?` 的**第 k 次出现**（左→右扫描，跳过字面量/标识符/注释/dollar-quote）对应 `$k`，
/// 调用方必须按**同一左→右顺序**调用 `.bind()`。
/// 状态机与边界处理见 §1.3.2。
pub fn render(sql: &str, dialect: DialectKind) -> Result<RewrittenQuery<'_>, SqlRenderError>;

/// 只统计占位符个数（供测试 / 与 QueryBuilder 对拍使用，不做改写）。
pub fn count_placeholders(sql: &str, dialect: DialectKind) -> Result<usize, SqlRenderError>;
```

#### 2.1.2 新增模块 `src/query.rs` —— 方言感知查询宏

> 说明：宏是 `#[macro_export]` 的，签名以**宏模式**给出（PRD 要求"接口定义用代码块给出"，宏模式即其接口）。
>
> ⚠️ **下列定义是已编译验证过的最终形态**（探针 `.target/cfg-probe2/`，sqlx 0.8.6）。
> **宏内不写任何 `#[cfg]`** —— 原因见 §1.4（cfg 求值主体是调用方 crate，会导致分支被全剔除）。
> 抄改前请先读 §1.4。

```rust
/// 读取多行。
/// # 产物类型
/// `Result<Vec<$ty>, sqlx::Error>` —— 宏内部**已经 await 完毕**，调用方直接 `?`，不要再加 `.await`。
/// # 顺序契约
/// `?` 的第 k 次出现 ↔ 第 k 个 bind 实参。二者在同一次宏展开中生成。
/// # 调用约束
/// 必须位于返回 `Result<_, E>`（`E: From<sqlx::Error>`）的函数内——与现有 `.execute(..).await?` 完全一致。
#[macro_export]
macro_rules! db_fetch_all {
    ($pool:expr, $ty:ty, $sql:expr $(, $bind:expr)* $(,)?) => {{
        let __sql_tpl: &str = &$sql;
        // 构造期计数：宏能「看到」bind 实参的个数
        let __n_binds: usize = 0usize $(+ { let _ = &$bind; 1usize })*;
        match &$pool {                                 // ← 注意是 &$pool（见 §1.4.8 细节 1）
            $crate::DbPool::Mysql(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Mysql)
                    .and_then(|rq| rq.expect_binds(__n_binds))    // ← 构造期断言（MySQL 同样校验）
                    .map_err($crate::sql::to_sqlx_error)?;         // ← 用 ?，不要裸 return（细节 2）
                let mut __q = sqlx::query_as::<sqlx::MySql, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*                      // ← 顺序 = 源码顺序
                __q.fetch_all(p).await
            }
            $crate::DbPool::Postgres(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Postgres)
                    .and_then(|rq| rq.expect_binds(__n_binds))
                    .map_err($crate::sql::to_sqlx_error)?;
                let mut __q = sqlx::query_as::<sqlx::Postgres, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*
                __q.fetch_all(p).await
            }
            $crate::DbPool::Sqlite(p) => {
                let __rq = $crate::sql::render(__sql_tpl, $crate::dialect::DialectKind::Sqlite)
                    .and_then(|rq| rq.expect_binds(__n_binds))
                    .map_err($crate::sql::to_sqlx_error)?;
                let mut __q = sqlx::query_as::<sqlx::Sqlite, $ty>(__rq.sql());
                $( __q = __q.bind($bind); )*
                __q.fetch_all(p).await
            }
        }
    }};
}

/// 读取单行（必须有行，否则 RowNotFound）
#[macro_export]
macro_rules! db_fetch_one { /* 同上，fetch_one */ }

/// 读取单行（可无）
#[macro_export]
macro_rules! db_fetch_optional { /* 同上，fetch_optional */ }

/// 标量查询（COUNT(*)、EXISTS 等）
#[macro_export]
macro_rules! db_fetch_scalar { /* -> Result<$ty, sqlx::Error> */ }

/// 写操作（INSERT/UPDATE/DELETE），**直接返回影响行数**（省掉 `.rows_affected()`）
#[macro_export]
macro_rules! db_execute { ($pool:expr, $sql:expr $(, $bind:expr)* $(,)?) => { /* -> Result<u64, sqlx::Error> */ }; }

/// 动态 SQL（条件拼装）场景：把同一份 builder body 在各后端展开一次。
/// `push_bind` 自动产出正确占位符（sqlx 原生能力），无需 render。
#[macro_export]
macro_rules! db_qb_fetch_all { ($pool:expr, $ty:ty, |$qb:ident| $body:block) => { /* -> Result<Vec<$ty>, sqlx::Error> */ }; }
#[macro_export]
macro_rules! db_qb_fetch_optional { /* 同上，fetch_optional */ }
#[macro_export]
macro_rules! db_qb_fetch_one { /* 同上，fetch_one */ }
#[macro_export]
macro_rules! db_qb_execute { /* 同上，execute */ }
```

**配套的库内 API**（`src/sql.rs`）：

```rust
/// SqlRenderError → sqlx::Error 的统一转换（宏内调用点使用）。
pub fn to_sqlx_error(e: SqlRenderError) -> sqlx::Error;

/// 运行期方言判定（**无条件** API，无 cfg）——供诊断/兜底臂使用。
impl DbPool {
    pub fn dialect_of(p: &DbPool) -> DialectKind;
}
```

**四条不可省略的实现细节**（均实测踩到，见 §1.4.8）：

| # | 细节 | 写错的后果 |
|---|---|---|
| 1 | `match &$pool`（**不是** `match $pool`） | `$pool` 是拥有值时绑定出 `Pool<DB>`，`Executor` 只为 `&Pool<DB>` 实现 → `E0271 the trait bound Pool<Postgres>: Executor<'_> is not satisfied` |
| 2 | 错误用 `map_err(...)?`（**不要**裸 `return Err(sqlx::Error::...)`） | 裸 return 不做 `From` 转换，调用方返回 `AppError` 时类型不匹配 |
| 3 | **不要**用 `async move { ... }` 包住 | `.await` 发生在宏块之外 → inline `format!()` 临时量提前 drop → `E0716 temporary value dropped while borrowed` |
| 4 | 宏内**不写任何 `#[cfg]`** | cfg 按调用方 crate 求值 → 分支被全剔除 → `E0004` / 展开成 `()` |

### 2.2 `libs/common-event`：`OutboxRepo` 泛型化（Q5-A）

现状（`src/outbox.rs`）：

```rust
#[async_trait]
pub trait OutboxRepo: Send + Sync {
    async fn enqueue<P: Event>(&self, tx: &mut Transaction<'_, MySql>, env: &mut Envelope<P>) -> Result<(), OutboxError>;
    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;
    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}
```

设计：**trait 泛型化到 DB 维度，同时提供方言无关的枚举入口**。

```rust
/// 方言无关的事务句柄（跨后端）
pub enum AnyTx<'a> {
    #[cfg(feature = "mysql")]  Mysql(&'a mut sqlx::Transaction<'a, sqlx::MySql>),
    #[cfg(feature = "postgres")] Postgres(&'a mut sqlx::Transaction<'a, sqlx::Postgres>),
}

#[async_trait]
pub trait OutboxRepo: Send + Sync {
    /// enqueue 接受方言无关事务：内部按 AnyTx 变体选择 SQL 渲染
    async fn enqueue<P: Event>(&self, tx: &mut AnyTx<'_>, env: &mut Envelope<P>) -> Result<(), OutboxError>;
    async fn claim_batch(&self, limit: u32) -> Result<Vec<OutboxRow>, OutboxError>;
    async fn mark_published(&self, ids: &[i64]) -> Result<(), OutboxError>;
}

/// 方言感知实现：持有 DbPool，内部用 digua_common_db::sql::render 渲染
pub struct SqlOutboxRepo { pool: DbPool, table: String }
impl SqlOutboxRepo {
    pub fn new(pool: DbPool) -> Self;                        // 默认 iam_outbox_events
    pub fn with_table(pool: DbPool, table: impl Into<String>) -> Self;
}
#[async_trait] impl OutboxRepo for SqlOutboxRepo { /* ... */ }

/// 兼容别名：保留旧名，避免 5 个服务的 `use` 语句同时改
#[deprecated(since = "1.7.0", note = "use SqlOutboxRepo")]
pub type MySqlOutboxRepo = SqlOutboxRepo;
```

**逐条 SQL 差异与处置**（`outbox.rs` 内 3 条）：

| 方法 | 现有 SQL 要点 | PG 处置 | 归属 |
|---|---|---|---|
| `enqueue` | `INSERT ... VALUES (?,?,?,?) ON DUPLICATE KEY UPDATE id = id` | `ON CONFLICT (event_id) DO NOTHING`（**冲突目标必填**，实测） | 应用层显式分支（宏内按方言二选一） |
| `claim_batch` | `SELECT ... LIMIT ? FOR UPDATE SKIP LOCKED` | 无需改（PG 支持 `FOR UPDATE SKIP LOCKED`），`?` → `$1` 由 `render` 处理 | 库层抽象 |
| `mark_published` | `SET published_at = CURRENT_TIMESTAMP(3)` | **无需改**（PG 16 接受 `CURRENT_TIMESTAMP(3)`，实测 #1）；列类型 `TIMESTAMPTZ(3)` 保证语义 | 迁移目录 |

> `enqueue` 的方言分叉用**同一个宏调用 + 两条 SQL 常量**表达（`ON CONFLICT` 必须显式声明冲突目标列，无法参数化），这是 PRD US-6 允许的「显式分支 + 注释说明原因」后门：

```rust
const ENQUEUE_MYSQL: &str = "INSERT INTO {} (event_id, event_type, routing_key, envelope_json) \
                             VALUES (?, ?, ?, ?) ON DUPLICATE KEY UPDATE id = id";
const ENQUEUE_PG:    &str = "INSERT INTO {} (event_id, event_type, routing_key, envelope_json) \
                             VALUES (?, ?, ?, ?) ON CONFLICT (event_id) DO NOTHING";
```

### 2.3 `libs/common-health`：`DbProbe` 增加 PG 臂（P0-4）

```rust
enum DbProbe {
    Any(sqlx::AnyPool),
    MySql(sqlx::MySqlPool),
    Postgres(sqlx::PgPool),   // ← 新增
    Sqlite(sqlx::SqlitePool),
}
impl HealthState {
    pub fn with_postgres(mut self, pool: sqlx::PgPool) -> Self;  // ← 新增
}
```

`Cargo.toml` 的 sqlx features 需增 `postgres`（`any` 已在）。

### 2.4 `libs/common`：门面解绑 + `connect()` 返回 `DbPool`（P0-5）

```rust
// before
pub async fn connect(url: &str, max: u32) -> anyhow::Result<MySqlPool>;

// after
pub async fn connect(url: &str, max: u32) -> anyhow::Result<digua_common_db::DbPool>;
```

`Cargo.toml`：
```toml
[features]
default = ["mysql"]                    # 保持默认不变
mysql    = ["sqlx/mysql", "digua_common_db/mysql"]
postgres = ["sqlx/postgres", "digua_common_db/postgres"]
sqlite   = ["sqlx/sqlite", "digua_common_db/sqlite"]

[dependencies]
sqlx = { workspace = true }                          # 去掉硬编码 features=["mysql"]
digua_common_db = { path = "../common-db" }          # 去掉硬编码 features=["mysql"]
```

调用点影响（已核实）：`audit-service/src/main.rs:2`、`iam-service/src/db.rs:24`、`iam-service/src/main.rs:21`、`iam-service/tests/common/mod.rs:67,88`。

### 2.5 `token-service`：删除自建 `DbPool`（P1-12）

现状：`services/token-service/src/db.rs:41` 自建 `pub enum DbPool { Mysql(MySqlPool) }`，与 `digua_common_db::DbPool` 重复。

**处置：删除本地枚举，改用 `digua_common_db::DbPool`**（Q6 裁决明确要求）。同时：
- `init_schema` 里的 `let DbPool::Mysql(p) = pool;`（E0005 源）→ 改为按方言分派（`match pool.as_mysql()/as_postgres()`）。
- `after_connect` 的 `SET time_zone = '+00:00'` 保留（MySQL），PG 侧由 `common-db::init_pool` 统一 `SET TIME ZONE 'UTC'`。
- `config.rs` 的 `DbType` 枚举新增 `Postgres` 变体（P1-3），**保持"未知值忽略"行为不变**（R-12）。

### 2.6 各服务 `ensure_schema` 的方言分派形态

各服务现有 `ensure_schema(pool: &MySqlPool)` 改为 `ensure_schema(pool: &DbPool)`，内部：

```rust
pub async fn ensure_schema(pool: &DbPool) -> anyhow::Result<()> {
    // 1) marker 探测：库层统一入口（内部按方言选 SQL）
    if !digua_common_db::migration_state::probe_marker(pool, SCHEMA_MARKER_TABLE).await? {
        // 2) 执行内嵌迁移：按方言选迁移文本，再按 `;` 切分执行
        let script = match pool.dialect() {
            DialectKind::Postgres => INIT_SQL_POSTGRES,
            _ => INIT_SQL_MYSQL,
        };
        for stmt in split_sql(script) {
            // PG 走 db_execute! 宏；MySQL 同样。无 `?` 的 DDL 两者等价。
            db_execute!(pool, &stmt)?;
        }
    }
    Ok(())
}
```

> DDL 内部**无占位符**（迁移脚本是纯 DDL/DML），因此 `render` 对 MySQL/PG 都近乎直通；`db_execute!(pool, stmt)` 不带 bind 即为零成本。

### 2.7 存量代码的两类改造形态

#### 2.7.A `QueryBuilder` 的 44 处用法（主理人问 A）

> 说明：`QueryBuilder::new(` 实际 **44 处**（`grep -rn "QueryBuilder::new\|QueryBuilder::<" services/ libs/` 计数）。主理人提到的 57 与 PRD 早期口径一致，现以 44 为准（同一批文件，差异来自是否计入 `libs/common-db/src/scope.rs` 与测试）。

**结论：保持 `QueryBuilder` 不动其内部写法，只把「类型参数」和「终结调用」交给宏。**

| 备选 | 可行性 | 结论 |
|---|---|---|
| 改为 `QueryBuilder<'_, DB>` 泛型 | ❌ **不可行**。`push_bind<T>` 要求 `T: Encode<'args, DB> + Type<DB>`；`DB` 泛型时 `&str: Encode<'_, DB>` 不成立（与 §1.3.1 实测同因）。且服务持有的是**运行时** `DbPool` 枚举，`DB` 无法在编译期确定。 |
| 加类型别名 `type MyQb<'a> = QueryBuilder<'a, MySql>` | ❌ 只解决"写得短"，不解决"两种后端"——别名仍是单一 DB。 |
| 保持并按服务方言分支手写两遍 | ⚠️ 可行但重复：每个动态 SQL 站点的 `push`/`push_bind` 逻辑要抄两遍，44 × 2 = 88 份 → 正是要避免的分叉。 |
| **块粘贴宏 `db_qb_*!`** ✅ 采纳 | 同一个 `$body` token 块在两个 `match` 分支各展开一次，分别绑定 `QueryBuilder<MySql>` / `QueryBuilder<Postgres>`。`push_bind` 自动产出正确占位符（实测 [3]）→ **无需 `render`，天然跨方言**。已编译验证通过。 |

**改造前后（真实位置 `services/config-service/src/services/config.rs:78`）**

改造前：
```rust
let mut qb = QueryBuilder::new("SELECT * FROM cfg_namespaces WHERE 1=1");
qb.and_tenant(scope);
qb.push(" ORDER BY created_at DESC LIMIT ");
qb.push_bind(size);
qb.push(" OFFSET ");
qb.push_bind(offset);

let namespaces: Vec<Namespace> = qb
    .build_query_as::<Namespace>()
    .fetch_all(&self.pool)      // self.pool: MySqlPool
    .await?;
```

改造后：
```rust
// db_qb_fetch_all! 把整个 body 在两个后端分支各展开一次；
// and_tenant 已在 S1 对 3 个方言实现 WithTenantScope（§2.1.6）。
let namespaces: Vec<Namespace> = db_qb_fetch_all!(&self.pool, Namespace, |qb| {
    qb.push("SELECT * FROM cfg_namespaces WHERE 1=1");
    qb.and_tenant(scope);
    qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(size);
    qb.push(" OFFSET ").push_bind(offset);
})?;
```

要点：
- **`push_bind` 的顺序由源 token 顺序天然保证**，宏不改写它 → 顺序契约在这里是"免费"的。
- `qb` 的类型由宏分支决定，调用点**不出现** `QueryBuilder<MySql>`/`<Postgres>` 字样。
- `and_tenant`/`where_id`（79 处使用）在 S1 已对方言泛型化，无需在每个调用点判断。

#### 2.7.B `?` 字面量调用点的迁移路径（主理人问 B）

> 口径：**309 个 `sqlx::query*` 调用点，其中 226 个含 `?`**（PRD §5.1 权威口径；单行 grep 只得 102 是因为**漏计跨行 SQL 字面量**——本仓多数 SQL 是跨行 `r#"..."#`，PRD §5.1 已就此专门说明）。
> 所以下面三类形态覆盖全部 309 处：① 有 `?` 的 `query/query_as/query_scalar`（226 处）；② 无 `?` 的（83 处，仍需宏做池分派）；③ `QueryBuilder`（44 处，见 2.7.A，与 ① ② 有重叠，按"调用形态"归类）。

**形态 ①：有 `?` 的 INSERT（真实位置 `services/iam-service/src/service/auth_svc.rs:143`）**

改造前：
```rust
sqlx::query(
    "INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())"
)
.bind(&refresh_token_id)
.bind(&user.id)
.bind(&refresh_token_hash)
.bind(refresh_expires_at)
.bind(ip_address)
.bind(user_agent)
.execute(&self.state.db)     // self.state.db: MySqlPool
.await?;
```

改造后：
```rust
// 6 个 ? ←→ 6 个 bind，顺序与源码一致；::<...> 无需标注（execute 无行类型）
db_execute!(
    &self.state.db,
    "INSERT INTO iam_refresh_tokens (id, user_id, token_hash, expires_at, client_ip, user_agent, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, NOW(), NOW())",
    &refresh_token_id,
    &user.id,
    &refresh_token_hash,
    refresh_expires_at,
    ip_address,
    user_agent,
)?;
```
要点：`NOW()` **原样保留**（PG 有 `NOW()`，见归属表 #2）；`&str`/`String`/`DateTime<Utc>` 这些 bind 类型在两侧都有 `Encode`/`Type` 实现。

**形态 ②：有 `?` 的 UPDATE（真实位置 `auth_svc.rs:155`）**

改造前：
```rust
sqlx::query(
    "UPDATE iam_users SET last_login_at = NOW(), last_login_ip = ?, updated_at = NOW() WHERE id = ?"
)
.bind(ip_address)
.bind(&user.id)
.execute(&self.state.db)
.await?;
```

改造后：
```rust
db_execute!(
    &self.state.db,
    "UPDATE iam_users SET last_login_at = NOW(), last_login_ip = ?, updated_at = NOW() WHERE id = ?",
    ip_address,
    &user.id,
)?;
```

**形态 ③：`INSERT IGNORE` + `?` + `u64`（真实位置 `services/notification-service/src/db.rs:245`，三处差异叠加）**

改造前：
```rust
pub async fn mark_read(&self, ann_id: u64, user_id: String) -> AppResult<bool> {
    let res = sqlx::query(
        "INSERT IGNORE INTO ntf_announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())"
    ).bind(ann_id).bind(user_id).execute(&self.pool).await?;
    Ok(res.rows_affected() == 1)
}
```

改造后（注意 **`u64`→`i64`**、**`INSERT IGNORE`→方言分支**）：
```rust
pub async fn mark_read(&self, ann_id: i64, user_id: String) -> AppResult<bool> {
    // 归属表 #8：INSERT IGNORE 仅 MySQL 有；PG 用 ON CONFLICT DO NOTHING。
    // 两条 SQL 只有这一处关键词不同，其余完全一致 → 用两条 const 表达显式分支。
    const SQL_MYSQL: &str = "INSERT IGNORE INTO ntf_announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW())";
    const SQL_PG:    &str = "INSERT INTO ntf_announcement_reads (announcement_id, user_id, read_at) VALUES (?,?,NOW()) ON CONFLICT (announcement_id, user_id) DO NOTHING";

    let sql = match self.pool.dialect() {
        DialectKind::Postgres => SQL_PG,
        _ => SQL_MYSQL,
    };
    let res = db_execute!(&self.pool, sql, ann_id, user_id)?;
    Ok(res == 1)     // db_execute! 直接返回 rows_affected: u64
}
```
要点：`db_execute!` 返回值就是 `rows_affected()`，省掉 `res.rows_affected()`。

**形态 ④：有行类型 + `fetch_optional`（`prue_backend` 双方言骨架收敛）**

改造前（`services/prue_backend/src/service/menu_config_svc.rs:45-56`，两分支手写两条 SQL）：
```rust
let mysql_sql = format!("SELECT {} FROM prue_menu_configs ...", SELECT_FIELDS_MYSQL);
let sqlite_sql = format!("SELECT {} FROM prue_menu_configs ...", SELECT_FIELDS_SQLITE);
match pool {
    DbPool::Mysql(pool)  => sqlx::query_as(&mysql_sql).fetch_all(pool).await?,
    DbPool::Sqlite(pool) => sqlx::query_as(&sqlite_sql).fetch_all(pool).await?,
}
```

改造后（三方言统一入口，SQL 单份；`SELECT_FIELDS` 的方言差异**前置**为变量，见 §9 U-6）：
```rust
// SELECT_FIELDS 的差异只在于 JSON 取文本的方式：
//   MySQL: CAST(routes_json AS CHAR)   PG: routes_json::text   SQLite: routes_json
let fields = select_fields_for(pool.dialect());
let sql = format!("SELECT {fields} FROM prue_menu_configs WHERE is_active = true AND deleted_at IS NULL ORDER BY version DESC LIMIT 1");
let row: Option<MenuRow> = db_fetch_optional!(pool, MenuRow, &sql)?;
```
要点：SQL 本体**只有一份模板**，方言差异缩到 `select_fields_for(dialect)` 一个 3 行的纯函数里 —— 这就是设计目标「差异收敛、业务 SQL 单份」的具体样子。

**改造量小结**

| 形态 | 数量 | 每处改动 | 风险点 |
|---|---|---|---|
| ① `query` + `?`（写） | ~154 | 1 处调用改宏 + 池类型 | `u64`→`i64`（29 字段） |
| ② `query_as` + `?` | ~143 | 同上 + 行类型保留 | `TIMESTAMPTZ` 列、`bool` 列 |
| ③ `query_scalar` | 12 | 同上 | 标量类型（`i64`/`bool`） |
| ④ 无 `?`（仅需池分派） | ~83 | 同上（宏不带 bind） | 无 |
| ⑤ `QueryBuilder` | 44 | 块粘贴宏（2.7.A） | `and_tenant` 已泛型化 |
| ⑥ 方言特有语法 | ~30 | 显式 `match dialect` + 两条 const | 归属表 #6/#7/#8/#11/#21/#22 |

> ⑥ 是**唯一**允许出现"两条 SQL"的地方，且必须写注释说明为何无法统一（US-6 后门）。实测这 6 类合计约 30 处，占 309 的 10%。

---

## 3. 方言差异归属表（US-6 强制要求）

> 三类归属：**库层抽象**（`libs/common-db` 内解决，业务零感知）/ **迁移目录**（`migrations/postgres/` 内解决，运行期 SQL 不动）/ **应用层显式分支**（确实无法统一，显式 `match` + 注释）。
> 每条都给出**归属依据**与**是否需要改**。

| # | 差异点 | 出现规模 | 归属 | 处置 | 依据 |
|---|---|---|---|---|---|
| 1 | **`?` vs `$N` 占位符** | 309 调用点 / 226 含 `?` / 561 个 `?` | **库层抽象** | `sql::render` + `db_*!` 宏（§2.1） | 构造期一次性产出，顺序契约由宏展开保证（§1.3.1） |
| 2 | `NOW()` | 171（rs 161 + sql 10） | **库层抽象**（不改写） | **无需改写**：PG 有 `NOW()`。但注意返回 `timestamptz`；配合 #3 的 `TIMESTAMPTZ` 列即为绝对时刻，实测会话 `Asia/Shanghai` 下往返 `delta_ms = 0` | 实测 + 探针 |
| 3 | **`DATETIME` / `DATETIME(3)`** | 57 | **迁移目录** | → **`TIMESTAMPTZ` / `TIMESTAMPTZ(3)`**（**不是** `TIMESTAMP`） | **D-3 实测**：`TIMESTAMP -> DateTime<Utc>` 报 `mismatched types`（`type_compatible` 严格相等），而仓库有 75 个 `DateTime<Utc>` 字段 |
| 4 | `CURRENT_TIMESTAMP(3)` 默认值 | 9 | **迁移目录** | **无需改写**（PG 16 实测评测 #1 接受并按 3 位截断）。列类型配 `TIMESTAMPTZ(3)` → 存储绝对时刻，会话 TZ 无关（实测 `stored_utc = 11:40` 正确） | Q7-A + 实测 #1/#2 |
| 5 | **`JSON` 列型** | 8 | **迁移目录** | **无需改写**：PG 原生 `JSON`（实测 #2 接受）。且 sqlx 的 `Json<T>::compatible` **同时接受** `JSON` 与 `JSONB`（源码核对 `sqlx-postgres/src/types/json.rs:22`），`serde_json::Value` 读写 `JSON` 列**实测 OK** | 实测 #2 + 探针 `JSON -> Value = OK` |
| 6 | **`GROUP_CONCAT`** | 2 | **应用层显式分支** | → `STRING_AGG(x::text, ',')`（实测返回 `1,2`）。**必须 `::text` 强转**，否则 bigint 参数类型不匹配 | 实测 + 扫描 `internal_membership.rs` |
| 7 | **`ON DUPLICATE KEY UPDATE`** | 14 | **应用层显式分支** | → `ON CONFLICT (<冲突目标列>) DO UPDATE SET ...` / `DO NOTHING`。**冲突目标必填**（实测） | 实测 #6 |
| 8 | **`INSERT IGNORE`** | 18 | **应用层显式分支** | → `INSERT ... ON CONFLICT DO NOTHING` | PG 无 `INSERT IGNORE` |
| 9 | **`information_schema.tables` + `DATABASE()`** | 15 | **库层抽象** | `probe_marker` 统一入口；PG 用 `table_schema = current_schema()`（实测返回正确） | 实测 |
| 10 | **`information_schema.statistics`** | 2（仅 iam 测试） | **应用层显式分支** | → `pg_catalog.pg_indexes` / `pg_indexes` 视图 | PG 无此视图 |
| 11 | **`GET_LOCK` / `RELEASE_LOCK`** | 11 + 9 | **应用层显式分支** | → `pg_try_advisory_lock(hashtext($1))` / `pg_advisory_unlock(hashtext($1))`（实测返回 `t`）。**⚠️ 语义并非完全等价**（见下方「分布式锁语义差异」小节） | 实测 `pg_try_advisory_lock` 返回 `t` |
| 12 | **`TINYINT(1)`** | 5 | **迁移目录** | → `BOOLEAN` | 实测 #7 |
| 13 | **`AUTO_INCREMENT`** | 9 | **迁移目录** | → `GENERATED ALWAYS AS IDENTITY`（基座层 `dialect::pg::identity::identity_column` 已有） | 实测 |
| 14 | **列内联 `COMMENT '...'`** | 57 | **迁移目录** | → 丢弃（推荐）或 `COMMENT ON COLUMN t.c IS '...'`。**推荐丢弃**：注释不参与语义，保留会显著增加迁移长度与 `split(';')` 的语句数 | 实测 #8 |
| 15 | **内联 `INDEX idx_x (...)`** | 26 | **迁移目录** | → 独立 `CREATE INDEX idx_x ON t (...)` | 实测 #9 |
| 16 | **内联 `UNIQUE KEY uk_x (...)`** | 10 | **迁移目录** | → 独立 `CREATE UNIQUE INDEX uk_x ON t (...)` | 实测 |
| 17 | **表尾 `ENGINE=InnoDB DEFAULT CHARSET=... COLLATE=...`** | 18 | **迁移目录** | 删除 | 实测 #10 |
| 18 | **`ON UPDATE CURRENT_TIMESTAMP`** | 2 | **应用层显式分支**（Q2-A） | PG 迁移去掉该子句；Rust 侧 UPDATE 显式写 `updated_at = now()`。**token-service 需补 2 处**（`oauth_client_svc.rs:311,348`） | Q2-A；`split(';')` 与 `$$` 不兼容是客观约束 |
| 19 | **`u64` 字段 / `.bind(x as u64)`** | **DB 相关的 5 字段 + 4 绑定点**（完整清单见计划文档 S1「u64 → i64 清单」） | **应用层显式分支** | → **`i64`**（PG `BIGINT` 有符号）。**同时** MySQL 侧 `BIGINT UNSIGNED` DDL 需改 `BIGINT`（否则 `sqlx-mysql` 的 `int_compatible` 拒绝 `UNSIGNED` 标志解码到 `i64`） | **D-4 实测**：`sqlx-postgres` 无任何 unsigned `Type`/`Decode`；`bigint out of range` |
| 20 | **`bool` 列 / `.bind(x as i8)`** | 28 bool 字段 / 2 处 `as i8` | **应用层显式分支** | PG 侧 `BOOLEAN` 列；去掉 `.bind(a.pinned as i8)` 的 `as i8`（改 `bool`） | **D-5 实测**：`BOOLEAN -> i8` ❌ / `BOOLEAN -> bool` ✅ / MySQL `TINYINT(1) -> bool` ✅ |
| 21 | **`NOW(3)`** | 10（iam-service 6 + iam-outbox-publisher 4） | **应用层显式分支** | → `now()`（列已 `TIMESTAMPTZ(3)`，精度由列定义截断；实测 `now()::timestamp(3)` 正确） | **D-6 实测**：`function now(integer) does not exist` |
| 22 | **`INTERVAL ? HOUR` / `INTERVAL ? DAY`** | 10 | **应用层显式分支** | → `($N * INTERVAL '1 hour')`（实测 `PREPARE ... EXECUTE` 通过）。**注意**：`render` 后 `?`→`$N` 仍不够，因为 `INTERVAL $N HOUR` 语法非法，必须改成乘法式 | **D-7 实测**：`syntax error at or near "$1"` |
| 23 | **`UNSIGNED` 列型** | 10 | **迁移目录** | → `BIGINT`（`tenant_id` 等）或 `INT`。取值上界由业务保证（tenant_id 远小于 `i64::MAX`） | 实测 `bigint out of range`；与 #19 联动 |
| 24 | **`FOR UPDATE SKIP LOCKED`** | outbox `claim_batch` | **无需改** | PG 原生支持 | 实测 |
| 25 | **会话时区** | 全服务 | **库层抽象**（Q2-bis） | PG `after_connect` 统一 `SET TIME ZONE 'UTC'`（§2.1.4） | 实测：非 UTC 会话下 `CURRENT_TIMESTAMP` 写入 naive 列偏移 +08h |
| 26 | **`?` 出现在字面量内** | **0**（实测） | **库层抽象** | `render` 内**断言**：字面量/注释内发现 `?` → `SqlRenderError`（防回归） | PRD 配套要求 |

#### 分布式锁语义差异（`GET_LOCK` → `pg_try_advisory_lock`，**不等价**）

> ⚠️ **实施者不要以为两者等价**。主理人要求在设计里明确标注。

| 维度 | MySQL `GET_LOCK(name, 0)` | PG `pg_try_advisory_lock(hashtext(name))` |
|---|---|---|
| 非阻塞获取 | `0` 超时 = 不等待 | `pg_try_*` = 不等待 ✓ **一致** |
| 作用域 | **session 级**（同一连接） | **session 级**（同一连接）✓ 一致 |
| **释放时机** | `RELEASE_LOCK(name)` 显式释放；**连接断开也会释放** | `pg_advisory_unlock(...)` 显式释放；**连接断开也会释放** ✓ 一致 |
| **持有期间的连接占用** | 必须**独占一条连接**直到释放（仓库现有代码正是如此：`pool.acquire()` 后全程持有） | 同上 ✓ 一致 |
| 锁名 → 键的映射 | 字符串 | `hashtext()` → **int4**（32 位）→ **理论上存在哈希碰撞**（不同锁名映射到同一键） |
| 跨实例共享 | 同一 MySQL 实例内共享 | 同一 PG 实例内共享 ✓ |

**仓库现状**（`services/iam-service/src/outbox_archive.rs:50,67` 与 `services/iam-outbox-publisher/src/outbox_archive.rs`）：
只有 **1 个锁名** `iam_outbox_archive` → 碰撞风险**实际为零**（单键无碰撞）。
且现有注释已明确「GET_LOCK/RELEASE_LOCK 是 session-scoped，跨连接会泄漏 8 小时」——
PG 的 session 级语义与之一致，**沿用「单连接持有锁」的现有约束即可**。

**未来锁名增多时的升级路径（记录备查）**：
改用 `pg_try_advisory_lock($1::bigint)`，由 **Rust 侧计算稳定 64 位哈希**（如 `xxhash`/`fnv`），
彻底消除 `hashtext` 的 32 位碰撞面。届时 MySQL 侧可保留 `GET_LOCK(name,0)`（字符串键）
或同步改为 `GET_LOCK(<hash字符串>,0)`。

**PG 侧必须显式 `pg_advisory_unlock`**：虽然连接断开会自动释放，但归档任务持有连接期间
若异常返回而不解锁，锁会**一直持有到连接归还/断开**——与 MySQL 侧同样需要小心，
**不可依赖"连接池会回收"来兜底**。

**"无需改"结论汇总**（明确记录，避免实施者重复劳动）：
- `CURRENT_TIMESTAMP(3)`（#4）：PG 16 接受精度参数，实测按 3 位截断。
- `JSON` 列型（#5）：PG 原生支持；sqlx `Json<T>` 兼容 `JSON` 与 `JSONB` 两者。
- `FOR UPDATE SKIP LOCKED`（#24）：PG 原生支持。
- `NOW()`（#2）：PG 有该函数；只需配合 `TIMESTAMPTZ` 列。

---

## 4. PG 迁移集规范

### 4.1 文件与目录

```
services/<svc>/migrations/postgres/V001__init.sql
services/<svc>/migrations/postgres/V002__<same-name-as-mysql>.sql
```

| 规则 | 内容 |
|---|---|
| **目录** | `migrations/postgres/`，与 `migrations/mysql/` **同级并列**（不嵌套）；`prue_backend` 保持 `{mysql,sqlite,postgres}` 三轨 |
| **文件名** | 与 MySQL 侧**同名同序号**（`V001__init.sql` ↔ `V001__init.sql`）。序号与后缀必须一致，便于 B-1/B-6 自动比对 |
| **表名** | 必须带服务前缀（`iam_`/`audit_`/`token_`/`ntf_`/`cfg_`/`file_`/`prue_`），`check-db-naming.js` 守门 |
| **禁止** | ❌ 编辑任何已发布的 MySQL 迁移（forward-only）；❌ `$$`/`DO` 块（`split(';')` 会切坏）；❌ `CREATE FUNCTION`/`CREATE TRIGGER`（Q2-A 已排除触发器路线） |

### 4.2 版本号对齐 + marker 策略

各服务 `ensure_schema` 是"按版本顺序、逐个 marker 判定"的**幂等**执行器：

```
for each V00N:
    if probe_marker(<该版本的 marker_table>) -> 跳过
    else 执行 migrations/<dialect>/V00N__*.sql，然后（若需要）创建该版本 marker 表
```

- **marker table 名必须与 MySQL 侧完全一致**（如 `cfg_namespaces`、`token_migration_v002_applied`），这样 `MigrationDescriptor.marker_table` 常量**两端共用一份**，`/internal/migrations/state` 端点的 `dialect` 字段才能如实反映。
- **幂等性保证**（B-2「重复执行不报错」）三重手段：
  1. `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`（PG 支持 `IF NOT EXISTS` 与 MySQL 语法一致）。
  2. marker 探测前置：已应用则整版跳过。
  3. 幂等 DML：`INSERT ... ON CONFLICT DO NOTHING` 替代 `INSERT IGNORE`；`ON CONFLICT (cols) DO UPDATE SET x = x` 替代 `ON DUPLICATE KEY UPDATE x = x`。
- **版本号必须与 MySQL 一一对应**（B-6）：这保证 marker 语义在两端等价（同一 V 版本建同一批表）。

### 4.3 `split(';')` 约束如何被门禁保证

现状：7 个服务 × `ensure_schema` 都用 `script.split(';')` + 剥 `--` 行注释。这**不支持** dollar-quoted，因此：

| 约束 | 门禁手段 |
|---|---|
| PG 迁移内不得出现 `$$` / `DO` / `CREATE FUNCTION` / `CREATE TRIGGER` | **新增脚本** `scripts/check-pg-migrations.js`（S0 落地）：grep `migrations/postgres/**/*.sql`，命中即退出码 1。同时检 `AUTO_INCREMENT`/`ON DUPLICATE KEY`/`ENGINE=`/`UNIQUE KEY`/`TINYINT(1)`/`COMMENT '`/`DATETIME`/`UNSIGNED` 残留（即 B-4） |
| 列注释带来语句膨胀 | 推荐**直接丢弃** COMMENT（#14），而非生成 `COMMENT ON`，避免 `split(';')` 语句数与 MySQL 侧差异过大 |

> 该脚本把 B-4（PG 迁移零 MySQL 语法残留）从"人工 grep"升级为 CI 门禁，与 `check-db-naming.js` 并列。

### 4.4 DDL 类型映射对照表（写进 `contracts/db-conventions.md`）

| MySQL | PG | 备注 |
|---|---|---|
| `VARCHAR(n)` / `TEXT` / `CHAR(n)` | 同 | |
| `BIGINT` / `INT` | 同 | |
| `BIGINT UNSIGNED` / `INT UNSIGNED` | `BIGINT` / `INT` | **必须去 UNSIGNED**；Rust 侧 `u64`→`i64` |
| `TINYINT(1)`（布尔语义） | `BOOLEAN` | Rust 侧必须是 `bool`（不是 `i8`） |
| `TINYINT` / `SMALLINT`（非布尔） | `SMALLINT` | |
| `DATETIME` / `DATETIME(3)` | **`TIMESTAMPTZ` / `TIMESTAMPTZ(3)`** | **D-3 关键**：Rust `DateTime<Utc>` 只能解 `TIMESTAMPTZ` |
| `TIMESTAMP` | `TIMESTAMPTZ` | 同上 |
| `DATE` | `DATE` | |
| `JSON` | `JSON`（可保原样）或 `JSONB` | 两者 sqlx 都能读 |
| `DECIMAL(p,s)` | `NUMERIC(p,s)` | |
| `BLOB` | `BYTEA` | |
| `AUTO_INCREMENT` | `GENERATED ALWAYS AS IDENTITY` | |
| 内联 `INDEX` | 独立 `CREATE INDEX` | |
| 内联 `UNIQUE KEY` | 独立 `CREATE UNIQUE INDEX` | |
| 列 `COMMENT '...'` | 丢弃 | |
| 表尾 `ENGINE=`/`CHARSET=`/`COLLATE=` | 删除 | |
| `DEFAULT CURRENT_TIMESTAMP(3)` | `DEFAULT CURRENT_TIMESTAMP(3)` | **保持原样**（实测接受） |

### 4.5 会话 TZ 兜底

即便全部用 `TIMESTAMPTZ`（绝对时刻，天然 TZ 无关），仍按 Q2-bis 在 `common-db::init_pool` 统一 `SET TIME ZONE 'UTC'`：
- 理由 1：`now()` 的**文本展示**（日志、`dump-openapi` 样例）与 `date_trunc('day', ...)` 等会话敏感计算需要稳定基准。
- 理由 2：万一某列漏改仍是 `TIMESTAMP`（naive），UTC 会话能让 `CURRENT_TIMESTAMP` 写入 UTC 值，与 MySQL 侧 `SET time_zone='+00:00'`（token-service 已有）行为一致。
- 理由 3：单点修复优于 9 处迁移各写一遍 TZ 表达式（Q7 裁决理由）。

---

## 5. feature 开关矩阵

### 5.1 crate × 方言矩阵

| crate | `mysql` | `postgres` | `sqlite` | **default** | 说明 |
|---|---|---|---|---|---|
| `digua_common_db` | ✅ 既有 | ✅ 既有（`["sqlx/postgres"]`） | ✅ 既有 | `["sqlite"]` **不动** | 基座层已完成 |
| `digua_common`（门面） | ✅ 新增 feature | ✅ 新增 | ✅ 新增 | `["mysql"]` | 解除硬编码 `features=["mysql"]` |
| `digua_common_event` | ✅ 新增 feature | ✅ 新增 | — | `["mysql"]` | `AnyTx` 变体按 feature 编译 |
| `digua_common_health` | ✅ | ✅ 新增到 sqlx features | ✅ | 父 crate 决定 | `any` 已启用 |
| `config_service` | ✅ | ✅ 新增 feature | — | `["mysql"]` | |
| `file_service` | ✅ | ✅ | — | `["mysql"]` | |
| `audit_service` | ✅ | ✅ | ✅（现依赖 `common-db/sqlite`） | `["mysql"]` | |
| `prue_backend` | ✅ | ✅ | ✅ | `["mysql","sqlite"]` | 三轨 |
| `token_service` | ✅ | ✅ | — | `["mysql"]` | |
| `notification_service` | ✅ | ✅ | — | `["mysql"]` | 含 `ntf_outbox_publisher` bin |
| `iam_service` | ✅ | ✅ | — | `["mysql"]` | |
| `iam_outbox_publisher` | ✅ | ✅ | — | `["mysql"]` | 独立二进制 |

服务 `Cargo.toml` 统一形态：

```toml
[features]
default  = ["mysql"]
mysql    = ["sqlx/mysql", "digua_common_db/mysql", "digua_common/mysql", "digua_common_event/mysql"]
postgres = ["sqlx/postgres", "digua_common_db/postgres", "digua_common/postgres", "digua_common_event/postgres"]
```

### 5.2 `postgres` 不进 default 的**验证方式**（D-9）

三条独立命令，互不依赖（放进 CI + code review 清单）：

```bash
# 1) 声明层面：default 行不含 postgres
grep -n 'default = ' libs/common-db/Cargo.toml        # 期望: default = ["sqlite"]
grep -rn '^default' services/*/Cargo.toml              # 期望: 无 postgres
# 2) 解析层面：默认构建的 feature 图不含 postgres
cargo tree --workspace -e features -i digua_common_db | grep -c postgres   # 期望: 0
# 3) 产物层面：默认构建与改动前逐字节一致（对比 SHA）
cargo build --workspace --offline 2>/dev/null && sha256sum target/debug/*.rlib | sha256sum
```

> 第 3 条在 CI 里用「改动前 benchmark 值」对比，只需在 S1 首次落地时记录基线。

---

## 6. 测试基建设计

### 6.1 `tests/common/mod.rs` 后端参数化 fixture

现状（config/file 为例）：`spawn_mysql()` 返回 `MysqlHandle { pool: MySqlPool, container }`，模式二选一（共享容器 `TEST_MYSQL_HOST/PORT` vs testcontainers）。

**设计：引入统一的 `TestDb` 抽象 + 后端枚举，同一测试文件参数化跑两遍。**

```rust
// tests/common/mod.rs（每个 P0/P1 服务同构）
pub enum Backend { MySql, Postgres }

pub struct TestDb {
    pub pool: digua_common_db::DbPool,
    pub backend: Backend,
    /// Some = testcontainers 本地拉起（drop 即销毁）；None = 共享容器
    _guard: Option<Box<dyn std::any::Any + Send + Sync>>,
}

/// 参数化入口：读 `TEST_BACKENDS`（默认 "mysql"）。
/// - "mysql,postgres" → 返回两个 TestDb，测试内 for-each 跑两轮
/// - 单后端 → 返回一个
/// - 后端连通性不可用（无 TEST_PG_URL 且 testcontainers 启动失败）→ **跳过该后端**并打印 `[skip]`
pub async fn spawn_all() -> Vec<TestDb>;

/// 单后端入口（保留给"只测 MySQL"的既有测试，零改动迁移路径）
pub async fn spawn(backend: Backend) -> Option<TestDb>;
```

**"缺 `TEST_PG_URL` 即跳过而非失败"** 的判定顺序（与现有 MySQL 行为一致）：

1. 有 `TEST_PG_URL` → 直连共享容器（`coder-test-pg`，端口 5434）。
2. 无 `TEST_PG_URL` 且 `cfg!(test)` 允许 testcontainers → 起 `testcontainers_modules::postgres::Postgres`。
3. 步骤 2 失败（如 CI 无 docker）→ **打印 `[skip] postgres backend unavailable` 并 `return None`**，测试体 `for db in spawn_all().await` 自然跳过。

`testcontainers-modules` 已确认 0.8.0 含 `postgres` feature（PRD 已核实）。

### 6.2 testcontainers PG 模块用法

```rust
// Cargo.toml (dev-dependencies)
testcontainers-modules = { version = "0.8", features = ["mysql", "postgres"] }
```

```rust
use testcontainers_modules::postgres::Postgres;
let c = Postgres::default().start().await.expect("start pg");
let port = c.get_host_port_ipv4(5432).await?;
let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
```

> 注意：`Postgres::default()` 的默认用户/库是 `postgres`/`postgres`（与 MySQL 模块的 `root`/`test` 不同），URL 拼接需按此。

### 6.3 `scripts/lib/shared-db.js` 的 `coder-test-pg` 对接（Q8-A）

现状：`MYSQL_CONTAINER = 'coder-test-mysql'`、`REDIS_CONTAINER`，导出 `ensureMysql/ensureRedis`。

新增（**与 `ensureMysql` 完全对称**）：

```js
const PG_CONTAINER = 'coder-test-pg';

/**
 * 确保 PostgreSQL 可用：探活 → 拉起已有容器 → 再探。语义与 ensureMysql 一致。
 * @returns {Promise<boolean>}
 */
async function ensurePostgres(host, port, log = console.log) { /* 同构 */ }

module.exports = { MYSQL_CONTAINER, REDIS_CONTAINER, PG_CONTAINER,
                   probeTcp, startExistingContainer, ensureMysql, ensureRedis, ensurePostgres };
```

**端口隔离**：`coder-test-pg` 用 **5434**（基座层设计已定），与 `aipm-pg-dev:5433` 隔离，避免撞端口。

### 6.4 与既有 MySQL fixture 的兼容策略

- **不改** `spawn_mysql()` 的既有签名 → 现有测试零改动。
- 新增 `spawn_all()` / `spawn(Backend)` → 新测试用参数化入口。
- 迁移顺序：S3 先让 P0 三服务的 `spawn_mysql()` **内部**改为 `spawn(Backend::MySql).unwrap()`（薄包装），保证既有测试仍绿；再逐步把测试体改为 `spawn_all()`。

### 6.5 `sql::render` 单测表（**纯函数，无需 DB，是 S1 的第一道验收**）

`libs/common-db/tests/sql_render_test.rs` 必须覆盖下表全部 18 条。这是本设计的**最低防线**：
只要这张表全绿，226 个调用点的改造就只剩机械替换。

| # | 输入 SQL | dialect | 期望输出 / 期望 Err | 覆盖的状态迁移 |
|---|---|---|---|---|
| 1 | `SELECT * FROM t WHERE a = ? AND b = ?` | PG | `... a = $1 AND b = $2`，`expected_binds = 2` | Normal |
| 2 | 同 1 | MySQL | **原样**，`Cow::Borrowed`，`expected_binds = 2` | Normal（零分配） |
| 3 | 同 1 | SQLite | 原样，`expected_binds = 2` | Normal |
| 4 | `SELECT * FROM t WHERE a = ? AND b = ?` | PG | 调用 `.expect_binds(1)` → `Err(PlaceholderCountMismatch{expected:2, actual:1})` | 构造期计数契约 |
| 5 | `SELECT '?' AS a` | PG | `Err(PlaceholderInLiteral{offset:8})` | Normal→InSingle→`?` |
| 6 | `SELECT 'it''s ?' AS a` | PG | `Err(PlaceholderInLiteral)` | InSingle 遇 `''` **停留** |
| 7 | `SELECT 'a' , ? ` | PG | `... , $1`，`expected_binds = 1` | InSingle 正常闭合 |
| 8 | `SELECT "a?b" FROM t WHERE id = ?` | PG | `SELECT "a?b" ... id = $1` | InDouble 内 `?` 不计数 |
| 9 | ``SELECT `a?b` FROM t WHERE id = ?`` | MySQL | 原样，`expected_binds = 1` | InBacktick |
| 10 | `-- ?\nSELECT 1` | PG | `Err(PlaceholderInLiteral)` | InLineComment |
| 11 | `/* ? */ SELECT 1` | PG | `Err(PlaceholderInLiteral)` | InBlockComment |
| 12 | `SELECT $$ ? $$ , ?` | PG | `Err(PlaceholderInLiteral)`（`?` 在 dollar-quoted 内） | InDollarQuote |
| 13 | `SELECT 1 WHERE x = 'unclosed` | PG | `Err(UnterminatedLiteral{kind:SingleQuote})` | 未闭合 |
| 14 | `SELECT 1 /* unclosed` | PG | `Err(UnterminatedLiteral{kind:BlockComment})` | 未闭合 |
| 15 | `SELECT 1 WHERE a = ? AND b = $1` | PG | `... a = $1 AND b = $1`，`expected_binds = 1`（`$1` 不是 `?`，不计数） | 已有 `$N` 不被误改 |
| 16 | `SELECT * FROM t WHERE name LIKE '?%' AND id = ?` | PG | `Err(PlaceholderInLiteral)` —— **防回归核心用例** | 混合 |
| 17 | `INSERT INTO t VALUES (?,?,?,?)` | PG | `... ($1,$2,$3,$4)` | 紧凑多占位符 |
| 18 | 同 17 | MySQL | 原样，`expected_binds = 4` | MySQL 侧也计数（**封堵静默错位**，见 §1.3.3 第 5 点） |
| 19 | `SELECT $$a?b$$, $1` | PG | `SELECT $$a?b$$, $1`（**原样**），`expected_binds = 0` | **dollar-quoted 整段跳过**：其中的 `?` 不编号、不报错；`$1` 不是 `?`，也不计数 |
| 20 | `SELECT $$a?b$$ FROM t WHERE id = ?` | PG | `SELECT $$a?b$$ FROM t WHERE id = $1`，`expected_binds = 1` | dollar-quoted 与真占位符共存时只后者编号 |

**与 `QueryBuilder` 对拍**（利用主理人实测 [3]，把正确性锚定在 sqlx 自身行为上）：
```rust
// 对 #1 / #17 这类纯占位符 SQL，我方 render 的产物必须与 QueryBuilder<Postgres> 一致
#[test]
fn render_matches_query_builder() {
    let sql = "SELECT $1::text AS a, $2::text AS b";   // qb 的产物（实测 [5]）
    let ours = render("SELECT ?::text AS a, ?::text AS b", DialectKind::Postgres).unwrap();
    assert_eq!(ours.sql(), sql);
    assert_eq!(Postgres_format_placeholder_sequence(), "$1,$2");
}
```

---

## 7. CI 与编排设计

### 7.1 `.github/workflows/dialect-matrix.yml` 新增 job

**不改动**既有 `test-sqlite`/`test-mysql`/`test-postgres`/`test-services` job（C-6「不回归」）。

```yaml
  # 服务层 PG 集成测试 - PR 级（新增）
  test-services-postgres:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:16
        env:
          POSTGRES_PASSWORD: coder
          POSTGRES_DB: coder_test
        options: >-
          --health-cmd=pg_isready
          --health-interval=10s
          --health-timeout=5s
          --health-retries=5
        ports:
          - 5432:5432
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2          # 缓存 cargo 构建产物（R-11 缓解）
      - name: Run service tests on PostgreSQL
        env:
          TEST_PG_URL: postgres://postgres:coder@localhost:5432/coder_test
        run: |
          # P0 三服务先跑，P1 四服务 + publisher 随后（同 job，串行，失败即红）
          cargo test -p config_service --features postgres
          cargo test -p file_service   --features postgres
          cargo test -p prue_backend   --features postgres
          cargo test -p audit_service  --features postgres
          cargo test -p token_service  --features postgres
          cargo test -p notification_service --features postgres
          cargo test -p iam_service    --features postgres
          cargo test -p iam_outbox_publisher --features postgres
```

设计要点：
- **PG job 与 MySQL job 并行**（不同 job，天然并行），R-11「时长」缓解。
- 用 `rust-cache` 缓存，避免每 job 重编译。
- **不引入 matrix**：8 个 crate 串行在一个 job 内，失败信息更直观（也不需要为每 crate 单开 service container）。若时长超限，退化为 `strategy.matrix.crate` 拆分 job。

### 7.2 `infrastructure/` compose 的 profile 隔离 PG service

`infrastructure/docker-compose-infra.yml` 新增（**profile 隔离，默认不启动**，US-3 要求）：

```yaml
  postgres:
    image: ${postgres:-postgres:16}
    hostname: postgres
    profiles: ["pg"]                 # ← 关键：无 profiles 匹配时默认不启动
    environment:
      POSTGRES_PASSWORD: ${PG_PASSWORD:-coder}
      POSTGRES_DB: ${PG_DATABASE:-coder_test}
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
    volumes:
      - postgres_data:/var/lib/postgresql/data
    networks:
      - superpowers
```

`docker-compose-infra.dev.yml` 新增端口映射（dev 覆盖层）：

```yaml
  postgres:
    ports:
      - "${PG_PORT_EXPORT:-5434}:5432"    # 5434，与 aipm-pg-dev:5433 隔离
```

启动方式：`docker compose --profile pg up -d`。**默认 `up` 不启 PG**，既有 MySQL 用户零感知。

### 7.3 门禁脚本扩展

| 脚本 | 改动 | 目的 |
|---|---|---|
| `scripts/check-db-naming.js` | ① `VALID_PREFIXES` 去双逗号；② `TABLE_PATTERNS` 增加 `ON\s+CONFLICT` 前置排除（等价于现有 `isOnUpdateClause` 的 `DO UPDATE SET` 处理）；③ `SKIP_IN_SOURCE` 增 `pg_catalog`、`current_schema`、`pg_indexes` | P0-6 / C-1 / C-1b |
| `scripts/check-pg-migrations.js` | **新增**：扫 `services/*/migrations/postgres/*.sql`，禁止 MySQL 残留语法 + `$$`/`DO`/`CREATE FUNCTION`/`CREATE TRIGGER` | B-4 + §4.3 约束 |

> 两个脚本都挂进 `package.json` 的 `check` 聚合与 CI `test` 阶段。

---

## 8. 数据流与时序

### 8.1 服务启动 → 连接 → ensure_schema → 就绪

```mermaid
sequenceDiagram
    autonumber
    participant Main as main.rs
    participant Cfg as config (DATABASE_URL)
    participant CDB as digua_common_db
    participant Pool as DbPool
    participant Svc as <svc>::db::ensure_schema
    participant MS as migration_state
    participant PG as PostgreSQL / MySQL

    Main->>Cfg: 读 database.url
    Cfg-->>Main: "postgres://..." | "mysql://..."
    Main->>CDB: init_pool(url)
    CDB->>CDB: DialectKind::from_url(url)
    alt Dialect = Postgres
        CDB->>PG: PgPoolOptions::connect
        Note over CDB,PG: after_connect: SET TIME ZONE 'UTC'（Q2-bis）
        CDB-->>Pool: DbPool::Postgres(PgPool)
    else Dialect = Mysql
        CDB->>PG: MySqlPoolOptions::connect
        CDB-->>Pool: DbPool::Mysql(MySqlPool)
    end
    Main->>Svc: ensure_schema(&pool)
    loop 每个 V00N（版本升序）
        Svc->>MS: probe_marker(&pool, marker_table)
        alt Postgres
            MS->>PG: SELECT COUNT(*) FROM information_schema.tables<br/>WHERE table_schema = current_schema() AND table_name = $1
        else MySQL
            MS->>PG: ... WHERE table_schema = DATABASE() AND table_name = ?
        end
        PG-->>MS: count
        alt marker 不存在
            Svc->>PG: 逐语句执行 migrations/<dialect>/V00N__*.sql
            Svc->>PG: CREATE TABLE IF NOT EXISTS <marker>（如需）
        else marker 已存在
            Note over Svc: 整版跳过（幂等）
        end
    end
    Svc-->>Main: Ok(())
    Main->>Main: 启动 HTTP server（/health 就绪）
```

### 8.2 一次带 `?` 的查询在两种后端的路径

```mermaid
sequenceDiagram
    autonumber
    participant Biz as 业务代码
    participant Macro as db_fetch_all! 宏
    participant Render as sql::render
    participant SQ as sqlx QueryAs
    participant DB as 后端

    Biz->>Macro: db_fetch_all!(&pool, Row,<br/>"SELECT ... WHERE a = ? AND b = ?", a, b)
    Note over Macro: 一份 SQL 源码 + 一份 bind 序列<br/>（顺序契约：第 k 个 ? ↔ 第 k 个 bind）

    alt pool = DbPool::Mysql
        Macro->>Render: render(sql, Mysql)
        Render-->>Macro: Cow::Borrowed(sql)（零分配，原样）
        Macro->>SQ: query_as::<MySql, Row>(sql)
        Macro->>SQ: .bind(a).bind(b)   %% 顺序 = 源码顺序
        SQ->>DB: prepare("SELECT ... WHERE a = ? AND b = ?")
        DB-->>SQ: 结果（MySQL 只认 ?）
    else pool = DbPool::Postgres
        Macro->>Render: render(sql, Postgres)
        Render-->>Macro: Cow::Owned("SELECT ... WHERE a = $1 AND b = $2")
        Note over Render: 单遍扫描：跳过 '...'/"..."/--/·* *·<br/>第 k 个 ? → $k；字面量内发现 ? → Err
        Macro->>SQ: query_as::<Postgres, Row>("... $1 ... $2")
        Macro->>SQ: .bind(a).bind(b)   %% 同一 token 顺序
        SQ->>DB: Parse("SELECT ... WHERE a = $1 AND b = $2")
        DB-->>SQ: 结果（PG 只认 $N）
    end
    SQ-->>Biz: Result<Vec<Row>, sqlx::Error>
```

**关键点**：两条路径的 `bind(a).bind(b)` **来自同一处宏展开**，因此不存在"SQL 改对了但 bind 顺序错了"的可能。

---

## 9. 待明确事项

| # | 事项 | 影响 | 建议 |
|---|---|---|---|
| U-1 | **Q1-bis-L2 的实现载体：宏（`db_*!`）而非 builder（`q(dialect).push(..).push_bind(..)`）** | 主理人已实测确认**技术方案可行**（`QueryBuilder` 自动产出 `$1`，自研 rewrite 等价可执行）。剩下的只是**载体形态**：主理人建议的 builder 形态在 sqlx 类型体系下不可编译（bind 值需 `Encode<'_, DB>+Type<DB>`，`DB` 运行时不可知；装箱非对象安全）。宏形态**完全同构于 `push_bind` 语义**（SQL 化 + bind 追加在同一次展开内完成），且已编译验证通过。 | 建议接受宏形态。**§1.3.1 的对照表**完整说明了三种 builder/泛型/trait 尝试各自的编译失败原因。若主理人坚持 builder 形态，则须接受「bind 值全部类型擦除 + 自研 3 后端 × ~12 类型的 `Encode`/`Type` 实现」这一更大代价。 |
| U-2 | **`DateTime<Utc>` → `TIMESTAMPTZ` 推翻 PRD §3.3/§5.0 的「DATETIME → TIMESTAMP」表述** | 影响 57 个列的 PG 类型；若不改，所有 `DateTime<Utc>` 字段在 PG 下**运行期解码失败**（实测 `mismatched types`） | 建议按 `TIMESTAMPTZ` 执行，并在 PRD / db-conventions 中修订该行。**这是本设计最重要的发现**。 |
| U-3 | **`u64` → `i64` 需同时改 MySQL 侧 DDL（`BIGINT UNSIGNED` → `BIGINT`）** | 触及"不编辑已发布 MySQL 迁移"的红线；但项目无生产数据（AGENTS.md 明示），且 V002 是开发期迁移 | 建议：**新增** `V003__align_bigint_signed.sql` 做 `ALTER ... MODIFY`（forward-only，不编辑 V002）；或经主理人确认后允许开发期重建。 |
| U-4 | **`GET_LOCK` → `pg_try_advisory_lock(hashtext(name))` 的哈希冲突** | `hashtext` 返回 int4，不同锁名理论上可哈希碰撞；当前仅 1 个锁名（`iam_outbox_archive`），风险极低 | 建议接受；若未来锁名增多，改用 `pg_try_advisory_lock(:bigint)` 由我方在 Rust 侧算稳定 64 位哈希。 |
| U-5 | **`iam-outbox-publisher` 是否复用 `iam-service` 的 `outbox_archive.rs`** | 两处代码目前**完全重复**（各 10 处 `NOW(3)`） | 建议 S5 顺带抽到 `libs/common-event`（或 iam 的 bin 内共享模块），避免 PG 适配改两遍。属范围微扩，需主理人确认。 |
| U-6 | **`prue_backend` 三轨（mysql/sqlite/postgres）的 `SELECT_FIELDS_*` 常量** | 现有 `mysql_sql`/`sqlite_sql` 成对写法，加 PG 后变三份 → 与"SQL 单份"原则冲突 | 建议：PG 复用 MySQL 分支的 `SELECT_FIELDS_MYSQL`（`CAST(routes_json AS CHAR)` 在 PG 下改成 `routes_json::text`，属 #6 GROUP_CONCAT 同类处置）。S3 落地时验证。 |

---

## 10. 关键设计决策速查

| 决策 | 内容 | 依据 |
|---|---|---|
| 占位符 | 构造期方言化（宏 + `sql::render`），SQL 单份源码 | §1.3，实测双向互斥 |
| 类型绑定 | `DbPool` 枚举决定后端；业务层用宏，禁止穷尽 match | Q6-C，E0004/E0005 规避 |
| 时间列 | PG 一律 `TIMESTAMPTZ(3)`，Rust 保持 `DateTime<Utc>` | D-3 实测 |
| 会话 TZ | `common-db::init_pool` PG 分支统一 `SET TIME ZONE 'UTC'` | Q2-bis |
| `updated_at` | 应用层维护，PG 迁移去 `ON UPDATE` | Q2-A |
| 迁移 | `migrations/postgres/` 新增，版本/marker 与 MySQL 对齐，幂等三重保证 | §4 |
| feature | `postgres` 不进 default，三条独立验证命令 | D-9 |
| 门禁 | S0 先修 `check-db-naming.js`，另加 `check-pg-migrations.js` | P0-6 / §4.3 |
