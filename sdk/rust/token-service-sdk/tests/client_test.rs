use digua_token_sdk::{
    AccessRequest, CreateApiKeyRequest, ListApiKeysQuery, RefreshRequest, RevokeRequest,
    TokenClient,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bearer_client(server: &MockServer, token: &str) -> TokenClient {
    let token = Arc::new(Mutex::new(Some(token.to_string())));
    TokenClient::new(server.uri(), move || token.lock().unwrap().clone())
}

#[tokio::test]
async fn access_posts_grant_and_returns_tokens() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/access"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-1",
            "refresh_token": "rt-1",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "ignored");
    let req = AccessRequest {
        grant_type: "authorization_code".into(),
        code: "abc".into(),
        client_id: "c1".into(),
        client_secret: "s1".into(),
    };
    let resp = client.access(&req).await.unwrap();
    assert_eq!(resp.access_token, "at-1");
}

#[tokio::test]
async fn refresh_posts_refresh_token_and_returns_new_pair() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/refresh"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-2",
            "refresh_token": "rt-2",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = RefreshRequest {
        refresh_token: "rt-1".into(),
        client_id: "c1".into(),
        client_secret: "s1".into(),
    };
    let resp = client.refresh(&req).await.unwrap();
    assert_eq!(resp.refresh_token, "rt-2");
}

#[tokio::test]
async fn revoke_posts_token_jti_and_returns_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/revoke"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "success": true })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = RevokeRequest {
        token_jti: "jti-1".into(),
        expires_in: 1234567890,
    };
    let resp = client.revoke(&req).await.unwrap();
    assert!(resp.success);
}

#[tokio::test]
async fn list_api_keys_builds_query_and_parses_paged_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api-keys"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "api_key_id": "ak-1",
                "name": "service-a",
                "status": "active",
                "permissions": ["iam:user:read"],
                "grant_types": ["client_credentials"]
            }],
            "total": 1,
            "page": 0,
            "size": 20
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let resp = client
        .list_api_keys(&ListApiKeysQuery::new().keyword("service").page(0).size(20))
        .await
        .unwrap();
    assert_eq!(resp.total, 1);
    assert_eq!(resp.items[0].api_key_id.as_deref(), Some("ak-1"));
}

#[tokio::test]
async fn create_api_key_returns_id_and_secret() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api-keys"))
        .and(header("authorization", "Bearer at-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "api_key_id": "ak-new",
            "api_key_secret": "secret-once"
        })))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "at-1");
    let req = CreateApiKeyRequest {
        name: "service-b".into(),
        permissions: vec!["iam:user:read".into()],
        grant_types: vec!["client_credentials".into()],
    };
    let created = client.create_api_key(&req).await.unwrap();
    assert_eq!(created.api_key_id, "ak-new");
    assert_eq!(created.api_key_secret, "secret-once");
}

#[tokio::test]
async fn access_401_maps_to_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tokens/access"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid client"))
        .mount(&server)
        .await;
    let client = bearer_client(&server, "ignored");
    let req = AccessRequest {
        grant_type: "client_credentials".into(),
        code: "x".into(),
        client_id: "c".into(),
        client_secret: "bad".into(),
    };
    let err = client.access(&req).await.expect_err("expected Auth error");
    match err {
        digua_http_client::SdkError::Auth { status, .. } => assert_eq!(status, 401),
        other => panic!("expected Auth, got {:?}", other),
    }
}
