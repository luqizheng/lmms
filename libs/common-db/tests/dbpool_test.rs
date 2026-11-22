use digua_common_db::dialect::{DialectKind, Paginate};

#[test]
fn dialect_from_sqlite_url() {
    assert_eq!(
        DialectKind::from_url("sqlite::memory:"),
        DialectKind::Sqlite
    );
}

#[test]
fn dialect_from_sqlite_file_url() {
    assert_eq!(
        DialectKind::from_url("sqlite:/data/app.db"),
        DialectKind::Sqlite
    );
}

#[test]
fn dialect_from_mysql_url() {
    assert_eq!(
        DialectKind::from_url("mysql://localhost:3306/db"),
        DialectKind::Mysql
    );
}

#[test]
fn dialect_from_mariadb_url() {
    assert_eq!(
        DialectKind::from_url("mariadb://localhost/db"),
        DialectKind::Mysql
    );
}

#[test]
fn dialect_pg_urls_are_explicit() {
    assert_eq!(
        DialectKind::from_url("postgresql://localhost/db"),
        DialectKind::Postgres
    );
    assert_eq!(
        DialectKind::from_url("postgres://localhost/db"),
        DialectKind::Postgres
    );
}

#[test]
fn dialect_unknown_url_defaults_mysql() {
    assert_eq!(
        DialectKind::from_url("oracle://localhost/db"),
        DialectKind::Mysql
    );
    assert_eq!(
        DialectKind::from_url("some-random-scheme://host/db"),
        DialectKind::Mysql
    );
}

#[test]
fn dialect_pg_has_explicit_utc_and_quoting() {
    let d = DialectKind::Postgres;
    assert_eq!(d.now_expr(), "now() AT TIME ZONE 'UTC'");
    assert_eq!(d.escape_identifier("user"), "\"user\"");
    assert!(d.is_reserved_keyword("SELECT"));
    assert!(!d.is_reserved_keyword("coder_not_reserved"));
}

#[test]
fn dialect_now_expr_sqlite() {
    assert_eq!(DialectKind::Sqlite.now_expr(), "CURRENT_TIMESTAMP");
}

#[test]
fn dialect_now_expr_mysql() {
    assert_eq!(DialectKind::Mysql.now_expr(), "UTC_TIMESTAMP()");
}

#[test]
fn dialect_paginate() {
    assert_eq!(DialectKind::Mysql.paginate(10, 20), "LIMIT 10 OFFSET 20");
    assert_eq!(DialectKind::Sqlite.paginate(10, 20), "LIMIT 10 OFFSET 20");
}
