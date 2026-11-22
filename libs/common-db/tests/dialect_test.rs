use digua_common_db::dialect::{DialectKind, Paginate};

#[test]
fn dialect_from_mysql_url() {
    assert_eq!(
        DialectKind::from_url("mysql://user:pwd@host:3306/db"),
        Some(DialectKind::Mysql)
    );
}

#[test]
fn dialect_from_mariadb_url_maps_to_mysql() {
    // mariadb 是 MySQL 协议家族的显式 scheme，不是「未知 scheme」。
    assert_eq!(
        DialectKind::from_url("mariadb://localhost/db"),
        Some(DialectKind::Mysql)
    );
}

#[test]
fn dialect_from_sqlite_url() {
    assert_eq!(
        DialectKind::from_url("sqlite::memory:"),
        Some(DialectKind::Sqlite)
    );
}

#[test]
fn dialect_pg_urls_are_explicit() {
    assert_eq!(
        DialectKind::from_url("postgresql://localhost/db"),
        Some(DialectKind::Postgres)
    );
    assert_eq!(
        DialectKind::from_url("postgres://localhost/db"),
        Some(DialectKind::Postgres)
    );
}

#[test]
fn dialect_unsupported_schemes_are_rejected() {
    // Dameng / MSSQL 已移除，与其它未知 scheme 一样必须被明确拒绝（返回 None），
    // 不得静默映射为 MySQL。
    assert_eq!(
        DialectKind::from_url("dameng://user:pwd@host:5236/db"),
        None
    );
    assert_eq!(
        DialectKind::from_url("mssql://sa:pwd@host:1433/master"),
        None
    );
    assert_eq!(DialectKind::from_url("oracle://localhost/db"), None);
    assert_eq!(DialectKind::from_url("some-random-scheme://host/db"), None);
    assert_eq!(DialectKind::from_url(""), None);
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
fn dialect_paginate_postgres() {
    assert_eq!(DialectKind::Postgres.paginate(10, 20), "LIMIT 10 OFFSET 20");
}

#[test]
fn dialect_now_expr() {
    assert_eq!(DialectKind::Mysql.now_expr(), "UTC_TIMESTAMP()");
    assert_eq!(DialectKind::Sqlite.now_expr(), "CURRENT_TIMESTAMP");
    assert_eq!(DialectKind::Postgres.now_expr(), "now() AT TIME ZONE 'UTC'");
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
fn dialect_escape_identifier_postgres() {
    assert_eq!(DialectKind::Postgres.escape_identifier("user"), "\"user\"");
    assert_eq!(
        DialectKind::Postgres.escape_identifier("user\"name"),
        "\"user\"\"name\""
    );
}

#[test]
fn dialect_is_reserved_keyword_postgres() {
    assert!(DialectKind::Postgres.is_reserved_keyword("USER"));
    assert!(DialectKind::Postgres.is_reserved_keyword("SELECT"));
    assert!(DialectKind::Postgres.is_reserved_keyword("TABLE"));
    assert!(!DialectKind::Postgres.is_reserved_keyword("custom_name"));
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
