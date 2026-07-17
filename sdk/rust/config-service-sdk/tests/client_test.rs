use std::sync::{Arc, Mutex};
use config_service_sdk::{
    ConfigClient, CreateConfigItem, CreateNamespace, ListItemsQuery, ListNamespacesQuery,
};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> ConfigClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    ConfigClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn list_namespaces_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/config/namespace"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "ns-1",
                "tenant_id": "t-1",
                "app": "iam-service",
                "env": "prod",
                "description": "iam production config",
                "created_at": "2026-07-01T00:00:00Z",
                "updated_at": "2026-07-01T00:00:00Z"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let page = client
        .list_namespaces(&ListNamespacesQuery::new().app("iam-service").env("prod"))
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].app, "iam-service");
    assert_eq!(page.items[0].env, "prod");
}

#[tokio::test]
async fn create_namespace_posts_body_and_returns_namespace() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/config/namespace"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ns-new",
            "tenant_id": "t-1",
            "app": "audit-service",
            "env": "staging",
            "created_at": "2026-07-17T00:00:00Z",
            "updated_at": "2026-07-17T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let req = CreateNamespace {
        app: "audit-service".into(),
        env: "staging".into(),
        description: Some("audit staging".into()),
    };
    let ns = client.create_namespace(&req).await.unwrap();
    assert_eq!(ns.id, "ns-new");
    assert_eq!(ns.env, "staging");
}

#[tokio::test]
async fn list_items_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/config/item"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "ci-1",
                "namespace_id": "ns-1",
                "key": "rate_limit",
                "value": 100,
                "value_type": "int",
                "is_secret": false,
                "created_at": "2026-07-01T00:00:00Z",
                "updated_at": "2026-07-01T00:00:00Z"
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let page = client
        .list_items(&ListItemsQuery::new().namespace_id("ns-1").key("rate_limit"))
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].key, "rate_limit");
    assert_eq!(page.items[0].value, json!(100));
}

#[tokio::test]
async fn create_item_posts_body_and_returns_item() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/config/item"))
        .and(header("authorization", "Bearer test.jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "ci-new",
            "namespace_id": "ns-1",
            "key": "log_level",
            "value": "info",
            "value_type": "string",
            "is_secret": false,
            "created_at": "2026-07-17T00:00:00Z",
            "updated_at": "2026-07-17T00:00:00Z"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "test.jwt");
    let req = CreateConfigItem {
        namespace_id: "ns-1".into(),
        key: "log_level".into(),
        value: json!("info"),
        description: None,
        is_secret: Some(false),
    };
    let item = client.create_item(&req).await.unwrap();
    assert_eq!(item.key, "log_level");
    assert_eq!(item.value, json!("info"));
}

#[tokio::test]
async fn list_namespaces_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/config/namespace"))
        .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "expired");
    let err = client
        .list_namespaces(&ListNamespacesQuery::new())
        .await
        .expect_err("expected Auth");
    match err {
        kudi_http_client::SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected Auth, got {:?}", other),
    }
}