use digua_common_db::dialect::{DialectKind, Paginate};

#[test]
fn dialect_from_dameng_url() {
    assert_eq!(
        DialectKind::from_url("dameng://user:pwd@host:5236/db"),
        DialectKind::Dameng
    );
}

#[test]
fn dialect_from_mssql_url() {
    assert_eq!(
        DialectKind::from_url("mssql://sa:pwd@host:1433/master"),
        DialectKind::Mssql
    );
}

#[test]
fn dialect_paginate_mysql() {
    assert_eq!(DialectKind::Mysql.paginate(10, 20), "LIMIT 10 OFFSET 20");
}

#[test]
fn dialect_paginate_sqlite() {
    assert_eq!(DialectKind::Sqlite.paginate(10, 20), "LIMIT 10 OFFSET 20");
}

#[test]
fn dialect_paginate_mssql() {
    assert_eq!(
        DialectKind::Mssql.paginate(10, 20),
        "OFFSET 20 ROWS FETCH NEXT 10 ROWS ONLY"
    );
}

#[test]
fn dialect_now_expr() {
    assert_eq!(DialectKind::Mysql.now_expr(), "UTC_TIMESTAMP()");
    assert_eq!(DialectKind::Dameng.now_expr(), "SYSDATE");
    assert_eq!(DialectKind::Sqlite.now_expr(), "CURRENT_TIMESTAMP");
    assert_eq!(DialectKind::Mssql.now_expr(), "GETUTCDATE()");
}

#[test]
fn dialect_escape_identifier_mysql() {
    assert_eq!(DialectKind::Mysql.escape_identifier("user"), "`user`");
    assert_eq!(
        DialectKind::Mysql.escape_identifier("user`name"),
        "`user``name`"
    );
}

#[test]
fn dialect_escape_identifier_sqlite() {
    assert_eq!(DialectKind::Sqlite.escape_identifier("user"), "\"user\"");
    assert_eq!(
        DialectKind::Sqlite.escape_identifier("user\"name"),
        "\"user\"\"name\""
    );
}

#[test]
fn dialect_escape_identifier_mssql() {
    assert_eq!(DialectKind::Mssql.escape_identifier("user"), "[user]");
    assert_eq!(
        DialectKind::Mssql.escape_identifier("user]name"),
        "[user]]name]"
    );
}

#[test]
fn dialect_escape_identifier_dameng() {
    assert_eq!(DialectKind::Dameng.escape_identifier("user"), "\"user\"");
    assert_eq!(
        DialectKind::Dameng.escape_identifier("user\"name"),
        "\"user\"\"name\""
    );
}

#[test]
fn dialect_is_reserved_keyword_mssql() {
    assert!(DialectKind::Mssql.is_reserved_keyword("USER"));
    assert!(DialectKind::Mssql.is_reserved_keyword("SELECT"));
    assert!(DialectKind::Mssql.is_reserved_keyword("TABLE"));
    assert!(!DialectKind::Mssql.is_reserved_keyword("custom_name"));
}

#[test]
fn dialect_is_reserved_keyword_dameng() {
    assert!(DialectKind::Dameng.is_reserved_keyword("USER"));
    assert!(DialectKind::Dameng.is_reserved_keyword("SESSION"));
    assert!(DialectKind::Dameng.is_reserved_keyword("TABLE"));
    assert!(!DialectKind::Dameng.is_reserved_keyword("my_table"));
}

#[test]
fn dialect_is_reserved_keyword_mysql() {
    // MySQL 不检查保留字（通过 mysql_reserved_words crate 处理）
    assert!(!DialectKind::Mysql.is_reserved_keyword("USER"));
}

#[test]
fn dialect_is_reserved_keyword_sqlite() {
    // SQLite 不检查保留字
    assert!(!DialectKind::Sqlite.is_reserved_keyword("TABLE"));
}
