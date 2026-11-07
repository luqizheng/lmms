//! `POST /tokens/access` 的 `client_credentials` 授权流。
//!
//! 方案 A 的核心链路：API Key（绑定真实用户的 client）→ client_credentials 换 token，
//! token 的 `sub` = 绑定的 user_id、`permissions` = 用户 claims ∩ client permissions。
//!
//! HTTP 级测试走 `tower::ServiceExt::oneshot` 直接打 Router（service_jwt_test.rs 房规）；
//! 表单编码（application/x-www-form-urlencoded）是 SDK 的实际格式
//! （sdk/rust/digua-http-client/src/fetcher.rs），必须与 JSON 并存接受。

mod common;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_jwt::verify_token;
use serde_json::{json, Value};
use token_service::models::oauth_client::Client;
use tower::ServiceExt;

const TEST_PRIVATE_PEM: &str =
    include_str!("../../../libs/common-jwt/tests/fixtures/test_private.pem");
const TEST_PUBLIC_PEM: &str =
    include_str!("../../../libs/common-jwt/tests/fixtures/test_public.pem");

/// 真实 DB pool + 测试密钥的 state（client 查询必须走真库）。
async fn build_state() -> token_service::TokenServerState {
    let pool = common::spawn_mysql().await;
    let jwks = std::sync::Arc::new(digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap());
    token_service::TokenServerState {
        db: pool,
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
        // 不可达地址 → 走既有降级逻辑（默认 claims）
        user_service_url: "http://iam-service:8081".into(),
        private_key_pem: TEST_PRIVATE_PEM.into(),
        bootstrap_tokens: vec![],
        bootstrap_owners: vec![],
        jwks,
    }
}

fn app(state: token_service::TokenServerState) -> axum::Router {
    axum::Router::new()
        .route(
            "/tokens/access",
            axum::routing::post(token_service::routes::access::handle_access),
        )
        .with_state(state)
}

/// user_id 绑定 + 未过期的 client。
async fn seed_client(
    pool: &token_service::db::DbPool,
    client_id: &str,
    secret: &str,
    user_id: Option<&str>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) {
    let c = Client {
        client_id: client_id.into(),
        client_secret_hash: token_service::secret::hash_client_secret(secret),
        name: format!("{client_id}-name"),
        permissions: vec!["PM_MCP_READ".into(), "PM_MCP_TASK_UPDATE".into()],
        grant_types: vec!["client_credentials".into()],
        access_token_ttl_sec: 3600,
        refresh_token_ttl_sec: 2592000,
        status: "active".into(),
        user_id: user_id.map(|s| s.into()),
        expires_at,
    };
    token_service::service::oauth_client_svc::insert(pool, &c)
        .await
        .unwrap();
}

#[tokio::test]
async fn client_credentials_issues_token_for_bound_user() {
    let state = build_state().await;
    seed_client(&state.db, "ak-mcp", "mcp-secret", Some("usr-abc123"), None).await;

    // 表单编码（SDK 的实际格式）
    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(
            header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(Body::from(
            "grant_type=client_credentials&client_id=ak-mcp&client_secret=mcp-secret",
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v["access_token"].as_str().unwrap().len() > 20);
    // RFC 6749 §4.4.3: client_credentials 不签发 refresh token
    assert_eq!(v["refresh_token"].as_str().unwrap_or(""), "");
    assert_eq!(v["token_type"].as_str().unwrap(), "Bearer");

    // 方案 A 核心：token 的 sub = API Key 绑定的 user_id
    let at = v["access_token"].as_str().unwrap();
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(at, &cache, None).await.expect("verify access token");
    assert_eq!(claims["sub"], "usr-abc123");
    // 通道盖章：client_credentials 兑换的令牌 purpose=api_key
    assert_eq!(claims["purpose"], "api_key");
}

#[tokio::test]
async fn client_credentials_accepts_json_body_too() {
    let state = build_state().await;
    seed_client(&state.db, "ak-json", "json-secret", Some("usr-json"), None).await;

    // 既有调用方可能用 JSON —— 两种编码都要接受
    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "grant_type": "client_credentials",
                "client_id": "ak-json",
                "client_secret": "json-secret"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn client_credentials_rejects_machine_identity_client() {
    let state = build_state().await;
    // user_id 为 NULL 的 client 不能走 client_credentials（无 sub 可签）
    seed_client(&state.db, "ak-machine", "machine-secret", None, None).await;

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "grant_type=client_credentials&client_id=ak-machine&client_secret=machine-secret",
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn client_credentials_rejects_expired_key() {
    let state = build_state().await;
    seed_client(
        &state.db,
        "ak-expired",
        "expired-secret",
        Some("usr-expired"),
        Some(chrono::Utc::now() - chrono::Duration::days(1)),
    )
    .await;

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "grant_type=client_credentials&client_id=ak-expired&client_secret=expired-secret",
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn client_credentials_rejects_wrong_secret() {
    let state = build_state().await;
    seed_client(&state.db, "ak-wrong", "right-secret", Some("usr-wrong"), None).await;

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "grant_type=client_credentials&client_id=ak-wrong&client_secret=wrong-secret",
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn client_credentials_rejects_unknown_client() {
    let state = build_state().await;

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(
            "grant_type=client_credentials&client_id=ak-ghost&client_secret=nope",
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn authorization_code_still_works_with_json_body() {
    // 既有分支行为不能被新编码分发破坏
    let state = build_state().await;
    token_service::service::auth_code_svc::save(&state.db, "ac-1", "user-42", 3600)
        .await
        .unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/access")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "grant_type": "authorization_code",
                "code": "ac-1",
                "client_id": "no-such-client",
                "client_secret": "nope"
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app(state).oneshot(req).await.unwrap();
    // client 校验在 code 消费之前：unknown client → 401（与现行为一致）
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
