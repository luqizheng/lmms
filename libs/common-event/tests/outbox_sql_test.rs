//! outbox 三条 SQL 的**纯构造断言**（不需要任何数据库 / 容器）。
//!
//! 这三条 SQL 是整个 outbox 的方言差异集中点，把它们做成纯函数就是为了在这里
//! 逐字符锁死——不需要容器即可在任意 CI 环境跑。
//!
//! 关键断言：
//! - `enqueue` 的 MySQL/PG 分叉；
//! - `claim_batch` 的 `LIMIT ?` 在 PG 下变成 `LIMIT $1`，且 `FOR UPDATE SKIP LOCKED` 保留；
//! - `mark_published` 的 `CURRENT_TIMESTAMP(3)` **不得**被改写成 `now()`；
//! - 动态 `IN (?, ?, ?)` 在 PG 下变成 `IN ($1, $2, $3)`，且编号顺序 = ids 顺序。

use digua_common_db::dialect::DialectKind;
use digua_common_db::sql::render;
use digua_common_event::outbox::{claim_batch_sql, enqueue_sql, mark_published_sql};

const TABLE: &str = "iam_outbox_events";

/// 把 `?`-风格 SQL 渲染成目标方言（与 `SqlOutboxRepo` 内部走同一条路径）。
fn rendered(sql: &str, dialect: DialectKind) -> String {
    let rq = render(sql, dialect).expect("render");
    let n = rq.expected_binds();
    rq.expect_binds(n).expect("bind count").sql().to_string()
}

// ---------------------------------------------------------------------------
// enqueue —— 唯一需要按方言分叉的一条
// ---------------------------------------------------------------------------

#[test]
fn enqueue_mysql_uses_on_duplicate_key() {
    let sql = enqueue_sql(TABLE, DialectKind::Mysql);
    assert!(
        sql.contains("ON DUPLICATE KEY UPDATE"),
        "MySQL 必须保留 ON DUPLICATE KEY UPDATE，实际: {sql}"
    );
    assert!(
        !sql.contains("ON CONFLICT"),
        "MySQL 不得出现 PG 语法，实际: {sql}"
    );
    assert_eq!(render(&sql, DialectKind::Mysql).unwrap().expected_binds(), 4);
}

#[test]
fn enqueue_postgres_uses_on_conflict_do_nothing() {
    let sql = enqueue_sql(TABLE, DialectKind::Postgres);
    assert!(
        sql.contains("ON CONFLICT (event_id) DO NOTHING"),
        "PG 必须是 ON CONFLICT (event_id) DO NOTHING，实际: {sql}"
    );
    assert!(
        !sql.contains("ON DUPLICATE KEY"),
        "PG 不得出现 MySQL 语法，实际: {sql}"
    );
    // 用 DO NOTHING 而不是 DO UPDATE SET id = id：后者会做一次无意义的空更新。
    assert!(
        !sql.contains("DO UPDATE"),
        "应使用 DO NOTHING（无空更新开销），实际: {sql}"
    );

    let out = rendered(&sql, DialectKind::Postgres);
    assert_eq!(
        out,
        "INSERT INTO iam_outbox_events (event_id, event_type, routing_key, envelope_json) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (event_id) DO NOTHING"
    );
}

// ---------------------------------------------------------------------------
// claim_batch —— 两方言同一份 SQL
// ---------------------------------------------------------------------------

#[test]
fn claim_batch_mysql_keeps_question_mark_limit() {
    let sql = rendered(&claim_batch_sql(TABLE), DialectKind::Mysql);
    assert!(sql.contains("LIMIT ?"), "MySQL 应保留 LIMIT ?，实际: {sql}");
    assert!(
        sql.contains("FOR UPDATE SKIP LOCKED"),
        "必须保留 FOR UPDATE SKIP LOCKED，实际: {sql}"
    );
}

#[test]
fn claim_batch_postgres_rewrites_limit_to_numbered_placeholder() {
    let raw = claim_batch_sql(TABLE);
    // bind 个数必须是 1（只有 LIMIT 一个 ?）：这是 render 校验的前提。
    assert_eq!(
        render(&raw, DialectKind::Postgres).unwrap().expected_binds(),
        1,
        "claim_batch 只应有 1 个占位符（LIMIT）"
    );

    let sql = rendered(&raw, DialectKind::Postgres);
    assert!(sql.contains("LIMIT $1"), "PG 必须是 LIMIT $1，实际: {sql}");
    assert!(
        !sql.contains("LIMIT ?"),
        "PG 不应残留 ? 占位符，实际: {sql}"
    );
    assert!(
        sql.contains("FOR UPDATE SKIP LOCKED"),
        "PG 也必须保留 FOR UPDATE SKIP LOCKED，实际: {sql}"
    );
    // 列顺序不得被改写（fetch 是按位置解包的）。
    assert!(
        sql.starts_with("SELECT id, event_id, routing_key, envelope_json"),
        "列顺序被改动会破坏按位置解包，实际: {sql}"
    );
}

// ---------------------------------------------------------------------------
// mark_published —— 动态 IN 列表
// ---------------------------------------------------------------------------

#[test]
fn mark_published_keeps_current_timestamp_3_on_postgres() {
    let sql = rendered(&mark_published_sql(TABLE, 3), DialectKind::Postgres);
    assert!(
        sql.contains("CURRENT_TIMESTAMP(3)"),
        "CURRENT_TIMESTAMP(3) 是两方言通用写法，**不得**改写成 now()，实际: {sql}"
    );
    assert!(
        !sql.contains("now()"),
        "不应改写成 now()，实际: {sql}"
    );
}

#[test]
fn mark_published_postgres_numbers_placeholders_in_order() {
    let raw = mark_published_sql(TABLE, 3);
    assert_eq!(
        render(&raw, DialectKind::Postgres).unwrap().expected_binds(),
        3,
        "IN 列表的占位符个数必须等于 ids.len()"
    );

    let sql = rendered(&raw, DialectKind::Postgres);
    assert!(
        sql.contains("WHERE id IN ($1,$2,$3)"),
        "PG 的 IN 列表必须是 $1..$3 且顺序对应 ids 顺序，实际: {sql}"
    );
}

#[test]
fn mark_published_mysql_keeps_question_marks() {
    let sql = rendered(&mark_published_sql(TABLE, 3), DialectKind::Mysql);
    assert!(
        sql.contains("WHERE id IN (?,?,?)"),
        "MySQL 应保留 ?,?,?，实际: {sql}"
    );
}

/// 单 id 也要正确（`IN (?)` → `IN ($1)`），避免 N=1 的边界漏测。
#[test]
fn mark_published_single_id() {
    let sql = rendered(&mark_published_sql(TABLE, 1), DialectKind::Postgres);
    assert!(sql.contains("WHERE id IN ($1)"), "实际: {sql}");
}

// ---------------------------------------------------------------------------
// 表名安全校验（表名是插入进 SQL 的，不是 bind）
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "invalid outbox table name")]
fn enqueue_sql_rejects_unsafe_table_name() {
    let _ = enqueue_sql("iam_outbox_events; DROP TABLE iam_users", DialectKind::Mysql);
}
