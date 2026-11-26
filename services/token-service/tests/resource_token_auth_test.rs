//! WP-LMMS-HARDEN-01-R1: `/tokens/resource` 认证回归测试。
//!
//! 修复前该端点是签发类端点中唯一无认证的（公开签名神谕：任何调用方可让
//! 平台密钥签出任意 resource_type/resource_id/permission 的 JWT）。
//! 修复后与 `/tokens/user-jwt` 一致，要求 service-jwt（aud=token-service）准入：
//! 1. 未认证请求 → 401；
//! 2. aud 不符的 service-jwt → 401；
//! 3. aud=token-service 的 service-jwt → 200，签出的 resource token claims
//!    与修复前保持一致（sub=resource-service、scope/purpose=resource、
//!    resource 三元组原样；不含 aud/tid——契约变更须另行提案）。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_jwt::verify_token;
use serde_json::Value;
use tower::ServiceExt;

mod common;

use common::build_state;

const RESOURCE_REQUEST: &str =
    r#"{"resource_type":"contract","resource_id":"CT-1","permission":"read","ttl_sec":300}"#;

#[tokio::test]
async fn resource_token_without_auth_is_rejected() {
    let pool = common::spawn_mysql().await;
    let state = build_state(pool);
    let app = token_service::routes::resource::routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/resource")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(RESOURCE_REQUEST))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn resource_token_with_wrong_audience_service_jwt_is_rejected() {
    let pool = common::spawn_mysql().await;
    let state = build_state(pool);
    let bearer = token_service::mint_service_bearer_for_state(&state, "file-service", 300)
        .expect("mint wrong-aud service jwt");
    let app = token_service::routes::resource::routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/resource")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(RESOURCE_REQUEST))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn resource_token_with_token_service_audience_issues_resource_jwt() {
    let pool = common::spawn_mysql().await;
    let state = build_state(pool);
    let bearer = token_service::mint_service_bearer_for_state(&state, "token-service", 300)
        .expect("mint service jwt");
    let app = token_service::routes::resource::routes(state.clone());

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/resource")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(RESOURCE_REQUEST))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["expires_in"], 300);
    let resource_token = parsed["resource_token"].as_str().unwrap();

    // resource token claims 与修复前一致（不含 aud/tid——契约变更须另行提案）
    let cache = digua_common_jwt::JWKSCache::from_pem(common::TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(resource_token, &cache, None).await.unwrap();
    assert_eq!(claims["sub"], "resource-service");
    assert_eq!(claims["scope"], "resource");
    assert_eq!(claims["purpose"], "resource");
    assert_eq!(claims["resource_type"], "contract");
    assert_eq!(claims["resource_id"], "CT-1");
    assert_eq!(claims["permission"], "read");
}
