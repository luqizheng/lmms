//! `/api-keys` 与 `/admin/oauth-clients` 的鉴权。
//!
//! spec §3.5「附带发现」：这两个路径原先没有鉴权中间件——任何能访问
//! token-service 的人都能创建 API Key、增删 OAuth 客户端。引入"绑定用户
//! 的高权限 Key"后，未鉴权创建等于可凭空造出任意用户身份的密钥。
//!
//! 测试导入 `routes::api_key::routes` / `routes::admin::routes` 这两个
//! 路由构造器（与 main.rs 同一装配），保证测的就是生产挂载方式。

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_jwt::sign_token;
use serde_json::json;
use std::time::Duration;
use tower::ServiceExt;

/// aud=token-service 的 service-jwt（中间件验签走 state.jwks() = TEST_PUBLIC_PEM）。
fn service_jwt() -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = json!({
        "iss": "token-service",
        "sub": "test-admin",
        "aud": "token-service",
        "tid": 1,
        "iat": now,
        "exp": now + 3600,
        "roles": ["service"],
    });
    sign_token(common::TEST_PRIVATE_PEM, &claims, Duration::from_secs(3600))
        .expect("sign service jwt")
}

#[tokio::test]
async fn create_api_key_without_service_jwt_is_rejected() {
    let state = common::build_state(common::spawn_mysql().await);
    let app = token_service::routes::api_key::routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api-keys")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"name": "no-auth"}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_api_keys_without_service_jwt_is_rejected() {
    let state = common::build_state(common::spawn_mysql().await);
    let app = token_service::routes::api_key::routes(state);

    let req = Request::builder()
        .method("GET")
        .uri("/api-keys")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_client_create_without_service_jwt_is_rejected() {
    let state = common::build_state(common::spawn_mysql().await);
    let app = token_service::routes::admin::routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/admin/oauth-clients")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"name": "no-auth"}).to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_api_key_with_service_jwt_succeeds() {
    let state = common::build_state(common::spawn_mysql().await);
    let app = token_service::routes::api_key::routes(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api-keys")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", service_jwt()))
        .body(Body::from(
            json!({
                "name": "with-auth",
                "permissions": ["PM_MCP_READ"],
                "grant_types": ["client_credentials"],
                "user_id": "usr-abc123"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_client_list_with_service_jwt_succeeds() {
    let state = common::build_state(common::spawn_mysql().await);
    let app = token_service::routes::admin::routes(state);

    let req = Request::builder()
        .method("GET")
        .uri("/admin/oauth-clients")
        .header(header::AUTHORIZATION, format!("Bearer {}", service_jwt()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
