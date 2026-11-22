//! `sql::render` 的回归测试（设计文档 §6.5 测试表）。
//!
//! 这些用例是**顺序契约**与**字面量护栏**的守门人：
//! - 01~04 / 17~18：占位符编号与 bind 计数契约；
//! - 05~16：字面量 / 注释 / dollar-quote 的状态机迁移（`?` 落在字面量内必须报错）。

use digua_common_db::dialect::DialectKind;
use digua_common_db::sql::{count_placeholders, render, LiteralKind, SqlRenderError};

fn pg(sql: &str) -> Result<String, SqlRenderError> {
    let rq = render(sql, DialectKind::Postgres)?;
    let expected = rq.expected_binds();
    Ok(rq.expect_binds(expected)?.sql().to_string())
}

#[test]
fn t01_pg_numbered_in_order() {
    let rq = render(
        "SELECT * FROM t WHERE a = ? AND b = ? AND c = ?",
        DialectKind::Postgres,
    )
    .unwrap();
    assert_eq!(rq.expected_binds(), 3);
    assert_eq!(
        rq.sql(),
        "SELECT * FROM t WHERE a = $1 AND b = $2 AND c = $3"
    );
}

#[test]
fn t02_mysql_borrowed_and_still_counted() {
    let rq = render("SELECT * FROM t WHERE a = ? AND b = ?", DialectKind::Mysql).unwrap();
    assert_eq!(rq.sql(), "SELECT * FROM t WHERE a = ? AND b = ?");
    assert!(matches!(rq.sql_cow(), std::borrow::Cow::Borrowed(_))); // 零分配
    assert_eq!(rq.expected_binds(), 2); // MySQL 也计数 → 封堵静默错位
}

#[test]
fn t03_sqlite_unchanged() {
    assert_eq!(pg("SELECT ?").unwrap(), "SELECT $1");
    assert_eq!(
        render("SELECT ?", DialectKind::Sqlite).unwrap().sql(),
        "SELECT ?"
    );
}

#[test]
fn t04_bind_count_mismatch_at_construction() {
    let rq = render(
        "SELECT * FROM t WHERE a = ? AND b = ?",
        DialectKind::Postgres,
    )
    .unwrap();
    let e = rq.expect_binds(1).unwrap_err();
    assert!(matches!(
        e,
        SqlRenderError::PlaceholderCountMismatch {
            expected: 2,
            actual: 1,
            ..
        }
    ));
}

#[test]
fn t05_placeholder_in_single_quote() {
    let e = render("SELECT '?' AS a", DialectKind::Postgres).unwrap_err();
    assert!(matches!(e, SqlRenderError::PlaceholderInLiteral { .. }));
}

#[test]
fn t06_escaped_quote_stays_in_string() {
    // 'it''s ?' —— '' 是转义，不得提前退出字符串态
    assert!(matches!(
        render("SELECT 'it''s ?' AS a", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}

#[test]
fn t07_string_closes_then_placeholder_counts() {
    assert_eq!(pg("SELECT 'a' , ? ").unwrap(), "SELECT 'a' , $1 ");
}

#[test]
fn t08_double_quoted_ident_skipped() {
    assert_eq!(
        pg(r#"SELECT "a?b" FROM t WHERE id = ?"#).unwrap(),
        r#"SELECT "a?b" FROM t WHERE id = $1"#
    );
}

#[test]
fn t09_backtick_ident_skipped_mysql() {
    let rq = render("SELECT `a?b` FROM t WHERE id = ?", DialectKind::Mysql).unwrap();
    assert_eq!(rq.expected_binds(), 1);
}

#[test]
fn t10_line_comment() {
    assert!(matches!(
        render("-- ?\nSELECT 1", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}

#[test]
fn t11_block_comment() {
    assert!(matches!(
        render("/* ? */ SELECT 1", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}

#[test]
fn t12_dollar_quoted() {
    assert!(matches!(
        render("SELECT $$ ? $$ , ?", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}

#[test]
fn t13_unterminated_string() {
    assert!(matches!(
        render("SELECT 1 WHERE x = 'unclosed", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::UnterminatedLiteral {
            kind: LiteralKind::SingleQuote,
            ..
        }
    ));
}

#[test]
fn t14_unterminated_comment() {
    assert!(matches!(
        render("SELECT 1 /* unclosed", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::UnterminatedLiteral {
            kind: LiteralKind::BlockComment,
            ..
        }
    ));
}

#[test]
fn t15_existing_dollar_n_not_recounted() {
    let rq = render("SELECT 1 WHERE a = ? AND b = $1", DialectKind::Postgres).unwrap();
    assert_eq!(rq.sql(), "SELECT 1 WHERE a = $1 AND b = $1");
    assert_eq!(rq.expected_binds(), 1);
}

#[test]
fn t16_like_placeholder_regression_guard() {
    assert!(matches!(
        render(
            "SELECT * FROM t WHERE name LIKE '?%' AND id = ?",
            DialectKind::Postgres
        )
        .unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}

#[test]
fn t17_compact_multi_pg() {
    assert_eq!(
        pg("INSERT INTO t VALUES (?,?,?,?)").unwrap(),
        "INSERT INTO t VALUES ($1,$2,$3,$4)"
    );
}

#[test]
fn t18_compact_multi_mysql_counts() {
    assert_eq!(
        render("INSERT INTO t VALUES (?,?,?,?)", DialectKind::Mysql)
            .unwrap()
            .expected_binds(),
        4
    );
}

#[test]
fn t19_error_echoes_raw_sql_and_dialect() {
    let e = render("SELECT '?'", DialectKind::Postgres)
        .unwrap_err()
        .to_string();
    assert!(e.contains("SELECT '?'"));
    assert!(e.contains("postgres"));
}

/// 对同一批纯占位符 SQL，我方 render 的产物必须与 sqlx QueryBuilder 的产物逐字节一致。
///
/// 这条测试的价值：把「我方重写逻辑是否正确」的判定权交给 sqlx 自己，而不是靠审阅猜测。
/// 若未来 sqlx 升级改变了 `format_placeholder` 行为，这条测试会**第一个红**。
///
/// 需要 PG 后端的 sqlx 类型，故按 `postgres` feature 门控（否则 `--features mysql`
/// 的构建会因为 `sqlx::Postgres` 不存在而编译失败）。
#[cfg(feature = "postgres")]
#[test]
fn render_matches_sqlx_query_builder() {
    // `Query::sql()` 来自 `sqlx::Execute` trait，需在作用域内。
    use sqlx::Execute;

    // 主理人实测 [5] 的原始用例
    let ours = render("SELECT ?::text AS a, ?::text AS b", DialectKind::Postgres).unwrap();
    assert_eq!(ours.sql(), "SELECT $1::text AS a, $2::text AS b");
    assert_eq!(ours.expected_binds(), 2);

    // 与 QueryBuilder 对拍：QueryBuilder::push_bind 内部就是 format_placeholder
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("");
    qb.push("SELECT ")
        .push_bind("a")
        .push("::text AS a, ")
        .push_bind("b")
        .push("::text AS b");
    let built = qb.build();
    assert_eq!(
        built.sql(),
        ours.sql(),
        "我方 render 必须与 sqlx QueryBuilder 产物一致"
    );
}

/// `count_placeholders` 只计数、不改写，且与 `render` 的计数必须一致。
#[test]
fn count_placeholders_matches_render_count() {
    for dialect in [
        DialectKind::Mysql,
        DialectKind::Sqlite,
        DialectKind::Postgres,
    ] {
        let sql = "SELECT * FROM t WHERE a = ? AND b = ?";
        assert_eq!(
            count_placeholders(sql, dialect).unwrap(),
            render(sql, dialect).unwrap().expected_binds(),
            "count_placeholders 与 render 的计数必须一致（dialect={dialect:?}）"
        );
    }
    // 字面量内的 `?` 不计入占位符，且必须报错（而不是被静默跳过）。
    assert!(matches!(
        count_placeholders("SELECT '?' , ?", DialectKind::Postgres).unwrap_err(),
        SqlRenderError::PlaceholderInLiteral { .. }
    ));
}
