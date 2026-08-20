use chrono::Utc;
use digua_notification_sdk::{CreateAnnouncement, ListAnnouncementsQuery, NotificationClient};
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> NotificationClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    NotificationClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn list_announcements_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/notification/announcements"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": 1,
                "title": "Maintenance window",
                "body": "2026-07-20 02:00 UTC",
                "publish_at": "2026-07-18T00:00:00Z",
                "pinned": true,
                "popover": false,
                "status": "published",
                "created_by": "admin",
                "created_at": "2026-07-15T00:00:00Z"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let page = client
        .list_announcements(&ListAnnouncementsQuery::new().pinned(true).page(0).size(20))
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].title, "Maintenance window");
    assert!(page.items[0].pinned);
}

#[tokio::test]
async fn get_announcement_returns_single_announcement() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/notification/announcements/1"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1,
            "title": "Welcome",
            "body": "Hello",
            "publish_at": "2026-07-15T00:00:00Z",
            "pinned": false,
            "popover": true,
            "status": "draft",
            "created_by": "admin",
            "created_at": "2026-07-14T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let a = client.get_announcement(1).await.unwrap();
    assert_eq!(a.id, 1);
    assert!(a.popover);
}

#[tokio::test]
async fn create_announcement_posts_body_and_returns_draft() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/notification/announcements"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42,
            "title": "New",
            "body": "Body",
            "publish_at": "2026-07-20T00:00:00Z",
            "pinned": false,
            "popover": false,
            "status": "draft",
            "created_by": "admin",
            "created_at": "2026-07-17T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let req = CreateAnnouncement {
        title: "New".into(),
        body: "Body".into(),
        publish_at: Utc::now(),
        pinned: false,
        popover: false,
        recipients: json!({"scope": "all"}),
    };
    let a = client.create_announcement(&req).await.unwrap();
    assert_eq!(a.id, 42);
    assert_eq!(a.status, "draft");
}

#[tokio::test]
async fn publish_posts_to_publish_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/notification/announcements/1/publish"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    client.publish(1).await.unwrap();
}

#[tokio::test]
async fn unread_count_returns_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/notification/unread-count"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 5 })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let resp = client.unread_count().await.unwrap();
    assert_eq!(resp["count"], 5);
}
