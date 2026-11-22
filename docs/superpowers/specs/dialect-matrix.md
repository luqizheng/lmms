# 数据库方言支持矩阵

> LMMS 当前只支持 **MySQL、PostgreSQL、SQLite**。Dameng（达梦）和 Microsoft SQL Server（MSSQL）不在支持范围内；仓库不包含这两种数据库的驱动、方言适配或 CI 测试。若未来出现明确客户需求，再通过独立适配工作重新评估。

## 支持范围

| 数据库 | SQLx driver | Cargo feature | 用途 | CI |
|---|---|---|---|---|
| MySQL | `sqlx-mysql` | `mysql` | 主要生产数据库 | PR |
| PostgreSQL | `sqlx-postgres` | `postgres` | ai-pm 生产及通用后端 | PR |
| SQLite | `sqlx-sqlite` | `sqlite` | 本地及单元测试 | PR |

## 统一约定

- 方言枚举和 URL 识别位于 `libs/common-db/src/dialect`。
- 参数绑定使用 SQLx QueryBuilder / 公共宏，不拼接用户输入。
- PostgreSQL 使用编号占位符（`$1..$N`）；MySQL 和 SQLite 使用匿名占位符（`?`）。
- 当前时间表达式、标识符转义及保留字行为由 `DialectKind` 提供。
- 服务级数据库测试通过 `TEST_MYSQL_HOST/PORT` 与 `TEST_PG_URL` 连接 CI 容器；未配置外部连接时，相关集成测试按各自约定跳过。

## CI 矩阵

` .github/workflows/dialect-matrix.yml ` 对 MySQL、PostgreSQL、SQLite 运行公共数据库 crate 测试，并对当前实际使用的服务运行 MySQL / PostgreSQL 测试。工作流不再包含 Dameng 或 MSSQL job，也不再设置只为这两种数据库服务的每日定时触发。

## 服务 × 方言

| 服务 | MySQL | PostgreSQL | SQLite |
|---|---:|---:|---:|
| prue_backend | ✅ | ✅ | ✅ |
| file-service | ✅ | ✅ | — |
| audit-service | ✅ | ✅ | — |
| token-service | ✅ | ✅ | — |
| notification-service | ✅ | ✅ | — |
| iam-service | ✅ | ✅ | — |
| iam-outbox-publisher | ✅ | ✅ | — |

以上矩阵表示代码配置与测试入口，不替代具体部署环境的集成验收。