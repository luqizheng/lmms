//! Phase 4.3 集成测试。每个用例起 MySQL testcontainer，建 ntf schema，
//! 用 publish_announcement 驱动，断言 outbox/delivery/status/幂等/空集/reload。
mod common;

use common_http::{NoopObserver, ResilienceConfig, ResilientClient};
use notification_service::db::Db;
use notification_service::iam_client::IamMemberClient;
use std::sync::Arc;

fn stub_iam_client() -> Arc<IamMemberClient> {
    let inner = reqwest::Client::new();
    let rc = Arc::new(ResilientClient::new(
        inner,
        ResilienceConfig::default(),
        Arc::new(NoopObserver),
    ));
    Arc::new(IamMemberClient::new(rc, "http://unused".into()))
}

async fn setup_db() -> (Db, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    (h.db, h.container)
}

async fn seed_announcement(db: &Db, scopes: &[(&str, &str)]) -> u64 {
    // 直接插一条 scheduled 公告 + recipients（绕过 REST，聚焦 db 层）
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

// B: 正常路径
#[tokio::test]
async fn publish_inserts_deliveries_status_and_outbox() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1"), ("user", "u2")]).await;
    let client = stub_iam_client();
    let users = db.publish_announcement(id, &client, 1).await.unwrap();
    assert_eq!(users.len(), 2);
    // status
    let (st,): (String,) = sqlx::query_as("SELECT status FROM ntf_announcements WHERE id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(st, "published");
    // deliveries
    let (d,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(d, 2);
    // outbox
    assert_eq!(outbox_count(&db, id).await, 1);
}

// C: 幂等守卫 — 二次调用不新增 outbox
#[tokio::test]
async fn publish_is_idempotent_no_duplicate_outbox() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1")]).await;
    let client = stub_iam_client();
    db.publish_announcement(id, &client, 1).await.unwrap();
    let again = db.publish_announcement(id, &client, 1).await.unwrap();
    assert!(again.is_empty(), "second publish should be no-op");
    assert_eq!(outbox_count(&db, id).await, 1, "no duplicate outbox row");
}

// D: role/dept resolved via expand_scopes (phase 4.3-fu2). Stub IAM => 502 on role/dept.
// This test covers user-only scopes; role/dept resolution is tested in Task 9.
#[tokio::test]
async fn publish_with_mixed_scopes_user_only() {
    let (db, _c) = setup_db().await;
    let id = seed_announcement(&db, &[("user", "u1"), ("user", "u2")]).await;
    let client = stub_iam_client();
    let users = db.publish_announcement(id, &client, 1).await.unwrap();
    assert_eq!(users.len(), 2);
    let (d,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?")
        .bind(id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(d, 2);
}

// E: 无 user-scope 接收人 — 仍 published，enqueue 空 user_ids
// NOTE: role-only scopes trigger IAM call which fails with stub client — this test covers empty user-scope case.
#[tokio::test]
async fn publish_with_no_user_scopes_does_nothing() {
    let (db, _c) = setup_db().await;
    // Only role/dept scopes would 502 with stub; test user-only empty case instead
    let id = seed_announcement(&db, &[]).await;
    let client = stub_iam_client();
    let users = db.publish_announcement(id, &client, 1).await.unwrap();
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
    let client = stub_iam_client();
    db.publish_announcement(id, &client, 1).await.unwrap();
    // 模拟用户重连后拉取（无任何 WS 推送参与）
    let list = db.list_for_user("u1".to_string()).await.unwrap();
    assert!(list.iter().any(|a| a.id == id), "announcement must be visible on reload via deliveries");
}
