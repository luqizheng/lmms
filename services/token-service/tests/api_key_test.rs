//! `POST /api-keys` 与 `POST /admin/oauth-clients` 的用户绑定 + 过期字段。
//!
//! 两条路径创建同一张 `token_oauth_clients` 表的行，字段语义必须一致。
//! HTTP 级测试走 oneshot 房规（见 common::build_state）。

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

fn app(state: token_service::TokenServerState) -> axum::Router {
    axum::Router::new()
        .route(
            "/api-keys",
            axum::routing::post(
                token_service::routes::api_key::handle_create_api_key,
            )
            .get(token_service::routes::api_key::handle_list_api_keys),
        )
        .route(
            "/admin/oauth-clients",
            axum::routing::post(token_service::routes::admin::handle_create),
        )
        .with_state(state)
}

#[tokio::test]
async fn create_api_key_with_user_and_expiry() {
    let state = common::build_state(common::spawn_mysql().await);

    let req = Request::builder()
        .method("POST")
        .uri("/api-keys")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "mcp",
                "permissions": ["PM_MCP_READ"],
                "grant_types": ["client_credentials"],
                "user_id": "usr-abc123",
                "expires_at": "2026-12-31T00:00:00Z"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app(state.clone()).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(!v["api_key_id"].as_str().unwrap().is_empty());
    assert!(!v["api_key_secret"].as_str().unwrap().is_empty());

    // 列表里该条必须带出 user_id / expires_at
    let req = Request::builder()
        .method("GET")
        .uri("/api-keys?keyword=mcp")
        .body(Body::empty())
        .unwrap();
    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 256 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    let item = v["items"]
        .as_array()
        .expect("items array")
        .iter()
        .find(|i| i["name"] == "mcp")
        .expect("created key in list");
    assert_eq!(item["user_id"], "usr-abc123");
    assert!(
        item["expires_at"].as_str().is_some(),
        "expires_at should be serialized in list item"
    );
}

#[tokio::test]
async fn create_api_key_with_invalid_expiry_is_400() {
    let state = common::build_state(common::spawn_mysql().await);

    let req = Request::builder()
        .method("POST")
        .uri("/api-keys")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "bad-expiry",
                "expires_at": "not-a-date"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_admin_client_with_user_and_expiry() {
    let state = common::build_state(common::spawn_mysql().await);

    let req = Request::builder()
        .method("POST")
        .uri("/admin/oauth-clients")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "name": "admin-bound",
                "permissions": ["iam:user:read"],
                "user_id": "usr-admin",
                "expires_at": "2026-06-30T12:00:00Z"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["client"]["user_id"], "usr-admin");
    assert!(
        v["client"]["expires_at"].as_str().is_some(),
        "expires_at should be serialized in client dto"
    );
}
