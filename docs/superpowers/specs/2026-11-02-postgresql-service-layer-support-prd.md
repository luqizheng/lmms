# 服务层全量 PostgreSQL 支持 — 增量 PRD

> 日期：2026-11-02
> 状态：**已裁决**（2026-11-02，主理人裁决 Q1~Q6；Q1-bis / Q2-bis / Q7 / Q8 为补充裁决项）
> 关联：`docs/superpowers/specs/2026-10-30-postgresql-dialect-support-design.md`（基座层，已完成）
> 关联：`docs/superpowers/specs/dialect-matrix.md`（方言矩阵）
> 范围：`services/<7 个服务> + services/iam-outbox-publisher/` + `libs/common-db`、`libs/common-event`、`libs/common-health`、`libs/common` + `.github/workflows/` + `infrastructure/` + `scripts/` + `contracts/` + `docs-site/`
> 风险：**高** — 服务层 SQL 硬编码 MySQL 方言，且 `DbPool` 变体增加会击穿现有不可反驳模式（已实测）
>
> **交付范围：全量 7 服务 + `iam-outbox-publisher`**（需求方已选定）。本文档的 **P0/P1 是「实施顺序」而非「交付分期」**——P1 项属本次交付范围，不另起一期。§9 的 S0~S6 是**提交切分**（一个切片一个 commit），非交付分期。

---

## 0. 结论先行

| 结论 | 内容 |
|---|---|
| 基座层可复用 | `libs/common-db` 的 PG 方言实现已就绪，`cargo check -p digua_common_db --features postgres` **实测通过**（22.89s，2 个 unused warning）。服务层不需要重做方言层。 |
| 真正的瓶颈不是 PG 语法，是**类型绑定** | 7 个服务中 6 个直接依赖 `sqlx::mysql::MySqlPool`；`MySqlPool`(124) + `MySqlPoolOptions`(21) = **145 处**硬编码（services + libs），分布在 30+ 文件。**PG 支持 = 把 MySQL 类型绑定泛型化，不是加一份 SQL 文件。** |
| ⚠️ **占位符不可逐条改成 `$N`** | 实测（真实 MySQL 8.0 + PG 16 容器）：**MySQL 把 `$1` 解析为列名**（`ERROR 1054 Unknown column '$1'`），**PG 把 `?` 解析为 jsonb 操作符**（`syntax error at or near "?"`）。sqlx-core 中 `format_placeholder` 的**唯一调用方是 `QueryBuilder`**，MySQL/SQLite 走默认实现（输出 `?`），仅 PG 覆写为 `$N`。**故「把源码里的 `?` 逐条改成 `$N`」会直接打断 MySQL 路径**——这是硬约束，不是偏好问题。→ 见 §7 Q1。 |
| 工作量集中在 3 个点 | ① 占位符跨方言表达（**309 个查询调用点**，其中 **226 个**含 `?` 需适配，见 §5.1 口径说明）；② MySQL 专有 DDL/DML 语法（`ENGINE=`/内联 `INDEX`/`UNIQUE KEY`/`TINYINT(1)`/`DATETIME`/`AUTO_INCREMENT`/`ON UPDATE CURRENT_TIMESTAMP`/`ON DUPLICATE KEY UPDATE`/`GET_LOCK`/`GROUP_CONCAT` 等）；③ `DbPool` 变体增加引发**编译期不可反驳模式破裂**（已用 rustc 最小复现验证 E0005/E0004）。 |
| ⚠️ 门禁脚本会误伤 PG 语法 | `scripts/check-db-naming.js` 对 `ON CONFLICT ... DO UPDATE SET` 会把 `SET` 误判为裸表名并报错。**新增 PG 迁移前必须先修门禁**，否则 P0 直接卡死。已用脚本实测复现。 |
| 建议分批 | **P0 = 3 个低风险服务**（`prue_backend` / `config-service` / `file-service`）跑通全链路，验证方言抽象形态；**P1 = 其余 4 个**（`audit` / `token` / `notification` / `iam`）+ `iam-outbox-publisher`。`iam-service` 因 `GET_LOCK` 语义 + `GROUP_CONCAT` + `information_schema.statistics` 三重耦合，放最后。**（P0/P1 = 实施顺序，交付范围仍为全量）** |
| 占位符策略推荐 | **方言感知的 SQL 构造辅助层**（在 `libs/common-db` 提供，服务侧以 `q()`/`QueryBuilder` 风格调用），使**同一份 SQL 源码**在两种后端下正确产出 `?` 或 `$N`。理由见 §7 Q1。 |

### 0.0 裁决记录（2026-11-02，主理人）

| Q | 议题 | **裁决** | 裁决理由 |
|---|---|---|---|
| Q1 | 占位符适配策略 | **A — 方言感知的 SQL 构造辅助层** ✅ 采纳推荐 | 唯一同时满足「SQL 单份」+「零散改」；`QueryBuilder` 已有方言化先例 |
| Q1-bis | 辅助层落点 | **L2 — 库层 trait 上的 `query`/`query_as`** | 与「`MySqlPool` 泛型化」落在同一层，可合并为一个 commit |
| Q2 | `ON UPDATE CURRENT_TIMESTAMP` 替代 | **A — 应用层维护** ✅ 采纳推荐 | `split(';')` 与 PG `$$` 不兼容是客观约束 |
| Q2-bis | PG 会话时区 | **A — 库层统一 `SET TIME ZONE 'UTC'`** | 静默偏移风险，等级同占位符 |
| Q3 | 交付范围 | **B — 全量 7 服务 + `iam-outbox-publisher`** ⬅️ 覆盖原推荐 | 需求方已明确选定「服务层**全量** PG 支持」；**提交粒度仍按 S0~S6 切片，一个切片一个 commit** |
| Q4 | testcontainers PG 覆盖范围 | **A — 仅 P0 服务 + 共享库** ✅ 采纳推荐 | fixture 形态未定就铺 7 份放大返工面 |
| Q5 | `common-event` outbox 泛型化形态 | **A — trait 泛型化** ✅ 采纳推荐 | 与 `DbPool` 枚举风格一致 |
| Q6 | `MySqlPool` 类型绑定改造形态 | **C — 混合（`DbPool` 枚举 + 库层重写）** ✅ 采纳推荐 | 枚举定后端 + 重写层保 SQL 单份 |
| Q7 | `DEFAULT CURRENT_TIMESTAMP(3)` 写法 | **A — 保持 `CURRENT_TIMESTAMP(3)`，列类型用 `TIMESTAMP(3)`** | PG 16 实测**接受**精度参数（详见 §7 Q7 的实测纠正） |
| Q8 | 本地 PG 编排入口 | **A — `shared-db.js` 增 `ensurePostgres` + compose 可选 PG service** | 与既有 `ensureMysql` 惯例一致，改动集中在一个文件 |

> **Q3 变更说明**：本文档 §3 的 P0/P1 分级从「交付批次」降级为「**实施顺序**」。P0 项仍是最先做（库层抽象 + 门禁 + P0 三服务打样），但**最终交付范围是全部 7 服务 + `iam-outbox-publisher`**，P1 项在本次交付内必须完成，不是「另起一期」。§9 相应表述已同步修订。

### 0.1 关键约束的实证结论（用真实容器验证）

> 验证方式：启动 `mysql:8.0` 与 `postgres:16` 临时容器，直接执行 SQL 探针；测试容器已清理。

| 探针 | 实测结果 | 结论 |
|---|---|---|
| MySQL `PREPARE ... SELECT ?` | ✅ 成功返回 `1` | MySQL 只认 `?` |
| MySQL `PREPARE ... SELECT $1` | ❌ `ERROR 1054 Unknown column '$1' in 'field list'` | **`$N` 在 MySQL 是列名，不是占位符** |
| PG `PREPARE ... SELECT ?` | ❌ `ERROR syntax error at or near "?"` | `?` 在 PG 是 jsonb 存在性操作符 |
| PG `PREPARE ... SELECT $1` | ✅ 成功返回 `42` | PG 只认 `$N` |
| PG `CREATE TABLE ... a INT AUTO_INCREMENT` | ❌ syntax error | 须 `GENERATED ALWAYS AS IDENTITY` |
| PG `CREATE TABLE ... a DATETIME` | ❌ `type "datetime" does not exist` | 须 `TIMESTAMP` |
| PG `CREATE TABLE ... a TINYINT(1)` | ❌ `type "tinyint" does not exist` | 须 `BOOLEAN` |
| PG `CREATE TABLE ... ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4` | ❌ syntax error | 整段删除 |
| PG `CREATE TABLE ... UNIQUE KEY uk_x (a)` | ❌ syntax error | 须独立 `CREATE UNIQUE INDEX` |
| PG `CREATE TABLE ... INDEX idx_x (a)` | ❌ `type "idx_x" does not exist` | 须独立 `CREATE INDEX` |
| PG `CREATE TABLE ... b INT ON UPDATE CURRENT_TIMESTAMP` | ❌ syntax error | 须触发器或应用层维护 |
| PG `CREATE TABLE ... a JSON NOT NULL` | ✅ **接受** | PG 有原生 `JSON` 类型，非必须 `JSONB` |
| PG `CREATE TABLE ... a TIMESTAMP(3) DEFAULT CURRENT_TIMESTAMP(3)` | ✅ **建表成功**，精度按 3 位截断 | **PG 16 接受** `CURRENT_TIMESTAMP(3)`，非语法错误（见 §7 Q7） |
| PG `INSERT ... ON CONFLICT (id) DO UPDATE SET v=2` | ✅ 成功 | 冲突目标**必填** |
| PG `SELECT STRING_AGG(x::text, ',')` | ✅ 返回 `1,2` | 替代 `GROUP_CONCAT` |
| PG `information_schema.tables WHERE table_schema = current_schema()` | ✅ 正确返回 `1` | 替代 `DATABASE()` |
| **PG 会话 TZ = `Asia/Shanghai` 时，naive `TIMESTAMP` 列写入 `CURRENT_TIMESTAMP(3)`** | ⚠️ 存 **19:14**（本地），UTC 期望 **11:14** | ⚠️ **偏移 +08 小时且不报错** |
| **PG 会话 TZ = `Asia/Shanghai` 时，写入 `(now() AT TIME ZONE 'UTC')`** | ✅ 存 **11:14**（UTC） | ✅ **唯一 TZ 无关的写法**（基座层 `now_expr()` 语义） |

---

## 1. 产品目标

### 1.1 要解决的问题

当前基座层已把 PostgreSQL 列为正式方言（`dialect-matrix.md` 有 PG 行，`DialectKind::Postgres` 已实现，`postgres` feature 已提供），但**服务层对 PG 的实际支持度为零**：

| 现状事实（实测） | 影响 |
|---|---|
| 7 个服务均无 `migrations/postgres/` 目录，仅 `migrations/mysql/`（`prue_backend` 另有 `sqlite/`） | PG 上无法建表，服务无法启动 |
| 各服务 `Cargo.toml` 的 sqlx features 仅 `mysql`（`prue_backend` 为 `mysql,sqlite`）；`digua_common_db` 依赖项 features 同样未开 `postgres` | 链接期没有 PG 驱动，`DbPool::Postgres` 分支不可达 |
| `services/**` 内 `sqlx::` 引用 **443 处**；其中**查询构造点 309 处**（`iam-service` 163 / `notification-service` 53 / `token-service` 28 / `prue_backend` 24 / `audit-service` 18 / `config-service` 8 / `file-service` 8 / `iam-outbox-publisher` 7），真实 `?` 占位符 **102 处** | SQL 全部是 MySQL 方言；PG 下占位符与专有语法均失效 |
| 6/7 服务直接依赖 `sqlx::mysql::MySqlPool`（`MySqlPool`(124) + `MySqlPoolOptions`(21) = **145 处**硬编码（services + libs）），未走 `DbPool` 抽象 | 「换驱动」不是改 SQL，是改类型签名 |
| `libs/common-event`（`MySqlOutboxRepo`）、`libs/common-health`（`DbProbe::MySql`）、`libs/common`（`sqlx/mysql`）、`libs/common/src/db.rs::connect()` 硬返回 `MySqlPool`（**被 audit/iam 主流程与 iam 测试调用**） | 共享库先于服务成为阻塞点 |
| `dialect-matrix.yml` 的 `test-services` job 只跑 MySQL（audit/config/file 三包） | PG 服务集成测试无 CI 载体 |
| `infrastructure/` 下 7 个 compose 文件**零 postgres 引用**；`scripts/lib/shared-db.js` 的 `ensureMysql` 硬编码 `coder-test-mysql` 容器 | 本地/CI 无 PG 实例可连 |
| 服务级 `AGENTS.md` 的「数据库方言」小节写「支持 MySQL 和 SQLite」 | 文档与实现漂移 |

### 1.2 成功的判据

**必须全部满足**（可执行、可验证，详见 §6）：

1. **G1 编译门禁**：7 个服务在 `postgres` feature 开启下 `cargo check` 全绿，且 **MySQL/SQLite 既有构建零回归**（`postgres` 不进 default features，默认构建产物不变）。
2. **G2 迁移门禁**：7 个服务各有 `migrations/postgres/`，文件数与 MySQL 侧一一对应，PG 容器内 `ensure_schema` 幂等通过（重复启动不报错）。
3. **G3 行为门禁**：P0 服务的核心业务用例（登录/令牌/配置读写/文件读写/菜单）在 PG 上跑通，MySQL 侧结果一致。
4. **G4 门禁脚本门禁**：`node scripts/check-db-naming.js` 对新增 PG 迁移**零误报**（当前会误报 `SET`，必须先修）。
5. **G5 CI 门禁**：新增 PG 服务集成 job 在 PR 级真实跑过一次并通过。
6. **G6 文档门禁**：7 个服务 `AGENTS.md` 的「数据库方言」小节与 `dialect-matrix.md` 同步更新。

---

## 2. 用户故事

| # | 角色 | 故事 | 验收标准 |
|---|---|---|---|
| US-1 | **框架使用者**（下游服务开发者） | 作为使用 `digua_common_db` 的下游开发者，我希望在同一份业务代码里切换 MySQL / PostgreSQL 后端，以便把服务部署到已有 PG 集群（云厂商 RDS / 自建 PG）而不必为 PG 重写一遍 | ① 服务仅改 `DATABASE_URL` 前缀（`postgres://`）即可启动并完成 `ensure_schema`；② 业务 SQL 源码**不因方言分叉而重复**（方言差异收敛在库层或迁移目录内）；③ MySQL 后端行为零变化 |
| US-2 | **下游服务开发者** | 作为新增服务的开发者，我希望有一份「PG 迁移怎么写」的现成范式，以便新服务默认同时支持 MySQL 与 PG | ① `contracts/db-conventions.md` 增补 PG 类型映射章节（AUTO_INCREMENT→IDENTITY、TINYINT(1)→BOOLEAN、DATETIME→TIMESTAMP、UNSIGNED→无对应、内联 `INDEX`/`UNIQUE KEY`→独立索引、`COMMENT`→`COMMENT ON`、`ENGINE=` 删除、`JSON`/`JSONB` 取舍等）；② 每个服务 `AGENTS.md`「数据库方言」小节给出 PG 支持声明与迁移目录约定；③ 迁移编写者能按对照表把 MySQL DDL 逐条翻译，无需查 PG 文档 |
| US-3 | **运维** | 作为运维，我希望用同一套编排起 MySQL 与 PG 两套环境，以便按客户要求选型并做对比验证 | ① `infrastructure/docker-compose-infra.dev.yml` 提供可选的 `postgres` service（profile 隔离，默认不启动）；② `TEST_PG_URL` 缺省时 PG 相关测试**跳过而非失败**（与现有 MySQL 行为一致）；③ CI 提供 PG service 容器与健康检查 |
| US-4 | **运维** | 作为运维，我希望一眼看出某个服务在哪些后端上被验证过，以便判断上线置信度 | ① `dialect-matrix.md` 的「测试矩阵」表按服务 × 方言列出覆盖状态（✅/🟡/⚠️）；② 每个服务的迁移状态端点（`/iam/internal/migrations/state` 等）返回的 `dialect` 字段准确反映当前后端 |
| US-5 | **质量工程师** | 作为 QA，我希望同一份集成测试能对 MySQL 与 PG 各跑一遍，以便用最小成本发现方言回归 | ① `tests/common/mod.rs` 提供后端参数化的容器 fixture（MySQL / PG 双分支）；② 同一测试文件对两种后端参数化执行，断言结果一致；③ PG 侧失败时 CI 明确标红为 PG 专属失败 |
| US-6 | **架构评审者** | 作为评审者，我希望看到方言差异在**哪些层**被解决，以便判断是否引入了不可维护的分叉 | ① 差异点逐条归属到「库层抽象 / 迁移目录 / 应用层分支」三类之一；② 迁移目录（DDL 差异）与运行时 SQL（函数/语法差异）分离，不混在同一次改动里；③ 存在明确的后门：若某条 SQL 无法跨方言，用 `dialect` 分支显式表达并留注释说明原因 |

---

## 3. 需求池

> **⚠️ P0/P1/P2 的语义（务必先读）**：本次**交付范围为全量**（7 个服务 + `iam-outbox-publisher`，见 §7 Q3 裁决 B）。以下分级表示**实施先后顺序**，**不表示交付分期**——P1 与 P2 中列入交付范围的项目，**必须在本次交付内完成**，不另起一期。分级的唯一用途是**排序**：先做能验证抽象形态、风险最低的部分，用以控制全量交付的返工风险。

### 3.1 P0 — 核心链路能在 PG 上跑通（**最先实施**，阻塞后续铺开）

> P0 的定义边界：**证明方言抽象形态成立**。不追求全服务覆盖，但必须端到端打通「建表 → 写入 → 查询 → 迁移幂等 → 测试」。

| ID | 需求 | 归属层 | 验收 |
|---|---|---|---|
| P0-1 | `libs/common-db` 提供**方言感知的查询构造层**（Q1-bis-L2：库层 trait 上的 `query`/`query_as`/`query_scalar`），统一处理占位符（`?` vs `$N`）、`now()` 表达式、标识符转义、分页 | 库层 | 见 §7 Q1/Q1-bis；**MySQL 路径行为与今日逐字节一致**；PG 路径产出 `$1..$N` 且 bind 顺序严格对齐 |
| P0-1b | PG 连接建立后统一 `SET TIME ZONE 'UTC'`（Q2-bis），与基座层 `now_expr()` 的 UTC 语义对齐 | 库层 | 写入 naive `TIMESTAMP` 的值在非 UTC 会话下不偏移（容器实证） |
| P0-2 | `libs/common-db` 增加 **PG marker 表探测**（`information_schema` + `current_schema()`），与既有 `probe_marker_mysql` / `probe_marker_sqlite` 并列 | 库层 | `migration_state.rs` 三个探测函数并存，PG 分支返回正确布尔（实测 `current_schema()` 探测返回 `1`） |
| P0-3 | `libs/common-event` 的 `MySqlOutboxRepo` 泛型化（`OutboxRepo` trait 不绑死 `MySql`），支持 PG outbox | 库层 | `iam-service` / `notification-service` / `iam-outbox-publisher` 的 outbox 写入在 PG 上成功；`outbox.rs:146` 的 `CURRENT_TIMESTAMP(3)` 适配 |
| P0-4 | `libs/common-health` 的 `DbProbe` 枚举增加 `Postgres` 臂（`features = ["any", ...]` 已含 `any`） | 库层 | `/health` 的 DB 探针对 PG 返回 healthy |
| P0-5 | `libs/common` 门面 crate 的 sqlx features 解除 mysql-only 绑定；**`db.rs::connect()` 改为返回方言无关的 `DbPool`**（当前硬返回 `MySqlPool`，被 audit/iam 主流程与 iam 测试调用） | 库层 | `digua_common` 在 `postgres` feature 下可编译；`connect()` 对 `postgres://` URL 返回 `DbPool::Postgres` |
| P0-6 | **修复 `scripts/check-db-naming.js` 对 PG 语法的误报**：`ON CONFLICT ... DO UPDATE SET` 中的 `SET` 被当作表名；`pg_catalog` / `current_schema` 需入白名单；顺带修正 `VALID_PREFIXES` 的双逗号 | 门禁 | 用 PG 语料（含 `ON CONFLICT DO UPDATE SET`、`DO NOTHING`、`pg_catalog.pg_indexes`）跑该脚本，**零误报** |
| P0-7 | 3 个 P0 服务的 `migrations/postgres/` 迁移集（`prue_backend` / `config-service` / `file-service`） | 迁移 | 文件数与 MySQL 侧一致；PG 容器内建表成功且字段语义等价 |
| P0-8 | P0 服务的 `sqlx` + `digua_common_db` features 开启 `postgres`（**不进 default features**） | 特性 | `cargo tree` 可见 `sqlx/postgres`；默认构建产物不变 |
| P0-9 | P0 服务的 `MySqlPool` 类型绑定 → 方言无关抽象（`DbPool` 或泛型 `Executor`） | 应用层 | P0 三个服务在 MySQL / PG 两种后端下均编译通过 |
| P0-10 | P0 服务的集成测试 fixture 支持 PG 容器（`testcontainers-modules` 的 `postgres` feature 已确认存在于 0.8.0） | 测试 | P0 服务集成测试对 PG 后端跑通；无 `TEST_PG_URL` 时跳过 |
| P0-11 | CI 新增 `test-services-postgres` job（postgres:16 service container，PR 级） | CI | PR 上真实跑过一次并通过 |

### 3.2 P1 — 其余服务覆盖 + 生产就绪（**本次交付范围内**，P0 之后实施）

| ID | 需求 | 延后理由 |
|---|---|---|
| P1-1 | `audit-service` / `token-service` / `notification-service` 的 `migrations/postgres/` | 依赖 P0-1~P0-6 的形态验证，方言写法需先定型 |
| P1-2 | 上述三服务的 `MySqlPool` 泛型化 + `?`→`$N` 占位符适配 | 同上 |
| P1-3 | `token-service` 的 `DbType` 枚举增加 `postgres` 变体（当前 `config.rs` 仅 `Mysql`，未知 db_type 被静默忽略） | 涉及配置语义变更，需独立 spec |
| P1-4 | `iam-service` PG 迁移集（8 个迁移文件，**全服务最多**） | 迁移量最大，且叠加 `GET_LOCK` / `GROUP_CONCAT` / `information_schema.statistics` 三重 MySQL 专有依赖 |
| P1-5 | `iam-service` 的 `GET_LOCK`/`RELEASE_LOCK` → PG `pg_advisory_lock`（PG 原生 advisory lock，sqlx-postgres 0.8.6 已内置 `PgAdvisoryLock`） | 语义敏感（session-scoped vs 事务级），需专门设计 |
| P1-6 | `iam-service` 的 `GROUP_CONCAT(ur.role_id)` → PG `STRING_AGG`（`internal_membership.rs` 消费为逗号分隔字符串） | 输出格式需与下游消费者对齐 |
| P1-7 | `infrastructure/` compose 增加可选 `postgres` service（profile 隔离）+ `scripts/lib/shared-db.js` 增加 `ensurePostgres` | 生产编排属发布范畴，与代码解耦 |
| P1-8 | 7 个服务 `AGENTS.md`「数据库方言」小节同步 | 依赖最终支持矩阵定稿 |
| P1-9 | `dialect-matrix.md` 增补 PG 差异列 + 按服务×方言测试矩阵 | 依赖 P1 全部完成 |
| P1-10 | `contracts/db-conventions.md` 增补 PG 类型映射章节 | 依赖 P0-7 迁移写法定型 |
| P1-11 | `docs-site/guides/testing.md` 增补 PG 测试容器用法 | 依赖 P1-7 编排落地 |
| P1-12 | 删除 `token-service` 自建的重复 `DbPool` 枚举，统一复用 `digua_common_db::DbPool` | 依赖 Q6 裁决 |
| P1-13 | `iam-outbox-publisher` 独立二进制的 PG 适配（`MySqlPool` 泛型化 + `GET_LOCK`→`pg_advisory_lock` + `iam_outbox_events` 直写） | **本次交付范围内**（Q3 裁决 B）；运维工具链路，但与 `iam-service` 共用 `common-event` outbox trait，需随 P1-4 一同落地 |

### 3.3 P2 — 增强与长期项（**非本次交付必需**，可后续迭代）

> ⚠️ 本节条目**不属于本次交付范围**（与 P1 不同）。列出仅为记录已识别的技术债与增强点，供后续排期参考。

| ID | 需求 | 延后理由 |
|---|---|---|
| P2-1 | `iam-service/tests/*` 中 `JSON_UNQUOTE(JSON_EXTRACT(...))` → PG `->>` 运算符 | 仅测试断言代码，生产 SQL 未用该组合 |
| P2-2 | `notification-service` 的 `JSON_CONTAINS` 路径 → PG `jsonb_path` / `@>` | 生产查询未实际调用（仅迁移注释提及） |
| P2-3 | `MySQL TINYINT(1)` 列 → PG `BOOLEAN` 的**跨端一致性**改造（当前 `TINYINT(1)` 在 sqlx-mysql 解码为 `i8`，非 `bool`） | 属既有语义债，与 PG 支持正交 |
| P2-4 | `DATETIME` → `TIMESTAMP` 列类型的跨端时间语义对齐（含时区） | 57 处 `DATETIME`，改动面大且触及既有业务语义 |
| P2-5 | `UNSIGNED` → PG 无对应类型（改 `CHECK` 约束或 `BIGINT`） | 10 处，需与业务确认取值上界 |
| P2-6 | PG 特有能力（JSONB 索引、GIN、`GENERATED ALWAYS AS IDENTITY` 优化） | 非「支持 PG」必需，属增强 |

---

## 4. 范围与非目标

### 4.1 范围（In Scope）

- **全部 7 个服务 + `iam-outbox-publisher`** 的 PG 迁移集、feature 开关、类型泛化、SQL 方言适配（Q3 裁决 B：全量交付）。
- 4 个共享库（`common-db` / `common-event` / `common-health` / `common`）的 PG 打通。
- 门禁脚本（`check-db-naming.js`）对 PG 语法的兼容修复。
- CI（`dialect-matrix.yml`）新增 PG 服务集成 job。
- `infrastructure/` compose 的可选 PG service（profile 隔离）。
- 服务级 `AGENTS.md` + `contracts/db-conventions.md` + `dialect-matrix.md` 文档同步。

### 4.2 非目标（Out of Scope，明确不做）

| 非目标 | 理由 |
|---|---|
| ❌ **不引入 ORM**（SeaORM / Diesel） | 会重写全部 309 个查询构造点 + 226 个占位符适配点的数据访问层，远超「支持 PG」范畴，且引入新的架构债 |
| ❌ **不做 MySQL → PG 的数据迁移工具** | 项目**无生产数据**（AGENTS.md 明示），可整体重建 schema |
| ❌ **不支持 PG 特有高级特性**（FTS / 分区表 / 逻辑复制 / `COPY`） | 超出「支持 PG」最小集；PG 高级能力属 P2 增强 |
| ❌ **不改 api-gateway 的无 DB 设计** | api-gateway 无 DB 依赖（仅 Redis + Consul），保持零 DB |
| ❌ **不把 `postgres` 放进 default features** | 硬约束：保持 default 构建产物零变化 |
| ❌ **不重写 `libs/common-db` 的方言层** | 基座层已完成且 `cargo check --features postgres` 实测通过，重复实现是浪费 |
| ❌ **不改 OpenAPI 契约** | PG 支持不改变 API 形状；`services/*/openapi.yaml` 仍由 `dump-openapi-specs.js` 自动生成，**禁止手改** |
| ❌ **不做跨方言的运行时 SQL 自动翻译器** | 见 §7 Q1，运行时仅做**占位符**与**受控表达式**的方言化，复杂 DDL 差异走迁移目录 |
| ❌ **不改达梦 / MSSQL 的服务层支持状态** | 二者仍是基座层预留，服务层未开工；本次只做 PG |
| ❌ **不编辑任何已发布的 MySQL 迁移文件** | AGENTS.md 强制 forward-only；PG 侧是**新增目录**，非修改 |

---

## 5. 兼容性矩阵

> 数据来源：本地实测（`grep` 全量统计 + 逐文件核对 + 脚本模拟 + rustc 最小复现 + 真实 MySQL 8.0 / PG 16 容器 SQL 探针）。迁移文件数为 `migrations/mysql/` 实际 `.sql` 文件数。

### 5.0 DDL 语法差异全量清单（PG 侧逐条实测）

> 「PG 实测」列由真实 PG 16 容器执行 `CREATE TABLE` 探针得出，非文档推断。

| MySQL 语法 | 出现次数 | PG 实测 | PG 写法 |
|---|---|---|---|
| 列注释 `COMMENT '...'` | 57 | ❌ 语法错误 | `COMMENT ON COLUMN t.c IS '...'` 或丢弃 |
| 表尾 `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=...` | 18 | ❌ 语法错误 | 整段删除 |
| 内联 `INDEX idx_x (...)` | 26 | ❌ `type "idx_x" does not exist` | 独立 `CREATE INDEX idx_x ON t (...)` |
| 内联 `UNIQUE KEY uk_x (...)` | 10 | ❌ 语法错误 | 独立 `CREATE UNIQUE INDEX uk_x ON t (...)` |
| `TINYINT(1)`（表示布尔） | 5 | ❌ `type "tinyint" does not exist` | `BOOLEAN` |
| `DATETIME` / `DATETIME(3)` | 57 | ❌ `type "datetime" does not exist` | `TIMESTAMP` / `TIMESTAMP(3)` |
| `DEFAULT CURRENT_TIMESTAMP(3)` | 9 | ✅ **接受并按 3 位截断**（`TIMESTAMP(3)` 列上实测 `11:10:28.735`） | 保持 `CURRENT_TIMESTAMP(3)`，列类型用 `TIMESTAMP(3)`；**但必须配合会话 TZ 锁 UTC**（见 §7 Q7） |
| `AUTO_INCREMENT` | 9 | ❌ 语法错误 | `GENERATED ALWAYS AS IDENTITY`（基座层 `dialect::pg::identity::identity_column` 已提供） |
| `JSON` 列型 | 8 | ✅ **接受**（PG 有原生 `JSON`） | 可用 `JSON`；若需索引/去重键序则用 `JSONB` |
| `ON UPDATE CURRENT_TIMESTAMP` | 2 | ❌ 语法错误 | 触发器或应用层维护（见 §7 Q2） |
| `NOW()` | 171（rs 161 + sql 10） | ✅ PG 有 `NOW()` | **但 PG 返回 `timestamptz`，MySQL 返回 naive datetime** → 会话 TZ 必须锁 UTC |
| `ON DUPLICATE KEY UPDATE` | 14 | ❌ 语法错误 | `ON CONFLICT (冲突目标列) DO UPDATE SET ...`（**冲突目标必填**，实测） |
| `GROUP_CONCAT(x)` | 2 | ❌ 函数不存在 | `STRING_AGG(x::text, ',')`（实测返回 `1,2`） |
| `information_schema.tables WHERE table_schema = DATABASE()` | 15 | ❌ `DATABASE()` 不存在 | `table_schema = current_schema()`（实测正确返回 `1`） |
| `GET_LOCK(name, 0)` / `RELEASE_LOCK(name)` | 11 + 9 | ❌ 函数不存在 | `pg_try_advisory_lock(hashtext(name))` / `pg_advisory_unlock(...)`（PG 原生，sqlx-postgres 0.8.6 亦内置 `PgAdvisoryLock`） |
| `INSERT IGNORE` | 18 | ❌ 语法错误 | `INSERT ... ON CONFLICT DO NOTHING` |
| `UNSIGNED` | 10 | ❌ 类型不存在 | `BIGINT` + `CHECK (col >= 0)` |
| `FOR UPDATE SKIP LOCKED` | — | ✅ PG 支持 | 无差异 |

### 5.1 服务级矩阵

> **统计口径说明（务必先读，避免误估改造量）**：
> - **查询调用点** = `sqlx::query` / `query_as` / `query_scalar` 的**调用处数量**（一处调用算 1 点），跨行 SQL 字符串按**一处**计。
> - **需占位符适配点** = 该调用点的 SQL 文本中**含至少一个 `?`** 的数量——**这才是占位符改造的实际工作量**。
> - **`?` 字符数** = SQL 文本中 `?` 的总个数（仅供了解参数规模，**不代表工作量**）。
> - ⚠️ 早前版本把 prue_backend 标为 44，那是 **`?` 字符数**而非调用点数，会高估改造量——已按上述口径统一。
> - ⚠️ 单行 grep 会**漏计跨行 SQL**（本仓库多数 SQL 为跨行字符串字面量），故 config/file 实为各 4 点而非 0 点。

| 服务 | MySQL 迁移数 | PG 迁移数（目标） | 查询调用点 | **需占位符适配点** | `?` 字符数 | MySQL 类型硬引用 | mysql 迁移行数 | 关键方言差异点 | 风险 | 风险来源 |
|---|---|---|---|---|---|---|---|---|---|---|
| `prue_backend` | 1 (+sqlite 1) | 1 | 24 | **17** | 46 | 0 ✅ | 20 | `ON UPDATE CURRENT_TIMESTAMP`(1)、`JSON`、`DATETIME`、`TINYINT(1)`、`information_schema` + `DATABASE()` | **中** | 已有 `DbPool` 双方言分支骨架（`menu_config_svc.rs` 的 mysql_sql/sqlite_sql 成对写法），是全仓**最成熟的改造模板**；占位符密度最高（17/24） |
| `config-service` | 2 | 2 | 8 | **4** | 22 | 22 ❌ | 56 | `DATETIME`、`TINYINT(1)`、`UNIQUE KEY`→独立 `CREATE UNIQUE INDEX`、`information_schema` | **低** | 占位符极少（4/8）+ 已有 `QueryBuilder`（`push_bind` 自动方言化，全仓 44 处 `QueryBuilder::new(`）；主要工作是 `MySqlPool` 类型签名改造 |
| `file-service` | 2 | 2 | 8 | **4** | 23 | 22 ❌ | 39 | `DATETIME`、`TINYINT(1)`、`information_schema` | **低** | 同 config-service |
| `audit-service` | 1 | 1 | 18 | **12** | 21 | 17 ❌ | 66 | `ON DUPLICATE KEY UPDATE`(3)、`JSON`、`UNIQUE KEY`、`COMMENT '(24 处内联)`、Rust 侧 `?` | **中** | 内联 `COMMENT` 语法 PG 不支持（需 `COMMENT ON COLUMN`）；`ON DUPLICATE KEY` → `ON CONFLICT`；且是 `common-event` 消费者 |
| `token-service` | 2 | 2 | 28 | **19** | 50 | 16 ❌ | 62 | **自带 `DbPool` 枚举**（`db.rs:41`，非复用 `common-db`）、`ON UPDATE CURRENT_TIMESTAMP`(1)、`JSON`、`DbType` 枚举仅 mysql、`after_connect` 强制 `SET time_zone='+00:00'` | **中高** | 自定义 `DbPool` 需独立扩展；`ON UPDATE` 语义依赖 `token_oauth_clients.updated_at`；`config.rs` 对未知 db_type 静默忽略，需改 |
| `notification-service` | 3 | 3 | 53 | **41** | 79 | 21 ❌ | 107 | `ON DUPLICATE KEY UPDATE`(2)、`JSON`、`DATETIME`、`UNIQUE KEY`、`COMMENT`、outbox（`common-event`） | **中高** | 占位符 41/53 + 依赖 `common-event` PG 化 + 独立 `outbox_publisher` 二进制 |
| `iam-service` | **8** | 8 | **163** | **122** | 305 | 26 ❌ | **331** | `GET_LOCK`/`RELEASE_LOCK`(11+9=20，session-scoped 分布式锁)、`GROUP_CONCAT`(2)、`ON DUPLICATE KEY UPDATE`(8)、`DATETIME`、`UNIQUE KEY`(6)、`information_schema.statistics`(PG 无此视图)、outbox | **高** | 全仓最重：迁移数最多(8)、SQL 量最大(163)、占位符适配点最多(122)、迁移行数最多(331)、MySQL 专有函数最密集。`GET_LOCK` 语义（session 级、跨连接泄漏 8 小时）在 PG 需换 `pg_advisory_lock` |
| `iam-outbox-publisher` | — | — | 7 | **7** | 15 | MySQL 类型硬编码 | — | `GET_LOCK`/`RELEASE_LOCK`、`iam_outbox_events` 直写 | **中** | 非在线链路，属运维工具；**本次交付范围内**（P2-7 调整为 P1） |
| **合计** | **19** | **19** | **309** | **226** | **561** | **145** | **681** | — | — | — |

### 5.2 跨服务共享阻塞点

| 共享单元 | 现状 | 阻塞的服务 | 需求 |
|---|---|---|---|
| `libs/common-db` | ✅ PG 已就绪（实测编译通过） | — | P0-1 / P0-2 |
| `libs/common-event` | `MySqlOutboxRepo` 硬绑 `MySqlPool` + `Transaction<'_, MySql>`；`outbox.rs:146` 用 `CURRENT_TIMESTAMP(3)` | iam / notification / audit / iam-outbox-publisher | P0-3 |
| `libs/common-health` | `DbProbe::{MySql, Sqlite}`，无 Postgres 臂 | 全部 | P0-4 |
| `libs/common` | `sqlx = { features = ["mysql"] }` + `digua_common_db features=["mysql"]`；**`db.rs::connect()` 硬返回 `MySqlPool`**，被 audit/iam 主流程与 iam 测试调用 | 全部 | P0-5 |
| `libs/common-db` 的 `migration_state.rs` | `probe_marker_mysql` / `probe_marker_sqlite` 两函数 | 全部 | P0-2 |
| `libs/common-db` 的 `lib.rs::run_migrations` | 对**所有方言**均返回 `DB_MIGRATE`（"runtime migration path not supported"）→ 运行期路径迁移未实现，各服务靠自己的 `ensure_schema` | 全部 | 见 §9 说明 |
| `libs/common-db/src/test_kit/sqlite_fixture.rs` | 现成方言 fixture 参考 | 全部 | P0-10 |

### 5.3 已识别的编译期破坏点（实测验证）

| 破坏点 | 位置 | 机制 | 实测证据 |
|---|---|---|---|
| **不可反驳模式破裂** | `token-service/src/db.rs:80`、`routes/migration_state.rs:69`、`tests/migration_marker_test.rs:44`（`let DbPool::Mysql(p) = pool;`） | `DbPool` 从 2 变体增至 3 变体后，`let` 模式变为可反驳 → **E0005** | rustc 最小复现：`error[E0005]: refutable pattern in local binding` |
| **非穷尽匹配破裂** | `prue_backend/src/service/menu_config_svc.rs`（20 处 `DbPool::Mysql/Sqlite` match 分支）、`routes/migration_state.rs:41-42` | 新增 `Postgres` 变体 → **E0004** | rustc 最小复现：`error[E0004]: non-exhaustive patterns: DbPool3::Postgres(_) not covered` |
| **门禁脚本误报** | `scripts/check-db-naming.js` | `TABLE_PATTERNS` 的 `UPDATE\s+([a-z_]\w*)` 会把 PG 的 `DO UPDATE SET` 中的 `SET` 当作表名 | 脚本模拟实测：`❌ VIOLATION \| PG upsert DO UPDATE SET -> [{"t":"SET"}]`；同时 `pg_catalog` 亦被捕获 |
| **占位符不兼容（双向）** | 全部 309 个 `sqlx::query` 查询点，其中 **226 个**含 `?` 需适配 | MySQL 只认 `?`；PG 只认 `$N`。sqlx-core 的 `format_placeholder` **唯一调用方是 `QueryBuilder`**，MySQL/SQLite 用默认实现（输出 `?`），仅 PG 覆写为 `$N` | **容器实证**：MySQL `SELECT ?`✅ / `SELECT $1`❌（`Unknown column '$1'`）；PG `SELECT ?`❌（`syntax error`）/ `SELECT $1`✅。源码：`sqlx-core-0.8.6/src/query_builder.rs:161`（唯一调用点）、`sqlx-postgres-0.8.6/src/arguments.rs:156`（PG 覆写）、`sqlx-core-0.8.6/src/arguments.rs:27`（默认输出 `?`） |
| **专用函数不存在** | `iam-service/src/outbox_archive.rs:50,67`（`GET_LOCK`/`RELEASE_LOCK`）、`routes/internal_membership.rs:55`（`GROUP_CONCAT`） | PG 无这些函数 | grep 实测 + PG 容器探针：`STRING_AGG(x::text, ',')` ✅ 返回 `1,2` |
| **会话时区未锁定** | `token-service/src/db.rs::after_connect` 强制 `SET time_zone = '+00:00'`；其余服务无此保障 | PG 的 `NOW()` 返回 `timestamptz`，写入 naive `TIMESTAMP` 时按**会话 TZ** 换算 → 非 UTC 会话下写入值偏移 | **容器实证**：`SET TIME ZONE 'Asia/Shanghai'` 后 `now()` 写入 naive 列偏移 +08 小时，而 `now() AT TIME ZONE 'UTC'` 不受影响 → PG 连接必须 `SET TIME ZONE 'UTC'` |

---

## 6. 验收标准（可执行门禁清单）

### 6.0 验证环境约定（**每条门禁都标注验证环境**）

| 标记 | 含义 | 说明 |
|---|---|---|
| 🟢 **沙箱内** | 当前受限沙箱环境即可当场确认 | 纯静态检查（脚本 / grep / Node）、无需编译或联网的项 |
| 🔵 **需 WSL2** | 必须在可跑 WSL2 的环境复验 | 涉及 `cargo` 编译/测试的项。**Windows 原生 MSVC 不可用**（`ring`/`aws-lc-sys` 的 C 依赖在 build script 阶段报 `D8050 无法加载 c1.dll`），且沙箱策略拦截 `wsl.exe` → **沙箱内的 cargo 结果不足以作为验收依据** |
| 🟣 **需 CI** | 必须在 CI 上确认 | 涉及 CI 编排、service container、多服务编排的项 |

> **已知约束链**（主理人 2026-11-02 确认）：Windows 原生 MSVC 编译不可用 → 沙箱拦截 `wsl.exe` → **服务层编译验证必须在能跑 WSL2 的环境复验**。因此下表的 🔵 项在实施阶段**不得仅凭沙箱结果判定通过**，必须留待 WSL2 复验。
> **本次 PRD 阶段的实测结论**（供实施参考）：`cargo check -p digua_common_db --features postgres` **已在沙箱内通过**（该 crate 不含 `ring`/`aws-lc-sys` 等 C 依赖）；服务层 crate 因 `utoipa-swagger-ui` 构建脚本需联网下载 swagger-ui 而在沙箱内失败，属**环境限制而非代码缺陷**。

### 6.1 编译门禁

| # | 命令 | 通过判据 | 验证环境 |
|---|---|---|---|
| A-1 | `cargo check -p digua_common_db --features postgres` | 零 error（**当前已通过**，仅 2 个 unused warning） | 🔵 需 WSL2 |
| A-2 | `cargo check -p digua_common_db --features mysql` | 零 error，无新增 warning | 🔵 需 WSL2 |
| A-3 | `cargo check -p digua_common_db --features sqlite` | 零 error，无回归 | 🔵 需 WSL2 |
| A-4 | `cargo check --workspace`（default features） | 零 error；**`postgres` 不在 default features，默认构建产物不变** | 🔵 需 WSL2 |
| A-5 | `cargo check -p <svc> --features postgres`（**全部 8 个交付单元**） | 零 error | 🔵 需 WSL2 |
| A-6 | `cargo tree -p <svc> -e features -i digua_common_db` | 可见 `digua_common_db feature "postgres"` | 🟢 沙箱内 |
| A-7 | `grep -n 'default = ' libs/common-db/Cargo.toml` | 仍为 `default = ["sqlite"]`，**不含 postgres** | 🟢 沙箱内 |

### 6.2 迁移门禁

| # | 命令 / 操作 | 通过判据 | 验证环境 |
|---|---|---|---|
| B-1 | `ls services/<svc>/migrations/postgres/` | 文件数 == MySQL 侧文件数（audit 1 / config 2 / file 2 / iam 8 / notification 3 / prue 1 / token 2） | 🟢 沙箱内 |
| B-2 | 在 PG 16 容器上跑 `ensure_schema` | 建表成功；**重复执行无错误**（幂等） | 🔵 需 WSL2 |
| B-3 | PG 与 MySQL schema 对比（`information_schema.columns` 逐列） | 列名、类型语义、可空性、默认值一致（`TINYINT(1)`↔`BOOLEAN` 视为等价） | 🔵 需 WSL2 |
| B-4 | `grep -rn "AUTO_INCREMENT\|ON DUPLICATE KEY\|ON UPDATE CURRENT_TIMESTAMP\|IFNULL\|GROUP_CONCAT\|DATABASE()\|ENGINE=\|UNIQUE KEY\|TINYINT(1)\|COMMENT '" services/*/migrations/postgres/` | **零命中**（PG 迁移内不得残留 MySQL 语法） | 🟢 沙箱内 |
| B-5 | 表名前缀校验 | 7 个服务的 PG 迁移表名均带服务前缀（`iam_`/`audit_`/`token_`/`ntf_`/`cfg_`/`file_`/`prue_`） | 🟢 沙箱内 |
| B-6 | **PG 迁移版本号与 MySQL 侧一一对应**（marker 语义对齐） | `ls services/<svc>/migrations/postgres/` 的文件序号与 `migrations/mysql/` 完全一致 | 🟢 沙箱内 |
| B-7 | **PG 会话 TZ 锁定验证** | 在非 UTC 会话 TZ 环境下写入 naive `TIMESTAMP` 列，值与 UTC 环境一致（容器实证 `Asia/Shanghai` 场景） | 🔵 需 WSL2 |
| B-8 | **PG 侧 DDL 可执行性** | PG 迁移集在 PG 16 容器内全量执行成功，且**无 MySQL 残留语法报错** | 🔵 需 WSL2 |
| B-9 | **`CURRENT_TIMESTAMP(3)` 精度验证** | PG 侧 `DEFAULT CURRENT_TIMESTAMP(3)` 建表成功，且列值按 3 位截断（Q7 裁决 A） | 🔵 需 WSL2 |

### 6.3 门禁脚本与 CI

| # | 命令 | 通过判据 | 验证环境 |
|---|---|---|---|
| C-1 | `node scripts/check-db-naming.js` | **零误报**（当前对 `ON CONFLICT DO UPDATE SET` 误报，必须先修 P0-6） | 🟢 沙箱内 |
| C-1b | `node scripts/check-db-naming.js`（PG 语料回归） | `ON CONFLICT ... DO UPDATE SET` / `DO NOTHING` / `pg_catalog.pg_indexes` / `current_schema()` 均不误报 | 🟢 沙箱内 |
| C-2 | `node scripts/check-spec.js` | 零 error | 🟢 沙箱内 |
| C-3 | `node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'` | 无 diff（PG 支持**不改变 API**，openapi.yaml 必须零变更） | 🟢 沙箱内 |
| C-4 | `node scripts/check-contracts.js`（若存在） | 零 error | 🟢 沙箱内 |
| C-5 | CI `dialect-matrix.yml` 的 `test-services-postgres` job | PR 上真实跑过一次且通过（postgres:16 service container + 健康检查） | 🟣 需 CI |
| C-6 | `dialect-matrix.yml` 的 `test-postgres` job（既有） | 不回归，仍通过 | 🟣 需 CI |

### 6.4 测试门禁

| # | 命令 | 通过判据 | 验证环境 |
|---|---|---|---|
| D-1 | `cargo test -p digua_common_db --features postgres` | 全绿；无 `TEST_PG_URL` 时 PG 用例**跳过而非失败** | 🔵 需 WSL2 |
| D-2 | `cargo test -p digua_common_db --features mysql` | 不回归 | 🔵 需 WSL2 |
| D-3 | `cargo test -p <svc> --features postgres`（**全部 8 个交付单元**） | 集成测试对 PG 后端通过 | 🔵 需 WSL2 |
| D-4 | `cargo test -p <svc> --features mysql`（全部） | 不回归 | 🔵 需 WSL2 |
| D-5 | `node scripts/run-tests-host.js`（WSL2） | 全绿 | 🔵 需 WSL2 |
| D-6 | `cargo test --workspace` | 不回归 | 🔵 需 WSL2 |
| D-7 | PG 后端下业务断言与 MySQL 后端**逐项一致** | 同一测试文件对两种后端参数化执行，断言结果一致 | 🔵 需 WSL2 |

### 6.5 文档门禁

| # | 检查项 | 通过判据 | 验证环境 |
|---|---|---|---|
| E-1 | 7 个服务 `AGENTS.md`「数据库方言」小节 | 均声明 PG 支持状态，且与实际实现一致 | 🟢 沙箱内 |
| E-2 | `contracts/db-conventions.md` | 含 PG 类型映射章节（`TINYINT(1)`→`BOOLEAN`、`DATETIME`→`TIMESTAMP`、`AUTO_INCREMENT`→`IDENTITY`、内联 `INDEX`/`UNIQUE KEY`→独立索引、`COMMENT`→`COMMENT ON`、`JSON`/`JSONB` 取舍） | 🟢 沙箱内 |
| E-3 | `docs/superpowers/specs/dialect-matrix.md` | 含 PG 差异列 + 按服务×方言的测试矩阵 | 🟢 沙箱内 |
| E-4 | `docs-site/guides/testing.md` | 补 PG 测试容器用法（`coder-test-pg` / `TEST_PG_URL` / `shared-db.js` 的 `ensurePostgres`） | 🟢 沙箱内 |
| E-5 | `scripts/lib/shared-db.js` | `ensurePostgres` 与 `MYSQL_CONTAINER` 对称（容器名 `coder-test-pg`，端口与 `aipm-pg-dev:5433` 隔离） | 🟢 沙箱内 |

### 6.6 交付完成判据（Q3 裁决 B：全量）

全部 8 个交付单元（7 服务 + `iam-outbox-publisher`）满足上述门禁，且：

| # | 判据 | 验证环境 |
|---|---|---|
| F-1 | 7 个服务各有 `migrations/postgres/`，共 19 个迁移文件，版本号与 MySQL 侧一一对应 | 🟢 沙箱内 |
| F-2 | 全部 8 个单元在 MySQL 与 PG 双后端下编译、测试通过 | 🔵 需 WSL2 |
| F-3 | `postgres` feature 未进入任何 `default features` | 🟢 沙箱内 |
| F-4 | CI `test-services-postgres` job 覆盖全部交付单元并通过 | 🟣 需 CI |
| F-5 | 门禁脚本对全部 PG 迁移零误报 | 🟢 沙箱内 |
| F-6 | 文档三处（服务 AGENTS.md / db-conventions / dialect-matrix）+ `docs-site` 同步 | 🟢 沙箱内 |

---

## 7. 待确认问题（请主理人裁决）

### Q1 ⭐ 最高优先级：占位符适配策略

> **背景（容器实证 + 源码核对）**：
> - **MySQL 只认 `?`**：`PREPARE s FROM 'SELECT ? AS a'` ✅ 返回 `1`；`SELECT $1 AS a` ❌ `ERROR 1054 Unknown column '$1' in 'field list'`。
> - **PG 只认 `$N`**：`PREPARE p AS SELECT ? AS a` ❌ `syntax error at or near "?"`（`?` 在 PG 是 jsonb 存在性操作符）；`SELECT $1 AS a` ✅ 返回 `42`。
> - **sqlx-core 0.8.6 中搜不到任何 `?`→`$N` 重写逻辑**：`format_placeholder` 的**唯一调用方**是 `sqlx-core/src/query_builder.rs:161`（即 `QueryBuilder::push_bind`）。MySQL/SQLite 用默认实现（`sqlx-core/src/arguments.rs:27` 输出 `?`），**仅 PG 覆写**为 `$N`（`sqlx-postgres/src/arguments.rs:156`）。
> - **仓库现状**：编译期宏 `sqlx::query!`/`query_as!`/`query_scalar!` 用量为 **0**；**309 个查询调用点全部是运行时 `sqlx::query`/`query_as`/`query_scalar` + `.bind()`**，其中**含 `?` 需适配的调用点 226 个**（iam 122 / notification 41 / token 19 / prue 17 / audit 12 / iam-outbox-publisher 7 / config 4 / file 4）——口径见 §5.1。
> - 全仓 **44 处 `QueryBuilder::new(`** 用法**天然跨方言**（`push_bind` 自动产出正确占位符），且 `QueryBuilder::new(...)` 初始串中**零 `?`**（实测）。这是 Q1 选 A 的既有范式先例。

| 选项 | 做法 | 优点 | 缺点 | 工作量 |
|---|---|---|---|---|
| **A. 方言感知的 SQL 构造辅助层** ⭐ 裁决采纳 | 在 `libs/common-db` 提供一层薄封装（如 `q(dialect, "SELECT ... ? ...")` 或 `Db`-trait 上的 `query`/`query_as` 方法），**在 SQL 构造期**按目标方言产出 `?`（MySQL/SQLite）或 `$1..$N`（PG）。**同一份 SQL 源码**服务两种后端 | ① 业务 SQL **不重复**，双后端单份源码（符合仓库「单一真相源」）；② 与 sqlx 自身的 `QueryBuilder` 机制**同构**（`format_placeholder` 即同一思路），不引入新范式；③ 复用既有 `DialectKind` 抽象，扩展达梦/MSSQL 时可直接沿用；④ **与 bind 顺序天然对齐**——辅助层同时产出 SQL 与参数序号，不存在「重写后编号错位」 | ① 需 226 个含 `?` 的调用点改为经由辅助层（**每处一行改动**，非 SQL 重写）；② 必须严格处理**SQL 字面量内的 `?`**——实测全仓字面量内**零个 `?`**（`LIKE '?%'` 类风险不存在），但需加断言防回归；③ 调试时看到的 SQL 与实际发送的可能不同，需在错误信息里回显原始 SQL | **中**（1 个库层模块 + 226 个调用点改写调用形式） |
| B. 逐条改为 `$N` + MySQL 侧另存一份 | 人工把 226 个含 `?` 的调用点改为 `$N`，MySQL 侧保留原 `?` 版本 | 无抽象层，最直白 | ⚠️ **实测已证伪「单份改写」**：MySQL 不认 `$N`（`Unknown column '$1'`），所以**每个查询必须维护两份 SQL**；309 个查询点双份 ≈ 600+ 份 SQL，漂移风险极高，与仓库「单一真相源」直接冲突 | **极大**（≈ 2 倍 SQL 维护量） |
| C. 编译期宏 / proc-macro | 新建宏在编译期生成对应方言的 SQL | 零运行时开销 | ① 需新建 proc-macro crate（新构建依赖 + 编译期复杂度）；② 仓库**零个编译期 SQL 宏**在用，无既有范式；③ 仍无法避免「一份源码 → 两份产物」的维护问题 | **大** |

**推荐：A（方言感知的 SQL 构造辅助层）**。

**推荐理由（修正上一版表述）**：上一版曾以「运行时重写」为名推荐，措辞不准确，易被理解为「在 sqlx 内部拦截并改写」——那确实不成立（sqlx 无此 hook）。**准确的做法是「在 SQL 构造期由我方代码产出正确占位符」**，即 sqlx 自己的 `QueryBuilder` 所做的事；A 方案只是把 `QueryBuilder` 的能力收敛为一个可复用、跨方言统一的服务级 API。

**关键澄清**：主理人提出的「逐条改写为 `$N`」在**单份 SQL** 语义下已被实测证伪（MySQL 会把 `$1` 当列名）。若要采纳该方向，**必须同时接受 MySQL 侧另存一份**（即选项 B 的完整形态）。这是二选一的真实取舍，不能只取其半。

> **配套要求（无论选哪个）**：辅助层必须对「SQL 字面量内出现 `?`」显式报错或跳过，并加单元测试锁定；错误信息须回显**原始 SQL** 与方言，便于排障。

---

### Q1-bis 方言感知的辅助层落点（供 Q1 选 A 时直接执行）

| 落点 | 做法 | 适用 |
|---|---|---|
| **L1. 库层函数** | `digua_common_db::q(dialect, sql) -> String` + `sqlx::query(&q(...)).bind(...)` | 改动最小；但调用点需持有 `dialect`，且返回 `String` 有生命周期约束 |
| **L2. 库层 trait** ⭐ 裁决采纳 | 在 `DbPool`/新 `Db` 上提供 `query`/`query_as`/`query_scalar` 方法，内部按方言分流 | 调用点形态与 `sqlx::query` 接近，**不暴露方言细节**；与 P0-1/P0-9 的类型泛型化同一处收敛 |
| **L3. 复用 `QueryBuilder`** | 把 226 个含 `?` 的调用点改写为 `QueryBuilder` | sqlx 原生支持，零自研；但 `QueryBuilder` 只能覆盖 `push_bind` 形式，复杂 SQL 改写量大，且现有 `QueryBuilder` 用法是 `MySqlPool` 类型专属（`qb.build_query_as::<T>().fetch_all(&self.pool)`），仍需先做类型泛型化 |

**推荐 L2**：与「`MySqlPool` 泛型化」（Q6）是**同一处改造**，避免为占位符单独引入一层。

---

### Q2 `ON UPDATE CURRENT_TIMESTAMP` 在 PG 的替代方案

> **背景（实测）**：全仓仅 **2 处**——`prue_backend/migrations/mysql/V001__init.sql:14` 与 `token-service/migrations/mysql/V001__init.sql:14`，均为 `updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP`。
> **应用层现状**：config-service / file-service **已在应用层显式维护** `updated_at`（`qb.push(", updated_at = ")`、`updated_at: now`）；prue_backend 的 UPDATE 也显式带 `updated_at = NOW()`。**token-service 的 `token_oauth_clients` 更新路径未显式写 `updated_at`**（`oauth_client_svc.rs:311,348`）。

| 选项 | 做法 | 优点 | 缺点 |
|---|---|---|---|
| **A. 应用层维护** ⭐ 裁决采纳 | PG 迁移去掉 `ON UPDATE`，改为 `DEFAULT CURRENT_TIMESTAMP`；由 Rust 侧在 UPDATE 语句中显式写 `updated_at = now()` | ① 无额外 DB 对象，跨方言行为一致（config/file 本来就这么做）；② 触发器在 PG 中需 `CREATE FUNCTION ... $$ ... $$`，而本仓迁移执行器用 `script.split(';')` 切分语句（`token-service/src/db.rs`、`prue_backend/src/db.rs:70`）——**`$$` 内的分号会被切坏**；③ 无触发器性能开销与维护成本 | token-service 需补 2 处 UPDATE 的 `updated_at`；若存在**未审计的第三方 SQL** 直接更新该表，语义会漂移 |
| B. PG 触发器 | `CREATE FUNCTION set_updated_at() RETURNS trigger ... $$ ... $$ LANGUAGE plpgsql;` + `CREATE TRIGGER` | ① 语义与 MySQL 完全等价，DB 层保证，第三方写入也覆盖 | ① **必须改造迁移切分器**（`split(';')` 会切坏 `$$` 函数体，需支持 dollar-quoted 感知或改用 `sqlx::migrate!`）；② 引入 PG 专有对象，与「MySQL/PG 迁移对称」的迁移集理念不合；③ 触发器在 `UPDATE` 热路径有开销 |
| C. 放弃该语义 | PG 侧 `updated_at` 仅在 INSERT 时赋值，UPDATE 不刷新 | 改动最小 | ⚠️ **语义降级**：`token_oauth_clients.updated_at` 会变成「创建时间」，对管理后台展示/审计是**功能性错误**——不可接受 |

**推荐：A（应用层维护）**。决定性理由是 **Q1 已确立「迁移目录按方言分叉、应用层统一」的形态**，而本仓迁移执行器（`include_str!` 内嵌 + `script.split(';')` 切分，`token-service/src/db.rs`、`prue_backend/src/db.rs:70`）与 PG 的 `$$` 函数体**根本不兼容**——这是独立于本决策的、客观存在的技术约束。另需在 P1 补齐 token-service 的 2 处 UPDATE。

> **补充约束**：PG 触发器方案还要求迁移集引入 `CREATE FUNCTION`，而 `libs/common-db/src/lib.rs::run_migrations` 对所有方言均返回 `DB_MIGRATE`（运行期路径迁移未实现），各服务靠自己的 `ensure_schema` 执行内嵌 SQL —— 即触发器函数必须能被 `split(';')` 正确切分，进一步加大 B 方案成本。

---

### Q2-bis ⚠️ 会话时区必须锁 UTC（新增，实测发现）

> **背景（容器实证）**：PG 的 `NOW()` 返回 `timestamptz`，MySQL 的 `NOW()` 返回**会话本地 naive datetime**。实测把 PG 会话 TZ 改为 `Asia/Shanghai` 后，写入 naive `TIMESTAMP` 列的 `now()` 值偏移了 **+08 小时**，而 `now() AT TIME ZONE 'UTC'` 不受影响。
> 仓库现状：**仅 `token-service` 有 `after_connect` 强制 `SET time_zone = '+00:00'`**，其余 6 个服务无此保障。

| 选项 | 内容 | 影响面 |
|---|---|---|
| **A. 库层统一在 PG 连接建立后 `SET TIME ZONE 'UTC'`** ⭐ 裁决采纳 | 在 `libs/common-db` 的 PG 分支 `init_pool` 里统一设置，与 token-service 现有做法对齐 | ✅ 一处生效，覆盖全部服务；✅ 与基座层 `now_expr()`（`now() AT TIME ZONE 'UTC'`）语义一致 |
| B. 各服务自行设置 | 每个服务 PG 连接后手动 `SET TIME ZONE 'UTC'` | ❌ 7 处重复，易漏；且与 `token-service` 现有 `SET time_zone='+00:00'` 写法不统一 |
| C. 依赖容器 TZ | 假定 PG 容器 TZ 为 UTC | ❌ 不可靠：客户自建 RDS 时区不受控；naive 列写入会静默偏移 |

**推荐：A**。这是**静默数据错误**（不报错，但时间戳偏移），风险等级应与占位符同级。

---

### Q3 交付范围：全量还是分批？

> **裁决（2026-11-02，主理人）：B — 全量交付。** 需求方已明确选定「服务层**全量** PostgreSQL 支持」。
> **⚠️ 关键区分**：本裁决改变的是**交付范围**，不是**提交粒度**。实施仍按 §9 的 S0~S6 切片推进（**一个切片一个 commit**），但 P1 项属**本次交付范围内**，不是「另起一期」。

| 选项 | 内容 | 影响面 |
|---|---|---|
| A. 分批（实施顺序优先） | 先交付 P0 三服务，P1 另起一期 | ⚠️ 与需求方「全量」诉求冲突<br>✅ 单次变更面小，返工风险低 |
| **B. 全量交付（7 服务 + `iam-outbox-publisher`）** ⭐ 裁决采纳 | 一次性交付全部服务的 PG 支持，但**按 S0~S6 切片分次提交** | ✅ 满足需求方「全量」诉求<br>⚠️ 单次变更面覆盖 309 个查询调用点 + 145 处 MySQL 类型硬编码 + 19 个迁移文件 + 4 个共享库 → **方言形态一旦选错，返工面覆盖全仓**<br>✅ **通过 S0~S6 切片 + 每切片独立验证来控制风险**：形态先在 P0 三服务定型并跑通，再照模板铺开<br>✅ 满足 AGENTS.md「一个任务一个 commit」——切片即 commit 边界 |
| C. 只做 P0 三服务，PG 支持**不承诺全量** | 明确把 PG 定位为「部分服务可选后端」 | ✅ 工作量最小<br>⚠️ 与需求标题直接冲突 |

**裁决：B（全量交付，按切片提交）**。

**实施约束**：
1. **P0/P1 语义调整**：§3 的 P0/P1 分级**仅表示实施先后**，不表示交付分期。P1 项必须在本次交付内完成。
2. **切片即 commit**：每个 S 切片一个 commit，遵循 AGENTS.md「一个任务一个 commit」「禁止 amend / force-push」。
3. **风险控制手段**：形态定型靠 S1（库层抽象）+ S3（P0 三服务打样）——**先验证抽象，再铺开**，这是全量交付下唯一的返工控制手段。Q1/Q2 已裁决，正是为了消除「形态未定就铺 7 份」的风险。
4. **交付完成判据**：7 个服务 + `iam-outbox-publisher` 全部满足 §6 门禁，方视为交付完成。

---

### Q4 testcontainers PG 是否所有服务都补？

> **背景（实测）**：`testcontainers-modules` 0.8.0 **已内置 `postgres` 模块**（`postgres = []` feature + `src/postgres/mod.rs`），PG 容器无版本障碍。

| 选项 | 内容 | 影响面 |
|---|---|---|
| **A. 仅 P0 服务 + 共享库** ⭐ 裁决采纳 | P0 三服务 + `common-db`/`common-event` 补 PG 容器变体；P1 服务在 P1 补 | ✅ CI 时长可控（PG 容器启动慢，每个服务多 ~10s）；✅ 先验证 fixture 抽象是否正确，再铺开<br>⚠️ P1 阶段需再改一次 fixture |
| B. 全部 7 服务一次性补 | 7 个服务 + 共享库全补 PG 容器 | ✅ 一次到位<br>⚠️ CI `test-services-postgres` job 运行时长显著上升（7 个服务 × 容器启动 + 迁移 + 测试）；⚠️ 抽象形态未定就铺开，返工风险 |
| C. 用共享常开 PG 容器替代 testcontainers | 复用 `127.0.0.1:5434`（基座层设计中的 `coder-test-pg`） | ✅ 快<br>⚠️ **CI 不可用**（CI 无常开容器），本地与 CI 行为分叉，与现有 MySQL 的 testcontainers 惯例不一致 |

**推荐：A**。理由：fixture 形态（P0-10）本质是 Q1/Q2 抽象的延伸，形态未定就铺 7 份等于放大返工面。

---

### Q5 `libs/common-event` 的 outbox 泛型化形态

| 选项 | 内容 | 影响面 |
|---|---|---|
| **A. trait 泛型化**（`OutboxRepo` 不绑 `MySql`，新增 `PgOutboxRepo`） ⭐ 裁决采纳 | 改 `libs/common-event/src/outbox.rs` | ✅ 与 `libs/common-db` 的 `DbPool` 枚举风格一致<br>⚠️ `common-event` 被 5 个服务依赖，变更影响面横跨 iam / notification / audit / iam-outbox-publisher / api-gateway |
| B. 平行实现（保留 `MySqlOutboxRepo`，另加 `PgOutboxRepo`） | 新增文件，不动既有 | ✅ 既有零回归风险<br>⚠️ 两份 SQL 逻辑重复，后续易漂移 |

**推荐：A**（trait 已存在，泛型化是自然延伸）。但这属于**跨 5 个服务的共享库变更**，建议独立 commit、独立 spec 段落。

---

### Q6 `MySqlPool` 类型绑定的改造形态（影响 145 处）

| 选项 | 内容 | 影响面 |
|---|---|---|
| **A. 全部收敛到 `DbPool` 枚举** | 各服务改用 `digua_common_db::DbPool`，按方言 match | ✅ 与基座层设计一致，`DbPool::Postgres` 已有<br>⚠️ 145 处签名改写；每处需 match 分支，**SQL 可能要分方言两份**（与 Q1-A 的单份 SQL 目标冲突） |
| B. 泛型化 `Executor`/`Pool` 参数 | 函数签名改为 `impl Executor` / 泛型 `DB: Database` | ✅ SQL 单份、零分支<br>⚠️ sqlx 的 `query_as` 在泛型 `DB` 下需 `DB: Database` + `for<'r> FromRow<'r, DB::Row>`，**行类型约束复杂**；且 `?` 与 `$N` 差异仍需 Q1 方案兜住；改动侵入性最强 |
| **C. 混合** ⭐ 裁决采纳 | `DbPool` 枚举决定后端（库层已就绪）+ Q1-bis 的 **L2 库层 trait** 提供方言感知查询方法 | ✅ 枚举提供「哪个后端」信息，trait 提供「SQL 单份」能力<br>✅ **两处改造落在同一层（`libs/common-db`），可合并为一个 commit**<br>⚠️ 需把 trait 方法与 `DbPool` 枚举协同设计 |

**裁决：C**。即 `DbPool` 决定后端（基座层已就绪），库层 trait 保证 SQL 单份（Q1-A / Q1-bis-L2）。若评审倾向更少改动，可退到 A 并接受部分 SQL 分方言两份（仅限确有语法差异者：`GET_LOCK`/`GROUP_CONCAT`/`ON DUPLICATE KEY`——实测这 3 类合计约 30 处，占比可控）。

> **注意**：`token-service` **自建了本地 `DbPool` 枚举**（`services/token-service/src/db.rs:41`，仅 `Mysql` 变体），与 `digua_common_db::DbPool` **重复**。Q6 选 A 或 C 时，应顺带**删除这个重复枚举**、统一复用 common-db 的 `DbPool`，避免两份方言抽象并存。

---

### Q7 `DEFAULT CURRENT_TIMESTAMP(3)` 的 PG 写法

> **背景（容器实证）**：PG 16.15 **接受** `CURRENT_TIMESTAMP(3)`——`CREATE TABLE ... a TIMESTAMP(3) DEFAULT CURRENT_TIMESTAMP(3)` 建表成功；`SELECT CURRENT_TIMESTAMP(3)` 返回 `11:10:28.735+00`（按 3 位截断）；且 `CURRENT_TIMESTAMP(3) = now()::timestamptz(3)` 为 **true**，证明精度参数被真正应用。
> **⚠️ 但真正的风险不是语法，是时区**：在 `SET TIME ZONE 'Asia/Shanghai'` 会话下，naive `TIMESTAMP` 列的 `DEFAULT CURRENT_TIMESTAMP(3)` 存入 **19:14**（本地），而 MySQL `DATETIME` 语义期望 **11:14**（UTC）。**偏移 +08 小时且不报错。**

| 选项 | 内容 | 语法 | 会话 TZ = Asia/Shanghai 时存入值 | 判定 |
|---|---|---|---|---|
| **A. 保持 `CURRENT_TIMESTAMP(3)`** ⭐ 裁决采纳 | 列类型用 `TIMESTAMP(3)`，默认值原样保留 | ✅ 接受 | `19:14`（本地，**随会话 TZ 变**） | 语法正确；**时区依赖 → 必须配合 Q2-bis 锁 UTC** |
| B. 改 `LOCALTIMESTAMP(3)` | PG 的 naive 时间戳函数 | ✅ 接受 | `19:14`（本地，**随会话 TZ 变**） | 同 A，时区依赖 |
| C. 改 `now()::timestamp(3)` | 显式转换 | ✅ 接受 | `19:14`（本地，**随会话 TZ 变**） | 同 A，时区依赖 |
| D. 改 `(now() AT TIME ZONE 'UTC')` | 基座层 `now_expr()` 的写法 | ✅ 接受 | ✅ **`11:14`（UTC，与会话 TZ 无关）** | **唯一时区无关的写法** |

**裁决：A（保持 `CURRENT_TIMESTAMP(3)`，列类型 `TIMESTAMP(3)`）**。

**裁决理由**：① 语法在 PG 16 确实被接受，无需为语法而改写；② 与 MySQL 版**语义与精度完全一致**（都是 3 位毫秒）；③ 9 处几乎零成本；④ 时区问题由 **Q2-bis 的库层统一 `SET TIME ZONE 'UTC'`** 统一解决——这是**单点修复**，比在 9 处迁移里各写一遍 TZ 表达式更可维护。

> **给实施者的强约束**：A 方案成立的前提是 **Q2-bis 必须落地**（PG 连接建立后 `SET TIME ZONE 'UTC'`）。若因任何原因未锁 UTC，则 A/B/C 三者**同样**会偏移 8 小时——**问题出在会话 TZ，不在写法**。若评审希望「写法自身免疫 TZ」（不依赖 Q2-bis），则应选 **D**（`(now() AT TIME ZONE 'UTC')`），代价是与 MySQL 版字面量不一致、9 处需人工改写。

**推荐：A**。实测已证 PG 支持该语法，无需改写；选 C 则把正确性绑死在会话 TZ 上。

> **但需注意语义差异（比语法更重要）**：MySQL 的 `CURRENT_TIMESTAMP` 返回**会话本地 naive 时间**，PG 的 `CURRENT_TIMESTAMP` 返回 **`timestamptz`**。写入 PG 的 naive `TIMESTAMP` 列时按会话 TZ 换算 → **必须配合 Q2-bis 锁定会话 TZ 为 UTC**，否则时间戳静默偏移。

---

### Q8 本地测试容器与 PG 服务编排入口（新增）

> **背景（实测）**：`scripts/lib/shared-db.js` 的 `ensureMysql(host, port)` 硬编码 `MYSQL_CONTAINER = 'coder-test-mysql'`，是本地测试拉起 DB 的统一入口；`infrastructure/` 下 7 个 compose 文件**零 postgres 引用**。基座层设计文档已规划 `coder-test-pg`（端口 5434，与 `aipm-pg-dev:5433` 隔离）。

| 选项 | 内容 | 影响面 |
|---|---|---|
| **A. `shared-db.js` 增加 `ensurePostgres` + compose 增加可选 PG service（profile 隔离，默认不启动）** ⭐ 裁决采纳 | 与 MySQL 现有惯例对齐 | ✅ 本地开发与 CI 路径统一；✅ profile 隔离不影响现有 MySQL 用户<br>⚠️ 需同步 `docs-site/guides/testing.md` |
| B. 仅 CI 用 testcontainers，本地不提供 PG 入口 | 只做 CI job | ✅ 改动最小<br>⚠️ 本地开发者无法本地验证 PG，只能依赖 CI，调试成本高 |

**推荐：A**。与既有 `ensureMysql` 惯例一致，且 `shared-db.js` 是现成的统一入口，改动集中在一个文件。

---

## 8. 风险登记（按严重度排序）

| # | 严重度 | 风险 | 触发条件 | 缓解措施 |
|---|---|---|---|---|
| R-1 | 🔴 **致命** | **`DbPool` 变体增加击穿不可反驳模式**：`token-service` 3 处 `let DbPool::Mysql(p) = ...` 触发 E0005；`prue_backend` 20 处 match 触发 E0004 | 任何服务开启 `postgres` feature 即刻发生（已用 rustc 最小复现验证） | ① P0-1/P0-9 先行改造这些模式为 `match`/显式分支；② 改造前用 `cargo check --features postgres` 做**红→绿**验证；③ 把 `let DbPool::` 模式列入代码评审清单 |
| R-2 | 🔴 **致命** | **占位符双向不兼容**：MySQL 只认 `?`、PG 只认 `$N`；226 个含 `?` 的调用点在 PG 侧全部语法错误，**且把源码逐条改成 `$N` 会反向打断 MySQL**（MySQL 报 `Unknown column '$1'`） | 首次在 PG 上执行任一含 `?` 的查询 | ① 裁决 Q1 为「方言感知构造层」（Q1-bis L2），**不采用「单份改写为 `$N`」**；② 辅助层加「字面量内 `?`」断言；③ P0 验收强制含 `?` 的服务（prue/iam/notification）实跑 |
| R-3 | 🔴 **致命** | **门禁脚本误报阻断交付**：`ON CONFLICT ... DO UPDATE SET` 被判为裸表名 `SET`，CI 直接红 | 提交首个 PG 迁移文件即触发（已脚本模拟复现） | ① P0-6 优先级提到 P0 最前，**先修脚本再写迁移**；② 修复需同时把 `pg_catalog` / `current_schema` 加入白名单；③ 用 PG 语料做脚本回归测试（C-1b） |
| R-3b | 🟢 **低** | **`check-db-naming.js` 第 7 行 `VALID_PREFIXES` 存在 JS 稀疏数组空洞**：`['iam_', ..., 'prue_', ,]` 末尾多一个逗号，产生 `undefined` 元素 | 已存在（非本次引入）；`isValidTableName` 的 `VALID_PREFIXES.some(p => name === p \|\| name.startsWith(p))` 遇 `p === undefined` 时 `name.startsWith(undefined)` 会把 undefined 转成字符串 `"undefined"` 比较——**不崩**（`"undefined"` 不匹配任何合法表名），且 `name === p` 对 undefined 永不匹配 → **当前不阻塞门禁** | ① S0 顺手修正为无空洞写法（`['iam_', ..., 'prue_']`），**不扩大范围**；② 扩展前缀列表时若不修，可能引入真 bug（`some` 跳过空洞，行为不可预期）；③ 补一条脚本自测断言前缀列表长度与内容 |
| R-4 | 🔴 **致命** | **会话时区未锁定导致时间戳静默偏移**：PG `NOW()` 返回 `timestamptz`，写入 naive `TIMESTAMP` 按会话 TZ 换算；实测 `Asia/Shanghai` 下偏移 **+08 小时**，**不报错**。仓库仅 `token-service` 有 `SET time_zone='+00:00'`，其余 6 服务无保障 | 任一非 UTC 会话 TZ 的 PG 部署 | ① 裁决 Q2-bis，在 `libs/common-db` PG 分支统一 `SET TIME ZONE 'UTC'`；② 验收加「跨时区写入一致性」检查；③ PG 迁移优先用 `TIMESTAMPTZ` 显式带时区列 |
| R-5 | 🟠 **高** | **MySQL 类型硬绑定 145 处**（`MySqlPool` 124 + `MySqlPoolOptions` 21），改造面超预期，工期失控 | 逐服务改造时发现签名外溢（状态、路由、测试 fixture 全线波及） | ① 优先做 P0 三服务把改造面量化；② 库层抽象先行（Q1/Q6 先裁决）；③ 每个服务独立 commit，严格「一个任务一个 commit」 |
| R-6 | 🟠 **高** | **PG 迁移与 MySQL 迁移语义漂移**：列类型/默认值/索引/注释不对齐，产生「建表成功但行为不同」的隐性缺陷 | 手工翻译 DDL 时遗漏细节（`TINYINT(1)`↔`BOOLEAN`、`ENGINE=` 需删除、内联 `INDEX`/`UNIQUE KEY` 需独立化、`COMMENT` 需改 `COMMENT ON`） | ① 验收加 B-3（逐列 schema 对比）；② B-4 禁止 MySQL 语法残留；③ PG 迁移必须写列级注释说明与 MySQL 版的语义对应关系 |
| R-7 | 🟠 **高** | **`iam-service` 的 `GET_LOCK` 语义不等价**：MySQL 是 session-scoped（注释明确「跨连接会泄漏 8 小时」），PG `pg_advisory_lock` 也是 session 级但行为细节不同 | iam-service PG 化时 | ① P1-5（及 P1-13 的同构改造）单独设计并写 spec；② 沿用「单连接持有锁」的现有约束；③ 考虑用 `pg_try_advisory_lock`（对应 `GET_LOCK(?, 0)` 的非阻塞语义） |
| R-8 | 🟠 **高** | **`libs/common-event` 变更横跨 5 个服务**，波及 iam / notification / audit / iam-outbox-publisher / api-gateway | P0-3 落地时 | ① 独立 commit + 独立 spec 段落；② 先只做 trait 泛型化（不改 SQL 语义），PG 实现另起 commit；③ api-gateway 只用 event 类型不用 outbox，实际影响面需在实施时复核 |
| R-9 | 🟡 **中** | **布尔类型跨端不一致**：`TINYINT(1)` 在 sqlx-mysql 解码为 `i8` 而非 `bool`；PG `BOOLEAN` 解码为 `bool` → 同字段两端解码类型不同 | 同一 struct 在两种后端下 `FromRow` 行为不一致 | ① P2-3 单列处理，PG 迁移阶段先保证「值语义等价」；② 优先选 `BOOLEAN` 而非 `SMALLINT`，并同步核对 Rust 侧读取路径 |
| R-10 | 🟡 **中** | **迁移执行器 `split(';')` 与 PG 不兼容**：PG 的 `$$` 函数体、`DO` 块内含分号会被切坏 | 引入 PG 触发器/函数（Q2-B 未采纳）或任何含分号的复合语句 | ① Q2 裁决 A 规避此问题；② 若未来必须用触发器，需先改用 `sqlx::migrate!` 或实现 dollar-quoted 感知的切分器 |
| R-11 | 🟡 **中** | **CI 时长与资源**：PG 容器启动 + 迁移 + 测试，7 服务全量跑将显著拉长 PR 门禁 | Q4 选 B 时 | ① Q4 裁决 A（P0 三服务先行）；② PG job 与 MySQL job 并行而非串行；③ 缓存 cargo 构建产物 |
| R-12 | 🟡 **中** | **`token-service` `DbType` 枚举仅 mysql**，且 `config.rs` 对未知 db_type **静默忽略**（保留 yaml 值）→ 加 `postgres` 变体时可能踩既有「未知值忽略」测试 | P1-3 实施 | ① 改配置语义需独立 spec + 更新既有测试断言；② 保持「未知值忽略」行为不变，仅新增已知变体 |
| R-13 | 🟡 **中** | **文档漂移**：服务 `AGENTS.md` 写「支持 MySQL 和 SQLite」，改后需同步；`dialect-matrix.md` 需补 PG 差异列与测试矩阵；`docs-site/guides/testing.md` 需补 PG 容器用法 | 实现完成后 | ① 文档改动纳入验收 E-1~E-4；② `dialect-matrix.md` 是方言真相源，须与实现一致 |
| R-14 | 🟢 **低** | **Windows 原生 MSVC 不可用**，`ring`/`aws-lc-sys` 的 C 依赖在 build script 阶段失败（`D8050 无法加载 c1.dll`）；本次验证中 `utoipa-swagger-ui` 构建脚本亦因沙箱网络受限失败 | 在 Windows 原生跑 cargo | ① 全部验证走 WSL2（`node scripts/run-tests-host.js`）；② PRD 阶段已声明该约束，交由实施者在 WSL2 复验 |
| R-15 | 🟢 **低** | **`postgres` feature 误入 default features**，导致默认构建产物变化 | 有人图省事把 `postgres` 加进 `common-db` 的 `default` | ① A-4 验收项专门守这条；② `libs/common-db/Cargo.toml` 的 `default = ["sqlite"]` 保持不变；③ code review 显式检查 |
| R-16 | 🟢 **低** | **`token-service` 自建本地 `DbPool` 与 `digua_common_db::DbPool` 重复**，两份方言抽象并存易漂移 | Q6 落地时若遗漏 | ① Q6 裁决 C 已明确要求顺带删除重复枚举；② code review 检查 `pub enum DbPool` 只应存在于 `libs/common-db` |
| R-17 | 🟢 **低** | **`libs/common-db::run_migrations` 对所有方言均返回 `DB_MIGRATE`**，运行期路径迁移未实现 → 若有人误以为库层已支持 PG 迁移，会写出错误调用 | 实施者误读 API 语义 | ① P0-1/P0-2 明确 PG 侧走各服务既有 `ensure_schema` 模式；② 库层补 PG 分支或补文档说明「运行期迁移不支持，请用内嵌 Migrator」；③ §9 交付切片已注明 |
| R-18 | 🟢 **低** | **`DEFAULT CURRENT_TIMESTAMP(3)` 的时区语义**：PG **接受**该语法（实测建表成功、按 3 位截断），但它返回 `timestamptz`，写入 naive `TIMESTAMP` 列时按**会话 TZ** 换算 | 非 UTC 会话 TZ 的 PG 部署，且 **Q2-bis 未落地** | ① 裁决 Q7-A 的前提是 **Q2-bis 必须落地**（库层统一 `SET TIME ZONE 'UTC'`）；② 验收加 B-9（精度）+ B-7（时区）；③ 若 Q2-bis 未落地，则 Q7 须改用 D 方案 `(now() AT TIME ZONE 'UTC')`（唯一 TZ 无关写法，实测存 UTC 值） |

---

## 9. 建议的交付切片（**提交切分**，非交付分期）

> **⚠️ 交付范围为全量**（Q3 裁决 B：7 个服务 + `iam-outbox-publisher`）。下表 S0~S6 是**提交切分**——**一个切片一个 commit**，遵循 AGENTS.md「一个任务一个 commit」「禁止 amend / force-push」。**所有切片完成后即为交付完成**（判据见 §6.6），不存在「另起一期」。

| 切片 | 内容 | 对应需求 | 依赖 | 切片完成判据 |
|---|---|---|---|---|
| **S0** | 修复 `check-db-naming.js` 对 PG 语法的误报 + `VALID_PREFIXES` 双逗号修正 + PG 语料回归 | P0-6、R-3、R-3b | 无（**可立即开工**） | C-1 / C-1b 通过 |
| **S1** | 库层：方言感知查询构造层（Q1-bis-L2）+ **PG 会话 TZ 锁定** + PG marker 探测 + health 探针 + `common::db::connect()` 泛型化 | P0-1、P0-1b、P0-2、P0-4、P0-5 | **Q1 / Q2-bis / Q6 裁决（已定）** | A-1~A-7 通过；库层单测覆盖双后端 |
| **S2** | `libs/common-event` outbox 泛型化（含 `CURRENT_TIMESTAMP(3)` 适配） | P0-3 | Q5 裁决（已定）；S1 | D-1 / D-2 通过 |
| **S3** | P0 三服务（prue / config / file）迁移集 + 类型改造 + 测试 fixture | P0-7~P0-10 | S1、S2 | 三服务在 MySQL / PG 双后端下 B / D 门禁通过 |
| **S4** | CI `test-services-postgres` job + compose 可选 PG service + `shared-db.js::ensurePostgres` + `docs-site/guides/testing.md` | P0-11、P1-7、P1-11、Q8 | S3 | C-5 / D-3 通过 |
| **S5** | 其余 5 个交付单元（audit / token / notification / iam / iam-outbox-publisher）逐个铺开 | P1-1~P1-6、P1-12、P1-13 | S3 形态验证通过；Q2/Q3 裁决（已定） | 五个单元双后端门禁通过；**iam 的 `GET_LOCK`/`GROUP_CONCAT` 专项验证** |
| **S6** | 文档同步（AGENTS.md / db-conventions.md / dialect-matrix.md） | P1-8~P1-10 | S5 | E-1~E-5 通过 |

> **全量交付下的返工控制**：Q1/Q2 已裁决，消除了「形态未定就铺 7 份」的风险；S1 定型库层抽象 → S3 用 P0 三服务打样验证 → S5 照模板铺开。**若 S3 阶段发现抽象形态需调整，改动仅限 S1/S3 范围（库层 + 3 服务），不会波及 S5 的 5 个单元**——这是全量交付仍可控的关键。

> **迁移机制说明**：各服务迁移为 `include_str!` 内嵌 + 各自 `ensure_schema` 执行（`libs/common-db::run_migrations` 对所有方言均返回 `DB_MIGRATE`，运行期路径迁移未实现）。**PG 侧沿用同一模式**：`services/<svc>/migrations/postgres/V00N__*.sql` 由服务内 `include_str!` 引入，版本号与 MySQL 侧**一一对应**以保持 marker 语义对齐。`prue_backend` 现有的 `migrations/{mysql,sqlite}/` 双轨结构即为参照范式；`libs/common-db/src/test_kit/sqlite_fixture.rs` 是现成的方言 fixture 参考。

> **分支与提交纪律**（AGENTS.md 强制）：从 `main` 新建 `feat/<scope>` 分支（如 `feat/postgres-service-support`）；**一个任务一个 commit**（建议按 S0~S6 切片提交）；**禁止 amend / force-push**；改代码前先有 spec（本 PRD 即前置 spec，同提交周期内创建）。
> **迁移纪律**：PG 侧为**新增** `migrations/postgres/` 目录，**不编辑任何已发布 MySQL 迁移**（forward-only）。
> **表名纪律**：PG 迁移表名必须带服务前缀，`node scripts/check-db-naming.js` 门禁覆盖。

---

## 10. 关联文档

- `docs/superpowers/specs/2026-10-30-postgresql-dialect-support-design.md` — 基座层设计（已完成）
- `docs/superpowers/specs/dialect-matrix.md` — 方言差异矩阵（需补 PG 差异列与服务级矩阵）
- `docs/superpowers/specs/2026-06-10-user-service-multi-db-design.md` — 历史多库设计参考
- `docs/superpowers/specs/2026-07-08-prue-backend-sqlite-decision-design.md` — SQLite 双方言决策先例
- `contracts/db-conventions.md` — DB 规范（需补 PG 类型映射）
- `AGENTS.md` — 仓库硬约束（spec 纪律 / 迁移 forward-only / 表名前缀 / 分支提交）
