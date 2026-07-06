//! Phase 4.3-fu2: notification publish_announcement resolves role/dept scopes
//! via the iam internal endpoints (mocked by wiremock).
mod common;

use jwt_ctx::TenantScope;
use notification_service::db::Db;
use notification_service::iam_client::IamMemberClient;
use std::sync::Arc;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path_regex};

async fn setup() -> (Db, MockServer, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    let server = MockServer::start().await;
    (h.db, server, h.container)
}

fn build_client(base_url: String) -> Arc<IamMemberClient> {
    use common_http::{NoopObserver, ResilienceConfig, ResilientClient};
    let inner = reqwest::Client::new();
    let cfg = ResilienceConfig::default();
    let rc = Arc::new(ResilientClient::new(inner, cfg, Arc::new(NoopObserver)));
    Arc::new(IamMemberClient::new(rc, base_url))
}

async fn seed_ann(db: &Db, recipients: &[(&str, &str)]) -> u64 {
    let mut tx = db.pool.begin().await.unwrap();
    let res = sqlx::query(
        "INSERT INTO ntf_announcements (title, body, publish_at, status, created_by, created_at) \
         VALUES ('', '', NOW(), 'draft', 'x', NOW())",
    ).execute(&mut *tx).await.unwrap();
    let id = res.last_insert_id();
    for (st, sid) in recipients {
        sqlx::query("INSERT INTO ntf_announcement_recipients (announcement_id, scope_type, scope_id) VALUES (?,?,?)")
            .bind(id).bind(*st).bind(*sid).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    id
}

async fn deliveries(db: &Db, ann_id: u64) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT user_id FROM ntf_announcement_deliveries WHERE announcement_id=?",
    ).bind(ann_id).fetch_all(&db.pool).await.unwrap();
    rows.into_iter().map(|(s,)| s).collect()
}

#[tokio::test]
async fn publish_announcement_role_scope_resolves_via_iam() {
    let (db, mock, _c) = setup().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-role/r1.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": ["u1", "u2", "u3"],
            "page": 1, "size": 100, "total": 3, "truncated": false
        })))
        .mount(&mock).await;
    let ann_id = seed_ann(&db, &[("role", "r1")]).await;
    let client = build_client(mock.uri());
    db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u1", "u2", "u3"]);
}

#[tokio::test]
async fn publish_announcement_dept_scope_resolves_via_iam() {
    let (db, mock, _c) = setup().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-dept/d1.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": ["u10", "u11"],
            "page": 1, "size": 100, "total": 2, "truncated": false
        })))
        .mount(&mock).await;
    let ann_id = seed_ann(&db, &[("dept", "d1")]).await;
    let client = build_client(mock.uri());
    db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u10", "u11"]);
}

#[tokio::test]
async fn publish_announcement_mixed_scopes_dedup() {
    let (db, mock, _c) = setup().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-role/r1.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": ["u1", "u2"], "page": 1, "size": 100, "total": 2, "truncated": false
        }))).mount(&mock).await;
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-dept/d1.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": ["u2", "u3"], "page": 1, "size": 100, "total": 2, "truncated": false
        }))).mount(&mock).await;
    let ann_id = seed_ann(&db, &[("role", "r1"), ("dept", "d1")]).await;
    let client = build_client(mock.uri());
    db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap();
    let mut v = deliveries(&db, ann_id).await;
    v.sort();
    assert_eq!(v, vec!["u1", "u2", "u3"]);
}

#[tokio::test]
async fn publish_announcement_iam_unavailable_fails_publish() {
    let (db, mock, _c) = setup().await;
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-role/r1.*"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&mock).await;
    let ann_id = seed_ann(&db, &[("role", "r1")]).await;
    let client = build_client(mock.uri());
    let err = db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap_err();
    assert!(err.to_string().contains("iam") || err.to_string().contains("503"));
    let (status,): (String,) = sqlx::query_as(
        "SELECT status FROM ntf_announcements WHERE id=?",
    ).bind(ann_id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(status, "draft");
    assert!(deliveries(&db, ann_id).await.is_empty());
}
