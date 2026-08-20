//! Phase 4.3 集成测试。每个用例起 MySQL testcontainer，建 ntf schema，
//! 用 publish_announcement 驱动，断言 outbox/delivery/status/幂等/空集/reload。
//! Phase 5B-fu3: publish_announcement 改走本地 ntf_user_projection 解析
//! role/dept scope (User scope 直接用 scope_id; role/dept 走 JSON_CONTAINS /
//! dept_id lookup)。本文件覆盖 user-scope 路径; role/dept 路径由
//! membership_projection_test.rs (T8) + bootstrap 集成覆盖。
mod common;

use digua_jwt_ctx::TenantScope;
use notification_service::db::Db;

async fn setup_db() -> (Db, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    (h.db, h.container)
}

async fn seed_announcement(db: &Db, scopes: &[(&str, &str)]) -> u64 {
    let mut tx = db.pool.begin().await.unwrap();
    let res = sqlx::query(
        "INSERT INTO ntf_announcements (title, body, publish_at, expire_at, pinned, popover, status, created_by, created_at) \
         VALUES ('t','b',NOW(),NULL,0,0,'scheduled','admin',NOW())")
        .execute(&mut *tx).await.unwrap();
    let id = res.last_insert_id();
    for (st, sid) in scopes {
        sqlx::query("INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)")
            .bind(id).bind(st).bind(sid).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    id
}

async fn outbox_count(db: &Db, ann_id: u64) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_outbox_events WHERE event_type='notification.announcement.published' \
         AND JSON_EXTRACT(envelope_json, '$.payload.announcement_id') = ?")
        .bind(ann_id as i64).fetch_one(&db.pool).await.unwrap();
    n
}

// B: 正常路径 (User scope 直接 collect)
#[tokio::test]
async fn publish_inserts_deliveries_status_and_outbox() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1"), ("user", "u2")]).await;
    let users = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    assert_eq!(users.len(), 2);
    let (st,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(st, "published");
    let (d,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(d, 2);
    assert_eq!(outbox_count(&db, id).await, 1);
}

// C: 幂等守卫 — 二次调用不新增 outbox
#[tokio::test]
async fn publish_is_idempotent_no_duplicate_outbox() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1")]).await;
    db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    let again = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    assert!(again.is_empty(), "second publish should be no-op");
    assert_eq!(outbox_count(&db, id).await, 1, "no duplicate outbox row");
}

// D: 多 user scope (Phase 5B-fu3: User scope 直 collect, 不走投影)
#[tokio::test]
async fn publish_with_mixed_scopes_user_only() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1"), ("user", "u2")]).await;
    let users = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    assert_eq!(users.len(), 2);
    let (d,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(d, 2);
}

// E: 无 user-scope 接收人 — 仍 published，enqueue 空 user_ids
#[tokio::test]
async fn publish_with_no_user_scopes_does_nothing() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[]).await;
    let users = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    assert!(users.is_empty());
    let (st,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(st, "published");
    assert_eq!(outbox_count(&db, id).await, 1, "event enqueued even with empty user_ids");
}

// I: reload 兜底 — 发布后(无 WS)，list_for_user 仍返回公告
#[tokio::test]
async fn published_announcement_visible_on_reload_without_ws() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1")]).await;
    db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    let list = db.list_for_user("u1".to_string()).await.unwrap();
    assert!(list.iter().any(|a| a.id == id), "announcement must be visible on reload via deliveries");
}

// Phase 5B-fu3: role scope 走本地 ntf_user_projection JSON_CONTAINS
#[tokio::test]
async fn publish_role_scope_uses_local_projection() {
    let (db, _c) = setup_db().await;
    // Seed projection: u1+u2 are in role r1
    sqlx::query(
        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
         VALUES ('u1', 1, JSON_ARRAY('r1'), NULL, 0), ('u2', 1, JSON_ARRAY('r1'), NULL, 0), ('u3', 1, JSON_ARRAY('r2'), NULL, 0)",
    ).execute(&db.pool).await.unwrap();

    let id = seed_announcement(&db, &[("role", "r1")]).await;
    let users = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    let mut sorted = users.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["u1", "u2"]);
    let (d,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(d, 2);
}

// Phase 5B-fu3: dept scope 走本地 ntf_user_projection dept_id 列
#[tokio::test]
async fn publish_dept_scope_uses_local_projection() {
    let (db, _c) = setup_db().await;
    sqlx::query(
        "INSERT INTO ntf_user_projection (user_id, tenant_id, role_ids, dept_id, last_event_at) \
         VALUES ('u10', 1, JSON_ARRAY(), 'd1', 0), ('u11', 1, JSON_ARRAY(), 'd1', 0), ('u12', 1, JSON_ARRAY(), 'd2', 0)",
    ).execute(&db.pool).await.unwrap();

    let id = seed_announcement(&db, &[("dept", "d1")]).await;
    let users = db.publish_announcement(id, &TenantScope::Single(1)).await.unwrap();
    let mut sorted = users.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["u10", "u11"]);
}