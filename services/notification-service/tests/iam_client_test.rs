use common_http::{ResilientClient, ResilienceConfig};
use notification_service::iam_client::IamMemberClient;
use std::sync::Arc;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(base_url: String) -> IamMemberClient {
    let http = Arc::new(ResilientClient::new(
        reqwest::Client::new(),
        ResilienceConfig::default(),
        Arc::new(common_http::NoopObserver),
    ));
    IamMemberClient::new(http, base_url)
}

#[tokio::test]
async fn by_role_passes_tenant_id_in_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/iam/internal/users/by-role/admin"))
        .and(query_param("tenant_id", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"items": ["u1", "u2"], "page": 1, "size": 1000, "total": 2, "truncated": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(server.uri());
    let result = c.by_role("admin", 2).await.unwrap();
    assert_eq!(result, vec!["u1", "u2"]);
}

#[tokio::test]
async fn by_dept_passes_tenant_id_in_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/iam/internal/users/by-dept/o1"))
        .and(query_param("tenant_id", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"items": ["u3"], "page": 1, "size": 1000, "total": 1, "truncated": false}),
        ))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(server.uri());
    let result = c.by_dept("o1", 3).await.unwrap();
    assert_eq!(result, vec!["u3"]);
}

#[tokio::test]
async fn fetch_items_returns_error_on_500() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let c = client(server.uri());
    let result = c.by_role("admin", 1).await;
    assert!(result.is_err());
}
