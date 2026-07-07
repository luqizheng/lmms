//! Phase 4.3-fu2: notification publish_announcement resolves role/dept scopes
//! via the iam internal endpoints (mocked by wiremock).
mod common;

use jwt_ctx::TenantScope;
use notification_service::db::Db;
use notification_service::iam_client::IamMemberClient;
use std::sync::Arc;
use std::time::Duration;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path_regex};

const TEST_PRIVATE_PEM: &str = include_str!("fixtures/test_private.pem");

async fn setup() -> (Db, MockServer, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    let server = MockServer::start().await;
    (h.db, server, h.container)
}

async fn build_client(base_url: String) -> Arc<IamMemberClient> {
    use common_http::{NoopObserver, ResilienceConfig, ResilientClient};
    use common_jwt::{ServiceJwtCache, ServiceJwtConfig};
    let inner = reqwest::Client::new();
    let cfg = ResilienceConfig::default();
    let rc = Arc::new(ResilientClient::new(inner, cfg, Arc::new(NoopObserver)));
    let service_jwt = Arc::new(ServiceJwtCache::new(
        ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "notification-service".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        reqwest::Client::new(),
        TEST_PRIVATE_PEM.to_string(),
    ));
    service_jwt
        .prime_cache("iam-service", Duration::from_secs(300))
        .await
        .unwrap();
    Arc::new(IamMemberClient::new(rc, base_url, service_jwt))
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
    let client = build_client(mock.uri()).await;
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
    let client = build_client(mock.uri()).await;
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
    let client = build_client(mock.uri()).await;
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
    let client = build_client(mock.uri()).await;
    let err = db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap_err();
    assert!(err.to_string().contains("iam") || err.to_string().contains("503"));
    let (status,): (String,) = sqlx::query_as(
        "SELECT status FROM ntf_announcements WHERE id=?",
    ).bind(ann_id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(status, "draft");
    assert!(deliveries(&db, ann_id).await.is_empty());
}

// Phase 4.3-fu1: batch INSERT IGNORE path covers role-scope fanout beyond a
// handful of users. Per-row loop would also pass this, but asserts the batch
// path keeps every row written.
#[tokio::test]
async fn publish_announcement_large_role_fanout_writes_all_deliveries() {
    let (db, mock, _c) = setup().await;
    let users: Vec<String> = (0..100).map(|i| format!("u{i:03}")).collect();
    Mock::given(method("GET"))
        .and(path_regex(r"/iam/internal/users/by-role/big.*"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": users, "page": 1, "size": 1000, "total": 100, "truncated": false
        })))
        .mount(&mock).await;
    let ann_id = seed_ann(&db, &[("role", "big")]).await;
    let client = build_client(mock.uri()).await;
    db.publish_announcement(ann_id, &client, &TenantScope::Single(1)).await.unwrap();
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM ntf_announcement_deliveries WHERE announcement_id=?",
    ).bind(ann_id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(n, 100, "all 100 deliveries must be written in one batch");
    let (status,): (String,) = sqlx::query_as(
        "SELECT status FROM ntf_announcements WHERE id=?",
    ).bind(ann_id).fetch_one(&db.pool).await.unwrap();
    assert_eq!(status, "published");
}
