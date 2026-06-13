use common_db::dialect::{DialectKind, Paginate};

#[test]
fn sqlite_pagination_uses_limit_offset() {
    assert_eq!(DialectKind::Sqlite.paginate(20, 40), "LIMIT 20 OFFSET 40");
}

#[test]
fn detect_from_url_picks_sqlite() {
    assert_eq!(DialectKind::from_url("sqlite::memory:"), DialectKind::Sqlite);
}

#[test]
fn detect_from_url_picks_dameng() {
    assert_eq!(DialectKind::from_url("dameng://user:pwd@host:5236/db"), DialectKind::Dameng);
}

#[test]
fn detect_from_url_defaults_to_mysql() {
    assert_eq!(DialectKind::from_url("mysql://localhost/db"), DialectKind::Mysql);
    assert_eq!(DialectKind::from_url("postgres://localhost/db"), DialectKind::Mysql);
    assert_eq!(DialectKind::from_url("unknown://localhost/db"), DialectKind::Mysql);
    assert_eq!(DialectKind::from_url("mssql://localhost/db"), DialectKind::Mysql);
}

#[test]
fn now_expr_returns_correct_sql() {
    assert_eq!(DialectKind::Mysql.now_expr(), "UTC_TIMESTAMP()");
    assert_eq!(DialectKind::Dameng.now_expr(), "UTC_TIMESTAMP()");
    assert_eq!(DialectKind::Sqlite.now_expr(), "CURRENT_TIMESTAMP");
}

#[test]
fn dialect_kind_is_eq_and_hashable() {
    let a = DialectKind::Mysql;
    let b = DialectKind::Mysql;
    let c = DialectKind::Sqlite;
    assert_eq!(a, b);
    assert_ne!(a, c);
    let _ = std::collections::HashSet::from([a, c]);
}

#[test]
fn all_dialects_use_same_pagination() {
    assert_eq!(DialectKind::Mysql.paginate(10, 5), "LIMIT 10 OFFSET 5");
    assert_eq!(DialectKind::Dameng.paginate(10, 5), "LIMIT 10 OFFSET 5");
    assert_eq!(DialectKind::Sqlite.paginate(10, 5), "LIMIT 10 OFFSET 5");
}