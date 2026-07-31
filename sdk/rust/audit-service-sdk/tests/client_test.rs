use digua_audit_sdk::{AuditClient, LoginLogQuery, OperationLogQuery, RetentionPolicy};
use digua_http_client::SdkError;
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_with_bearer(server: &MockServer, token: &str) -> AuditClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    AuditClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn query_login_logs_sends_get_and_parses_paged_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/audit/login"))
        .and(query_param("username", "alice"))
        .and(query_param("page", "0"))
        .and(query_param("size", "20"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": 1,
                "username": "alice",
                "success": true,
                "ip_address": "127.0.0.1",
                "request_id": "req-1",
                "created_at": "2026-07-01T00:00:00Z"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "test.jwt");
    let page = client
        .query_login_logs(
            &LoginLogQuery::new()
                .username("alice")
                .page(0)
                .size(20),
        )
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].username, "alice");
    assert!(page.items[0].success);
    assert_eq!(page.items[0].id, 1);
}

#[tokio::test]
async fn query_operation_logs_sends_get_and_parses_paged_response() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/audit/operation"))
        .and(query_param("action", "user.update"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": 42,
                "user_id": "u-1",
                "action": "user.update",
                "resource": "user/123",
                "request_id": "req-99",
                "created_at": "2026-07-02T00:00:00Z",
                "after_value": "{\"name\":\"new\"}",
                "resource_id": "u-123"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "test.jwt");
    let page = client
        .query_operation_logs(&OperationLogQuery::new().action("user.update"))
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].user_id, "u-1");
    assert_eq!(page.items[0].after_value.as_deref(), Some("{\"name\":\"new\"}"));
    assert_eq!(page.items[0].resource_id.as_deref(), Some("u-123"));
}

#[tokio::test]
async fn trigger_cleanup_posts_and_returns_object() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/audit/retention/cleanup"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "test.jwt");
    let result = client.trigger_cleanup().await.unwrap();

    assert!(result.is_object());
    assert_eq!(result.as_object().unwrap().len(), 0);
}

#[tokio::test]
async fn get_retention_policy_parses_complex_schema() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/audit/retention/policy"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hot_days": 7,
            "warm_days": 30,
            "cold_days": 90,
            "archive_enabled": true,
            "purge_enabled": false
        })))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "test.jwt");
    let policy = client.get_retention_policy().await.unwrap();

    assert_eq!(policy.hot_days, 7);
    assert_eq!(policy.warm_days, 30);
    assert_eq!(policy.cold_days, 90);
    assert!(policy.archive_enabled);
    assert!(!policy.purge_enabled);
}

#[tokio::test]
async fn query_login_logs_401_maps_to_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/audit/login"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "expired.jwt");
    let err = client
        .query_login_logs(&LoginLogQuery::new())
        .await
        .expect_err("expected Auth error");

    match err {
        SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected SdkError::Auth, got {:?}", other),
    }
}

#[tokio::test]
async fn update_retention_policy_round_trip() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/audit/retention/policy"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "hot_days": 14,
            "warm_days": 60,
            "cold_days": 180,
            "archive_enabled": true,
            "purge_enabled": true
        })))
        .mount(&server)
        .await;

    let client = client_with_bearer(&server, "test.jwt");
    let new_policy = RetentionPolicy {
        hot_days: 14,
        warm_days: 60,
        cold_days: 180,
        archive_enabled: true,
        purge_enabled: true,
    };
    let updated = client.update_retention_policy(&new_policy).await.unwrap();

    assert_eq!(updated.hot_days, 14);
    assert!(updated.purge_enabled);
}