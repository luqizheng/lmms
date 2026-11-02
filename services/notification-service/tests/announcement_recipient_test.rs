//! Phase 5B-fu3: publish_announcement resolves role/dept scopes via local
//! ntf_user_projection (replaces 4.3-fu2 wiremock-iam path).
//! Membership event apply / bootstrap / out-of-order covered in
//! membership_projection_test.rs (T8).
//!
//! 方言注（S5c）：seed / 断言 SQL 是 MySQL 方言，经 `common::mysql_pool` 取
//! 底层 MySQL 池执行；PG 往返由 `pg_roundtrip_test.rs` 覆盖。
mod common;

use digua_jwt_ctx::TenantScope;
use notification_service::db::Db;

async fn setup() -> (Db, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    (h.db, h.container)
}

async fn seed_ann(db: &Db, recipients: &[(&str, &str)]) -> i64 {
    let pool = common::mysql_pool(db);
    let mut tx = pool.begin().await.unwrap();
    let res = sqlx::query(
        "INSERT INTO ntf_announcements (title, body, publish_at, status, created_by, created_at) \
         VALUES ('', '', NOW(), 'draft', 'x', NOW())",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let id = res.last_insert_id() as i64;
    for (st, sid) in recipients {
        sqlx::query("INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)")
            .bind(id).bind(*st).bind(*sid).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    id
}

async fn deliveries(db: &Db, ann_id: i64) -> Vec<String> {
    let pool = common::mysql_pool(db);
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT user_id FROM ntf_announcement_deliveries WHERE announcement_id=?")
            .bind(ann_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    rows.into_iter().map(|(s,)| s).collect()
}

async fn seed_projection(
    db: &Db,
    user_id: &str,
    tenant_id: i64,
    role_ids: &[&str],
    dept_id: Option<&str>,
) {
    let pool = common::mysql_pool(db);
    let role_json = serde_json::to_string(role_ids).unwrap();
    sqlx::query(
        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
         VALUES (?, ?, CAST(? AS JSON), ?, 0)",
    )
    .bind(user_id)
    .bind(tenant_id)
    .bind(&role_json)
    .bind(dept_id)
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn publish_announcement_role_scope_resolves_via_local_projection() {
    let (db, _c) = setup().await;
    seed_projection(&db, "u1", 1, &["r1"], None).await;
    seed_projection(&db, "u2", 1, &["r1"], None).await;
    seed_projection(&db, "u3", 1, &["r1"], None).await;
    let ann_id = seed_ann(&db, &[("role", "r1")]).await;
    db.publish_announcement(ann_id, &TenantScope::Single(1))
        .await
        .unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u1", "u2", "u3"]);
}

#[tokio::test]
async fn publish_announcement_dept_scope_resolves_via_local_projection() {
    let (db, _c) = setup().await;
    seed_projection(&db, "u10", 1, &[], Some("d1")).await;
    seed_projection(&db, "u11", 1, &[], Some("d1")).await;
    let ann_id = seed_ann(&db, &[("dept", "d1")]).await;
    db.publish_announcement(ann_id, &TenantScope::Single(1))
        .await
        .unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u10", "u11"]);
}

#[tokio::test]
async fn publish_announcement_mixed_scopes_dedup() {
    let (db, _c) = setup().await;
    seed_projection(&db, "u1", 1, &["r1"], None).await;
    seed_projection(&db, "u2", 1, &["r1"], Some("d1")).await;
    seed_projection(&db, "u3", 1, &[], Some("d1")).await;
    let ann_id = seed_ann(&db, &[("role", "r1"), ("dept", "d1")]).await;
    db.publish_announcement(ann_id, &TenantScope::Single(1))
        .await
        .unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u1", "u2", "u3"]);
}

#[tokio::test]
async fn publish_announcement_role_scope_unknown_role_writes_no_deliveries() {
    // Phase 5B-fu3: role scope 查本地 projection, 找不到 role 不报错 (返回空)
    let (db, _c) = setup().await;
    seed_projection(&db, "u1", 1, &["r1"], None).await;
    let ann_id = seed_ann(&db, &[("role", "r_unknown")]).await;
    let users = db
        .publish_announcement(ann_id, &TenantScope::Single(1))
        .await
        .unwrap();
    assert!(users.is_empty(), "unknown role → no users (no HTTP error)");
    assert!(deliveries(&db, ann_id).await.is_empty());
    let pool = common::mysql_pool(&db);
    let (status,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id=?")
        .bind(ann_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "published",
        "announcement still marked published even with 0 deliveries"
    );
}

// Phase 4.3-fu1: large fanout path stays — no HTTP involved now, just batch INSERT.
#[tokio::test]
async fn publish_announcement_large_role_fanout_writes_all_deliveries() {
    let (db, _c) = setup().await;
    let users: Vec<String> = (0..100).map(|i| format!("u{i:03}")).collect();
    let user_refs: Vec<&str> = users.iter().map(|s| s.as_str()).collect();
    for u in &user_refs {
        seed_projection(&db, u, 1, &["big"], None).await;
    }

    let ann_id = seed_ann(&db, &[("role", "big")]).await;
    db.publish_announcement(ann_id, &TenantScope::Single(1))
        .await
        .unwrap();
    let pool = common::mysql_pool(&db);
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
            .bind(ann_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(n, 100, "all 100 deliveries must be written in one batch");
    let (status,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id=?")
        .bind(ann_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "published");
}
