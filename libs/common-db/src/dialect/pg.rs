/// PostgreSQL 方言
///
/// PG 作为基础方言进入方言矩阵（2026-10-30 设计：docs/superpowers/specs/2026-10-30-postgresql-dialect-support-design.md）。
/// 与 MySQL 的主要差异：
/// 1. 自增使用 `GENERATED ALWAYS AS IDENTITY`（PG 10+，推荐）而非 AUTO_INCREMENT
/// 2. 标识符转义为双引号（与 SQLite/达梦同类）
/// 3. 当前时间用 `now() AT TIME ZONE 'UTC'`（显式 UTC，对齐其余方言语义）
/// 4. 分页与 MySQL 相同（`LIMIT ? OFFSET ?`）
///
/// 保留字来源：PostgreSQL 官方文档 Appendix C「SQL Key Words」reserved 列全表
/// （https://www.postgresql.org/docs/current/sql-keywords-appendix.html）。
pub const PG_RESERVED_WORDS: &[&str] = &[
    "ALL",
    "ANALYSE",
    "ANALYZE",
    "AND",
    "ANY",
    "ARRAY",
    "AS",
    "ASC",
    "ASYMMETRIC",
    "BOTH",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "CONSTRAINT",
    "CREATE",
    "CURRENT_CATALOG",
    "CURRENT_DATE",
    "CURRENT_ROLE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "DEFAULT",
    "DEFERRABLE",
    "DESC",
    "DISTINCT",
    "DO",
    "ELSE",
    "END",
    "EXCEPT",
    "FALSE",
    "FETCH",
    "FOR",
    "FOREIGN",
    "FROM",
    "GRANT",
    "GROUP",
    "HAVING",
    "IN",
    "INITIALLY",
    "INTERSECT",
    "INTO",
    "LATERAL",
    "LEADING",
    "LIMIT",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "NOT",
    "NULL",
    "OFFSET",
    "ON",
    "ONLY",
    "OR",
    "ORDER",
    "PLACING",
    "PRIMARY",
    "REFERENCES",
    "RETURNING",
    "SELECT",
    "SESSION_USER",
    "SOME",
    "SYMMETRIC",
    "TABLE",
    "THEN",
    "TO",
    "TRAILING",
    "TRUE",
    "UNION",
    "UNIQUE",
    "USER",
    "USING",
    "VARIADIC",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
];

/// PG 自增语法
///
/// 使用 SQL 标准的 IDENTITY 列（PG 10+）：
/// CREATE TABLE t (id bigint GENERATED ALWAYS AS IDENTITY, ...);
pub mod identity {
    /// 生成 IDENTITY 列的列定义片段
    pub fn identity_column(column: &str, column_type: &str) -> String {
        format!("{column} {column_type} GENERATED ALWAYS AS IDENTITY")
    }
}
