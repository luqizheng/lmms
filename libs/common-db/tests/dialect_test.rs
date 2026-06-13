use common_db::dialect::{DialectKind, Paginate};

#[test]
fn sqlite_pagination_uses_limit_offset() {
    assert_eq!(DialectKind::Sqlite.paginate(20, 40), "LIMIT 20 OFFSET 40");
}

#[test]
fn mssql_pagination_uses_offset_fetch() {
    assert_eq!(DialectKind::Mssql.paginate(20, 40), "OFFSET 40 ROWS FETCH NEXT 20 ROWS ONLY");
}

#[test]
fn detect_from_url_picks_sqlite() {
    assert_eq!(DialectKind::from_url("sqlite::memory:"), DialectKind::Sqlite);
}

#[test]
fn detect_from_url_picks_dameng() {
    assert_eq!(DialectKind::from_url("dameng://user:pwd@host:5236/db"), DialectKind::Dameng);
}