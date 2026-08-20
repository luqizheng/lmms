/// MSSQL (Microsoft SQL Server) 方言
///
/// 主要差异点：
/// 1. 分页：使用 OFFSET ... FETCH NEXT ... ROWS ONLY
/// 2. 自增：使用 IDENTITY(1,1) 而非 AUTO_INCREMENT
/// 3. 布尔：使用 BIT 而非 TINYINT(1)
/// 4. 字符串前缀：N'' for NVARCHAR
/// 5. 标识符转义：[name]
/// 6. 当前时间：GETUTCDATE()
/// 7. TOP 子句：SELECT TOP N 而非 LIMIT
/// 8. 排序规则：COLLATE
///
/// MSSQL 保留字
pub const MSSQL_RESERVED_WORDS: &[&str] = &[
    "ADD",
    "ALL",
    "ALTER",
    "AND",
    "ANY",
    "AS",
    "ASC",
    "AUTHORIZATION",
    "BACKUP",
    "BEGIN",
    "BETWEEN",
    "BREAK",
    "BROWSE",
    "BULK",
    "BY",
    "CASCADE",
    "CASE",
    "CHECK",
    "CHECKPOINT",
    "CLOSE",
    "CLUSTERED",
    "COALESCE",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "COMPUTE",
    "CONSTRAINT",
    "CONTAINS",
    "CONTAINSTABLE",
    "CONTINUE",
    "CONVERT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "CURSOR",
    "DATABASE",
    "DBCC",
    "DEALLOCATE",
    "DECLARE",
    "DEFAULT",
    "DELETE",
    "DENY",
    "DESC",
    "DISK",
    "DISTINCT",
    "DISTRIBUTED",
    "DOUBLE",
    "DROP",
    "DUMP",
    "ELSE",
    "END",
    "ERRLVL",
    "ESCAPE",
    "EXCEPT",
    "EXEC",
    "EXECUTE",
    "EXISTS",
    "EXIT",
    "EXTERNAL",
    "FETCH",
    "FILE",
    "FILLFACTOR",
    "FOR",
    "FOREIGN",
    "FREETEXT",
    "FREETEXTTABLE",
    "FROM",
    "FULL",
    "FUNCTION",
    "GOTO",
    "GRANT",
    "GROUP",
    "HAVING",
    "HOLDLOCK",
    "IDENTITY",
    "IDENTITYCOL",
    "IDENTITY_INSERT",
    "IF",
    "IN",
    "INDEX",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTO",
    "IS",
    "JOIN",
    "KEY",
    "KILL",
    "LEFT",
    "LIKE",
    "LINENO",
    "LOAD",
    "MERGE",
    "NATIONAL",
    "NOCHECK",
    "NONCLUSTERED",
    "NOT",
    "NULL",
    "NULLIF",
    "OF",
    "OFF",
    "OFFSETS",
    "ON",
    "OPEN",
    "OPENDATASOURCE",
    "OPENQUERY",
    "OPENROWSET",
    "OPENXML",
    "OPTION",
    "OR",
    "ORDER",
    "OUTER",
    "OVER",
    "PERCENT",
    "PIVOT",
    "PLAN",
    "PRECISION",
    "PRIMARY",
    "PRINT",
    "PROC",
    "PROCEDURE",
    "PUBLIC",
    "RAISERROR",
    "READ",
    "READTEXT",
    "RECONFIGURE",
    "REFERENCES",
    "REPLICATION",
    "RESTORE",
    "RESTRICT",
    "RETURN",
    "REVERT",
    "REVOKE",
    "RIGHT",
    "ROLLBACK",
    "ROWCOUNT",
    "ROWGUIDCOL",
    "RULE",
    "SAVE",
    "SCHEMA",
    "SECURITYAUDIT",
    "SELECT",
    "SEMANTICKEYPHRASETABLE",
    "SEMANTICSIMILARITYDETAILSTABLE",
    "SEMANTICSIMILARITYTABLE",
    "SESSION_USER",
    "SET",
    "SETUSER",
    "SHUTDOWN",
    "SOME",
    "STATISTICS",
    "SYSTEM_USER",
    "TABLE",
    "TABLESAMPLE",
    "TEXTSIZE",
    "THEN",
    "TO",
    "TOP",
    "TRAN",
    "TRANSACTION",
    "TRIGGER",
    "TRUNCATE",
    "TRY_CONVERT",
    "TSEQUAL",
    "UNION",
    "UNIQUE",
    "UNPIVOT",
    "UPDATE",
    "UPDATETEXT",
    "USE",
    "USER",
    "VALUES",
    "VARYING",
    "VIEW",
    "WAITFOR",
    "WHEN",
    "WHERE",
    "WHILE",
    "WITH",
    "WITHIN GROUP",
    "WRITETEXT",
];

/// 生成 MSSQL 自增主键语法
pub fn identity_sql() -> &'static str {
    "IDENTITY(1,1)"
}

/// 生成 MSSQL 分页 SQL
pub fn paginate_sql(limit: u32, offset: u32) -> String {
    format!("OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, limit)
}

/// 生成 MSSQL TOP 子句
pub fn top_sql(limit: u32) -> String {
    format!("TOP({})", limit)
}

/// 达梦数据库方言
///
/// 主要差异点：
/// 1. 兼容 MySQL 协议（端口 5236）
/// 2. 序列（SEQUENCE）替代自增
/// 3. 关键字差异
/// 4. JSON 函数差异
/// 5. 分页与 MySQL 相同（LIMIT OFFSET）
///
/// 达梦保留字
pub const DAMENG_RESERVED_WORDS: &[&str] = &[
    "USER",
    "SESSION",
    "TABLE",
    "VIEW",
    "INDEX",
    "SEQUENCE",
    "SYNONYM",
    "PROCEDURE",
    "FUNCTION",
    "PACKAGE",
    "TRIGGER",
    "DATABASE",
    "SCHEMA",
    "ROLE",
    "GRANT",
    "REVOKE",
    "COMMIT",
    "ROLLBACK",
    "SAVEPOINT",
    "CONNECT",
    "IDENTIFIED",
    "PROFILE",
    "DEFAULT",
    "TEMP",
    "TEMPORARY",
];

/// 达梦序列语法
pub fn sequence_sql(table: &str, column: &str) -> String {
    format!("CREATE SEQUENCE {table}_{column}_SEQ START WITH 1 INCREMENT BY 1")
}

/// 达梦当前时间
pub fn now_expr() -> &'static str {
    "SYSDATE"
}

/// 达梦 UUID 生成
pub fn uuid_sql() -> &'static str {
    "SYS_GUID()"
}
