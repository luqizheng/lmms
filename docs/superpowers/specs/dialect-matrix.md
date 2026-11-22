# 数据库方言差异矩阵

> **STATUS: 📖 参考文档** — 记录 MySQL / PostgreSQL / SQLite 方言差异。
> 当前生产使用 MySQL，SQLite 用于 prue_backend 单测，PostgreSQL 为基础方言（2026-10-30 起，
> 服务 ai-pm 的 PG+FTS 技术栈并作为通用基础支持）。

## 支持范围（2026-11-22 裁决）

**仅支持 MySQL / PostgreSQL / SQLite 三种方言。**

Dameng（达梦）与 MSSQL 已从 `libs/common-db` 移除（枚举分支、`dialect/dameng.rs`、
`dialect/mssql.rs`、`dameng` feature、CI job 全部删除）：

- 达梦：Rust 生态没有可维护的驱动方案（社区方案不成熟、`sqlx` 无官方支持）。
- MSSQL：当前没有客户使用；未来有实际需求时再按当时的技术选型重新评估。
- `DialectKind::from_url` 对未知 scheme（含 `dameng:` / `mssql:`）返回 `None`，
  `init_pool*` 以 `DB_URL_SCHEME` 错误显式报错，**绝不静默回退 MySQL**。
- 测试保留 `dameng://` / `mssql://` 字符串，用于验证 unsupported scheme 被正确拒绝。

## 支持的数据库

| 数据库 | 驱动 | 方言键 | 用途 | 测试策略 |
|--------|------|--------|------|----------|
| MySQL | sqlx-mysql | `mysql` | 生产 | PR 级别 |
| SQLite | sqlx-sqlite | `sqlite` | 单测专用 | PR 级别 |
| PostgreSQL | sqlx-postgres | `postgres` | ai-pm 生产 + 通用基础 | PR 级别 |

## 方言差异对比

### 1. 分页语法

| 数据库 | 语法 | 示例 |
|--------|------|------|
| MySQL | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |
| SQLite | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |
| PostgreSQL | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |

### 2. 自增主键

| 数据库 | 语法 |
|--------|------|
| MySQL | `AUTO_INCREMENT` |
| SQLite | `AUTOINCREMENT` |
| PostgreSQL | `GENERATED ALWAYS AS IDENTITY`（PG 10+） |

### 3. 当前时间

| 数据库 | 函数 |
|--------|------|
| MySQL | `UTC_TIMESTAMP()` |
| SQLite | `CURRENT_TIMESTAMP` |
| PostgreSQL | `now() AT TIME ZONE 'UTC'` |

### 4. 标识符转义

| 数据库 | 语法 | 示例 |
|--------|------|------|
| MySQL | `` `name` `` | `` `user` `` |
| SQLite | `"name"` | `"user"` |
| PostgreSQL | `"name"`（双写转义） | `"user"` |

### 5. 布尔字面量

| 数据库 | True | False |
|--------|------|-------|
| MySQL | `1` / `TRUE` | `0` / `FALSE` |
| SQLite | `1` | `0` |
| PostgreSQL | `TRUE` / `BOOLEAN` 列 | `FALSE` |

### 6. UUID 生成

| 数据库 | 函数 |
|--------|------|
| MySQL | `UUID()` |
| SQLite | `lower(hex(randomblob(16)))` |
| PostgreSQL | `gen_random_uuid()`（pgcrypto / PG13+ 内置） |

### 7. JSON 函数

| 数据库 | 提取值 | 检查路径 |
|--------|--------|----------|
| MySQL | `JSON_EXTRACT(col, '$.key')` | `JSON_CONTAINS_PATH(col, 'one', '$.key')` |
| SQLite | `json_extract(col, '$.key')` | `json_each(col, '$.key')` |
| PostgreSQL | `col->>'key'` / `json_extract_path_text` | `jsonb ? 'key'` / `json_extract_path` |

### 8. 保留字

| 数据库 | 额外保留字 |
|--------|-----------|
| MySQL | 标准 SQL 保留字（`is_reserved_keyword` 返回 false，交由上层处理） |
| SQLite | 标准 SQL 保留字（同上） |
| PostgreSQL | `pg::PG_RESERVED_WORDS` 查表 |

## 使用指南

### 业务代码

```rust
use digua_common_db::dialect::DialectKind;

match pool.dialect() {
    DialectKind::Postgres => {
        // PG 特定逻辑（ON CONFLICT / STRING_AGG / FTS …）
    }
    DialectKind::Mysql | DialectKind::Sqlite => {
        // `?` 占位符家族
    }
}
```

### 迁移文件

```
migrations/<service>/
├── mysql/       # MySQL 专用
│   └── V001__init.sql
├── postgres/    # PostgreSQL 专用
│   └── V001__init.sql
└── sqlite/      # SQLite 专用（仅 prue_backend 要求双份）
    └── V001__init.sql
```

### URL 前缀识别

```rust
use digua_common_db::dialect::DialectKind;

DialectKind::from_url("mysql://...");       // -> Some(Mysql)
DialectKind::from_url("mariadb://...");     // -> Some(Mysql)（MySQL 协议家族）
DialectKind::from_url("postgres://...");    // -> Some(Postgres)
DialectKind::from_url("postgresql://...");  // -> Some(Postgres)
DialectKind::from_url("sqlite:...");        // -> Some(Sqlite)
DialectKind::from_url("dameng://...");      // -> None（明确拒绝）
DialectKind::from_url("mssql://...");       // -> None（明确拒绝）
DialectKind::from_url("oracle://...");      // -> None（未知 scheme 一律拒绝）
```

## 测试矩阵

| 触发条件 | MySQL | SQLite | PostgreSQL |
|----------|-------|--------|------------|
| PR（dialect-matrix CI job） | ✓ | ✓ | ✓ |
| 服务级集成（test-services / test-services-postgres） | ✓ | - | ✓ |

## 服务 × 方言支持矩阵（2026-11 服务层全量 PG 支持后）

| 服务 | MySQL | PostgreSQL | SQLite | PG 测试入口 |
|------|-------|------------|--------|-------------|
| prue_backend | ✅ | ✅ | ✅ | 单测（`--features mysql,postgres`） |
| file-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| audit-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| token-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| notification-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| iam-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| iam-outbox-publisher | ✅ | ✅ | - | 随 iam（池/repo 方言通用，归档全链在 `libs/common-event/tests/outbox_pg_test.rs`） |

- ✅ = 双后端编译通过 + 双后端集成测试通过（`--features postgres` + `TEST_PG_URL` 外部容器模式，未设时 `[skip]`）。
- MySQL 侧为 default feature，既有共享容器测试（`TEST_MYSQL_HOST/PORT`）零改动。
- 方言差异收敛两层：`libs/common-db` 的 `sql::render` + `db_*!` 宏（SQL 单份源码）；运行时无法统一的点（`ON CONFLICT`/`STRING_AGG`/advisory lock 等）在服务层显式 match 双臂并带注释。详见 `contracts/db-conventions.md` 的 PG 类型映射章节。
- config-service 已于 2026-11-06 移除，不在矩阵中。
