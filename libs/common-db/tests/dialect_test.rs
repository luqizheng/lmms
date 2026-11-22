use digua_common_db::dialect::{DialectKind, Paginate};

#[test]
fn supported_database_urls_are_detected() {
    assert_eq!(DialectKind::from_url("mysql://localhost/db"), Some(DialectKind::Mysql));
    assert_eq!(DialectKind::from_url("sqlite::memory:"), Some(DialectKind::Sqlite));
    assert_eq!(DialectKind::from_url("postgres://localhost/db"), Some(DialectKind::Postgres));
    assert_eq!(DialectKind::from_url("postgresql://localhost/db"), Some(DialectKind::Postgres));
}

#[test]
fn unsupported_database_urls_are_rejected() {
    assert_eq!(DialectKind::from_url("dameng://localhost/db"), None);
    assert_eq!(DialectKind::from_url("mssql://localhost/db"), None);
    assert_eq!(DialectKind::from_url("oracle://localhost/db"), None);
    assert_eq!(DialectKind::from_url(""), None);
}

#[test]
fn pagination_is_shared_by_supported_dialects() {
    for dialect in [DialectKind::Mysql, DialectKind::Sqlite, DialectKind::Postgres] {
        assert_eq!(dialect.paginate(10, 20), "LIMIT 10 OFFSET 20");
    }
}

#[test]
fn current_time_expressions_match_supported_dialects() {
    assert_eq!(DialectKind::Mysql.now_expr(), "UTC_TIMESTAMP()");
    assert_eq!(DialectKind::Sqlite.now_expr(), "CURRENT_TIMESTAMP");
    assert_eq!(DialectKind::Postgres.now_expr(), "now() AT TIME ZONE 'UTC'");
}

#[test]
fn identifier_escaping_matches_supported_dialects() {
    assert_eq!(DialectKind::Mysql.escape_identifier("user"), "`user`");
    assert_eq!(DialectKind::Sqlite.escape_identifier("user"), "\"user\"");
    assert_eq!(DialectKind::Postgres.escape_identifier("user"), "\"user\"");
}

#[test]
fn postgres_reserved_keywords_are_recognized() {
    assert!(DialectKind::Postgres.is_reserved_keyword("SELECT"));
    assert!(!DialectKind::Mysql.is_reserved_keyword("USER"));
    assert!(!DialectKind::Sqlite.is_reserved_keyword("TABLE"));
}
