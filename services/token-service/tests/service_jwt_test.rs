//! Phase 5-B: `POST /tokens/service-jwt` integration test.
//!
//! Uses axum's `Router` directly �?no MySQL needed. The handler reads only
//! `state.private_key_pem` and `state.bootstrap_tokens`.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_jwt::verify_token;
use serde_json::{json, Value};
use tower::ServiceExt;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../../libs/common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str =
    include_str!("../../../libs/common-jwt/tests/fixtures/test_public.pem");

fn build_state_with(
    bootstrap_tokens: Vec<String>,
    bootstrap_owners: Vec<String>,
) -> token_service::TokenServerState {
    let pool = sqlx::mysql::MySqlPool::connect_lazy("mysql://placeholder").unwrap();
    let jwks = std::sync::Arc::new(digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap());
    token_service::TokenServerState {
        db: token_service::db::DbPool::from(pool),
        issuer: std::sync::Arc::new(
            token_service::TokenIssuer::new(TEST_PRIVATE_PEM, "test-issuer").expect("issuer"),
        ),
        issuer_str: "test-issuer".into(),
        jwt_public_key: TEST_PUBLIC_PEM.into(),
        default_resource_ttl_sec: 3600,
        http_client: std::sync::Arc::new(digua_common_http::ResilientClient::new(
            reqwest::Client::new(),
            digua_common_http::ResilienceConfig::default(),
            std::sync::Arc::new(digua_common_http::NoopObserver),
        )),
        user_service_url: "http://iam-service:8081".into(),
        private_key_pem: TEST_PRIVATE_PEM.into(),
        bootstrap_tokens,
        bootstrap_owners,
        jwks,
    }
}

#[tokio::test]
async fn service_jwt_with_valid_bootstrap_returns_signed_jwt() {
    // sub = the bootstrap owner (parallel SERVICE_BOOTSTRAP_OWNERS list),
    // no longer token-service's own SERVICE_NAME env.
    let state = build_state_with(
        vec!["bt-iam".into(), "bt-token".into()],
        vec!["iam-service".into(), "token-service".into()],
    );
    let app = token_service::service_jwt_routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer bt-token")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "iam-service", "tid": 1}).to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["token_type"], "Bearer");
    assert_eq!(parsed["expires_in"], 3600);
    let access_token = parsed["access_token"].as_str().unwrap();

    // Verify with the public key
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(access_token, &cache, Some("iam-service"))
        .await
        .unwrap();
    assert_eq!(claims["iss"], "token-service"); // service-jwt always issued by token-service
    assert_eq!(claims["sub"], "token-service");
    assert_eq!(claims["aud"], "iam-service");
    assert_eq!(claims["tid"], 1);
    assert_eq!(claims["roles"][0], "service");
    assert_eq!(claims["purpose"], "service");
}

#[tokio::test]
async fn service_jwt_with_wrong_bootstrap_returns_401() {
    let state = build_state_with(vec!["bt-iam".into()], vec!["iam-service".into()]);
    let app = token_service::service_jwt_routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer bt-wrong")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "iam-service", "tid": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn service_jwt_without_auth_returns_401() {
    let state = build_state_with(vec!["bt-iam".into()], vec!["iam-service".into()]);
    let app = token_service::service_jwt_routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "iam-service", "tid": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn service_jwt_with_empty_aud_returns_400() {
    let state = build_state_with(vec!["bt-iam".into()], vec!["iam-service".into()]);
    let app = token_service::service_jwt_routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer bt-iam")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"aud": "", "tid": 1}).to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn service_jwt_ct_claim_and_unknown_owner() {
    let state = build_state_with(vec!["bt-pm".into()], vec![]);
    let app = token_service::service_jwt_routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer bt-pm")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "file-service", "tid": 42, "ct": true}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(parsed["access_token"].as_str().unwrap(), &cache, Some("file-service"))
        .await
        .unwrap();
    assert_eq!(claims["ct"], true);
    assert_eq!(claims["tid"], 42);
    // no SERVICE_BOOTSTRAP_OWNERS entry -> sub falls back to "unknown"
    assert_eq!(claims["sub"], "unknown");
}
