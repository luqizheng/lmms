# PostgreSQL 基础方言支持设计（libs 基座层）

- 日期：2026-10-30
- 状态：draft（brainstorming 产出，待批准）
- 范围：lmms 基座 crate 层（libs/dialect + libs/common-db + CI 方言矩阵 + 文档）

## 一、意图与背景

用户裁决（2026-10-30）：**PostgreSQL 作为基础数据库支持**——进入 `docs/superpowers/specs/dialect-matrix.md` 的正式方言序列（与 MySQL/SQLite 同级），初始动因是 ai-pm 的 PG+FTS 技术栈（ai-pm 将以 crate 引用方式消费 lmms 基座），定位从「ai-pm 专用」升级为「基础支持」。

现状（实测）：

- `libs/common-db/src/dialect/`：`DialectKind = { Mysql, Dameng, Sqlite, Mssql }`，达梦/MSSQL 是**完整预留实现**（dameng.rs 含保留字/自增序列等），是新增方言的现成模式。
- `postgresql://` URL 当前**静默回退 Mysql**（`from_url` 的 else 分支），且被 `libs/common-db/tests/dbpool_test.rs:46` 的断言钉死；token-service config 测试也钉死 `postgres` db_type「未知必须忽略」。
- `common-db` features：`mysql = ["sqlx/mysql"]`、`sqlite`（默认）、`dameng = ["sqlx/mysql"]`；`DbPool` 枚举按 cfg 门控 Mysql/Sqlite 臂。
- workspace sqlx 0.8，`default-features = false, features = ["runtime-tokio", "tls-rustls-aws-lc-rs"]`——PG 驱动（sqlx/postgres）为纯 Rust，不引入新的原生编译依赖，Windows MSVC 路径不受影响（host-native 构建已打通）。
- CI：`.github/workflows/dialect-matrix.yml` 已有 SQLite/MySQL（PR 级）与 Dameng/MSSQL（每日）四档先例。
- 本机已有 ai-pm 开发 PG 容器（`aipm-pg-dev:5433`）；lmms 测试容器惯例为共享常开容器（coder-test-mysql:33306 / coder-test-redis:36379，用户指示测试时不断开）。

## 二、目标

1. **`DialectKind::Postgres` 完整方言**（`dialect/pg.rs`，模式照抄 dameng.rs）：
   - `from_url`：`postgres://` 与 `postgresql://` → `Postgres`（**不再回退 Mysql**）；`mssql://`/`oracle://` 等未知前缀维持回退 Mysql 不变。
   - `now_expr`、`escape_identifier`（`"id"` 双写，与 SQLite/达梦同类）、保留字清单、`auto_increment`（PG 语法）、分页沿用通用 `LIMIT ? OFFSET ?`。
2. **common-db `postgres` feature**：`postgres = ["sqlx/postgres"]`，`DbPool::Postgres` cfg 臂 + 连接/探活实现；**不进 default features**（默认构建不变）。
3. **行为变更显式化**：`dbpool_test` 中 `postgresql://` 断言从「回退 Mysql」改为「= Postgres」；`mssql://`、`oracle://` 回退断言保留。
4. **CI**：dialect-matrix workflow 增加 `test-postgres` job，**PR 级**（postgres:16 service container），`cargo test --package common-db --features postgres`。
5. **本地测试容器**：`coder-test-pg`（postgres:16，端口 **5434**，与 aipm-pg-dev:5433 隔离），共享常开；测试环境变量 `TEST_PG_URL`（缺省时 PG 相关测试跳过而非失败，与 MySQL 现行为一致），随 batch-1 的 `coder` 体系无冲突。
6. **文档**：dialect-matrix.md 支持表新增 PG 行（驱动 sqlx-postgres、方言键 `postgres`、用途「ai-pm 生产 + 通用基础」、测试策略 PR 级别）；差异矩阵补 PG 列。
7. **版本**：随下一次 libs 统一版本发布走（`coder release publish rust`），不单独发版。

## 三、非目标

- lmms 各服务（token-service 等）的 PG **运行时支持**：服务保持 MySQL，迁移体系不动；`token-service` 的 `DbType` 不加 postgres 变体（现有「未知 db_type 忽略」测试保留）。
- ai-pm 仓内的 PG 迁移与 FTS 能力（归 ai-pm 仓，其通过 crate feature 消费本能力）。
- 生产部署编排（docker-compose）增加 PG 实例——PG 目前只有基座测试与 ai-pm 使用场景。

## 四、反目标（出现即失败）

- 既有 `--features sqlite` / `--features mysql` 测试任何一条回归。
- default features 构建（无 postgres）产物变化或编译受影响。
- `postgresql://` 在任何路径下静默当作 MySQL（必须显式判定）。
- 共享测试容器被测试流程启停（用户指示：常开不断）。

## 五、约束

- sqlx 0.8 workspace 统一版本；postgres 仅以 feature gate 引入，`[features]` 命名沿用现有小写惯例。
- Windows 原生 MSVC 编译路径不得回归（PG 驱动纯 Rust，理论上无影响，需实测确认）。
- 方言实现的文档风格沿用 dameng.rs（中文注释、差异说明、保留字表）。
- 测试跳过机制：无 `TEST_PG_URL` 时 `#[ignore]` 语义或运行时判断，CI 与本地有容器时全量跑。

## 六、已裁决事项

| # | 裁决 | 内容 |
|---|---|---|
| 1 | 定位（2026-10-30 用户，两次收敛） | 先「ai-pm 专用」，后升级为**基础数据库支持**（入方言矩阵正式序列） |
| 2 | 范围 | 基座层：dialect + common-db + CI 矩阵 + 文档；服务运行时 PG 化不在本轮 |
| 3 | 迁移归属 | lmms 迁移体系不动；ai-pm 迁移归 ai-pm 仓 |

## 七、留给建造者的决策

- `pg.rs` 的 PG 保留字清单与依据（PG 官方 keyword 表裁剪）。
- auto_increment 语法选型：`GENERATED ALWAYS AS IDENTITY`（推荐，PG 10+）vs `SERIAL`，及 `create_auto_increment` 辅助函数形态。
- `DbPool::Postgres` 臂的连接选项与 `connect` 函数签名（对照现有 Mysql/Sqlite 臂）。
- PG 测试用例清单（连接、方言往返、保留字转义、分页）与 `TEST_PG_URL` 缺省跳过的具体实现方式。
- CI job 的 cargo 缓存与 service container 健康等待细节。
- `test-kit` feature 是否连带纳入 postgres（现 `test-kit = ["sqlite", ...]`）。

## 八、验收判据

1. `cargo test -p common-db --features postgres` 在本地（coder-test-pg 在跑）全绿；无 `TEST_PG_URL` 时相关测试跳过、其余全绿。
2. `cargo test -p common-db --features sqlite` 与 `--features mysql` 既有结果不回归。
3. `from_url` 新旧行为断言齐备：postgres/postgresql → Postgres；sqlite/dameng/mssql → 原判定；未知 → Mysql。
4. dialect-matrix workflow 新 job 在 PR 上真实跑过一次（或本地 `act` 等价验证语法）。
5. dialect-matrix.md 文档与实现一致（PG 行 + 差异列）。
