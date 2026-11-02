# 数据库方言差异矩阵

> **STATUS: 📖 参考文档** — 记录 MySQL / 达梦 / MSSQL / SQLite 方言差异。
> 当前生产使用 MySQL，SQLite 用于 prue_backend 单测，PostgreSQL 为基础方言（2026-10-30 起，
> 服务 ai-pm 的 PG+FTS 技术栈并作为通用基础支持）。达梦 / MSSQL 为未来信创预留。

## 支持的数据库

| 数据库 | 驱动 | 方言键 | 用途 | 测试策略 |
|--------|------|--------|------|----------|
| MySQL | sqlx-mysql | `mysql` | 生产 | PR 级别 |
| 达梦 | sqlx-mysql（DM MySQL 兼容模式） | `dameng` | 信创生产 | 每日定时 |
| MSSQL | sqlx-mssql | `mssql` | 生产 | 每日定时 |
| SQLite | sqlx-sqlite | `sqlite` | 单测专用 | PR 级别 |
| PostgreSQL | sqlx-postgres | `postgres` | ai-pm 生产 + 通用基础 | PR 级别 |

## 方言差异对比

### 1. 分页语法

| 数据库 | 语法 | 示例 |
|--------|------|------|
| MySQL | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |
| 达梦 | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |
| MSSQL | `OFFSET ? ROWS FETCH NEXT ? ROWS ONLY` | `OFFSET 20 ROWS FETCH NEXT 10 ROWS ONLY` |
| SQLite | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |
| PostgreSQL | `LIMIT ? OFFSET ?` | `LIMIT 10 OFFSET 20` |

### 2. 自增主键

| 数据库 | 语法 |
|--------|------|
| MySQL | `AUTO_INCREMENT` |
| 达梦 | `SEQUENCE` (需单独创建) |
| MSSQL | `IDENTITY(1,1)` |
| SQLite | `AUTOINCREMENT` |
| PostgreSQL | `GENERATED ALWAYS AS IDENTITY`（PG 10+） |

### 3. 当前时间

| 数据库 | 函数 |
|--------|------|
| MySQL | `UTC_TIMESTAMP()` |
| 达梦 | `SYSDATE` (或 `UTC_TIMESTAMP()`) |
| MSSQL | `GETUTCDATE()` |
| SQLite | `CURRENT_TIMESTAMP` |
| PostgreSQL | `now() AT TIME ZONE 'UTC'` |

### 4. 标识符转义

| 数据库 | 语法 | 示例 |
|--------|------|------|
| MySQL | `` `name` `` | `` `user` `` |
| 达梦 | `"name"` | `"user"` |
| MSSQL | `[name]` | `[user]` |
| SQLite | `"name"` | `"user"` |
| PostgreSQL | `"name"`（双写转义） | `"user"` |

### 5. 布尔字面量

| 数据库 | True | False |
|--------|------|-------|
| MySQL | `1` / `TRUE` | `0` / `FALSE` |
| 达梦 | `1` | `0` |
| MSSQL | `1` / `TRUE` | `0` / `FALSE` |
| SQLite | `1` | `0` |

### 6. UUID 生成

| 数据库 | 函数 |
|--------|------|
| MySQL | `UUID()` |
| 达梦 | `SYS_GUID()` |
| MSSQL | `NEWID()` |
| SQLite | `lower(hex(randomblob(16)))` |

### 7. JSON 函数

| 数据库 | 提取值 | 检查路径 |
|--------|--------|----------|
| MySQL | `JSON_EXTRACT(col, '$.key')` | `JSON_CONTAINS_PATH(col, 'one', '$.key')` |
| 达梦 | `JSON_VALUE(col, '$.key')` | `JSON_EXISTS(col, '$.key')` |
| MSSQL | `JSON_VALUE(col, '$.key')` | `JSON_EXISTS(col, '$.key')` |
| SQLite | `json_extract(col, '$.key')` | `json_each(col, '$.key')` |

### 8. 保留字

| 数据库 | 额外保留字 |
|--------|-----------|
| MySQL | 标准 SQL 保留字 |
| 达梦 | USER, SESSION, TABLE, VIEW, INDEX, SEQUENCE, ... |
| MSSQL | USER, GROUP, ORDER, TABLE, SELECT, WHERE, FROM, ... |
| SQLite | 标准 SQL 保留字 |

## 使用指南

### 业务代码

```rust
use digua_common_db::dialect::DialectKind;

match pool.dialect() {
    DialectKind::Mssql => {
        // MSSQL 特定逻辑
        let sql = format!("SELECT TOP {} * FROM users", limit);
    }
    _ => {
        // MySQL/Dameng/SQLite
        let sql = format!("SELECT * FROM users LIMIT {}", limit);
    }
}
```

### 迁移文件

```
migrations/<service>/
├── mysql/       # MySQL + 达梦共用
│   └── V001__init.sql
├── mssql/      # MSSQL 专用
│   └── V001__init.sql
└── sqlite/     # SQLite 专用
    └── V001__init.sql
```

### URL 前缀识别

```rust
use digua_common_db::DbPool;

let pool = DbPool::from_url("mysql://...");   // -> Mysql
let pool = DbPool::from_url("dameng://...");  // -> Dameng (MySQL 驱动)
let pool = DbPool::from_url("mssql://...");   // -> Mssql
let pool = DbPool::from_url("sqlite://...");  // -> Sqlite
```

## 测试矩阵

| 触发条件 | MySQL | 达梦 | MSSQL | SQLite | PostgreSQL |
|----------|-------|------|-------|--------|------------|
| PR 合并 | ✓ | - | - | ✓ | ✓（dialect-matrix CI job） |
| 代码变更 | ✓ | - | - | ✓ | ✓ |
| 每日定时 | ✓ | ✓ | ✓ | ✓ | - |
| `[dameng]` commit | ✓ | ✓ | - | ✓ | - |
| `[mssql]` commit | ✓ | - | ✓ | ✓ | - |

## 服务 × 方言支持矩阵（2026-11 服务层全量 PG 支持后）

| 服务 | MySQL | PostgreSQL | SQLite | PG 测试入口 |
|------|-------|------------|--------|-------------|
| prue_backend | ✅ | ✅ | ✅ | 单测（`--features mysql,postgres`） |
| config-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| file-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| audit-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| token-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| notification-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| iam-service | ✅ | ✅ | - | `tests/pg_roundtrip_test.rs` |
| iam-outbox-publisher | ✅ | ✅ | - | 随 iam（池/repo 方言通用，归档全链在 `libs/common-event/tests/outbox_pg_test.rs`） |

- ✅ = 双后端编译通过 + 双后端集成测试通过（`--features postgres` + `TEST_PG_URL` 外部容器模式，未设时 `[skip]`）。
- MySQL 侧为 default feature，既有共享容器测试（`TEST_MYSQL_HOST/PORT`）零改动。
- 方言差异收敛两层：`libs/common-db` 的 `sql::render` + `db_*!` 宏（SQL 单份源码）；运行时无法统一的点（`ON CONFLICT`/`STRING_AGG`/advisory lock 等）在服务层显式 match 双臂并带注释。详见 `contracts/db-conventions.md` 的 PG 类型映射章节。

## 已知差异

### 达梦

1. **自增**：需要创建 SEQUENCE，不能直接用 `AUTO_INCREMENT`
2. **关键字**：USER、SESSION 等是保留字
3. **端口**：默认 5236（非 3306）

### MSSQL

1. **分页**：必须用 `OFFSET ... FETCH` 语法
2. **TOP**：可以用 `SELECT TOP(n)` 替代 `LIMIT`
3. **标识符**：使用 `[name]` 而非 `` `name` ``
4. **驱动**：使用 `sqlx-mssql` 而非 `sqlx-mysql`
