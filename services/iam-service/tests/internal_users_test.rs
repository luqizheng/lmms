//! Phase 4.4-fu2 + 4.3-fu2: tests for /iam/internal/users/by-{role,dept} endpoints.
//! Mesh intentions enforce service identity (Phase 5-A);
//! tenant_id from query param; cross-tenant reads forbidden.

mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use digua_common_cache::MemoryCache;
use digua_common_event::MySqlOutboxRepo;
use digua_common_jwt::sign_token;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::service::token_forward::TokenForwardClient;
use iam_service::routes::build_router;
use iam_service::state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

// Test fixture: matches the production token-service private key (regenerated
// for tests; production uses the baked-in PEM).
const TEST_TOKEN_PRIVATE_PEM: &str = include_str!("../../token-service/tests/fixtures/test_token_private.pem");

struct Ctx {
    app: axum::Router,
    state: AppState,
    _mysql: Option<common::MysqlContainer>,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let url = mysql_h.url;

    let config = Config {
        app: AppConfig { name: "iam-service".into(), host: "0.0.0.0".into(), port: 8080 },
        database: DatabaseConfig { url: url.clone(), max_connections: 5, min_connections: 1 },
        redis: RedisConfig { url: "redis://x".into(), ttl_sec: 86400, prefix: "test:".into() },
        jwt: JwtConfig {
            private_key_path: "/dev/null".into(),
            public_key_path: "/dev/null".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig { host: "consul".into(), port: 8500, service_name: "iam-service".into() },
    };
    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let jwks = std::sync::Arc::new(digua_common_jwt::JWKSCache::from_pem(
        include_str!("../../token-service/tests/fixtures/test_token_public.pem"),
    ).expect("test public key must parse"));
    let service_jwt = std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));    let token_forward = Arc::new(TokenForwardClient::new(service_jwt.clone(), String::new()));
    let state = AppState::new(pool, cache, config, None, outbox_repo, jwks, service_jwt, token_forward);

    let app = build_router(state.clone());
    Ctx { app, state, _mysql: mysql_h.container }
}

async fn seed_tenant(state: &AppState, id: i64, slug: &str) {
    sqlx::query(
        "INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'active', NOW(), NOW()) \
         ON DUPLICATE KEY UPDATE id = id",
    )
    .bind(id).bind(slug).bind(slug)
    .execute(&state.db).await.unwrap();
}

async fn seed_tenant_user(state: &AppState, tenant: i64, uid: &str) {
    sqlx::query(
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'h', 's', ?, 'active', NOW(), NOW())",
    )
    .bind(uid).bind(tenant).bind(uid).bind(uid)
    .execute(&state.db).await.unwrap();
}

async fn seed_role(state: &AppState, rid: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'self', 'active', NOW(), NOW())",
    )
    .bind(rid).bind(rid).bind(rid)
    .execute(&state.db).await.unwrap();
}

async fn seed_org(state: &AppState, oid: &str) {
    sqlx::query(
        "INSERT INTO iam_org_units (id, name, code, parent_id, created_at, updated_at) \
         VALUES (?, ?, ?, NULL, NOW(), NOW())",
    )
    .bind(oid).bind(oid).bind(oid)
    .execute(&state.db).await.unwrap();
}

async fn bind_user_to_role(state: &AppState, uid: &str, rid: &str) {
    sqlx::query("INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())")
        .bind(uid).bind(rid)
        .execute(&state.db).await.unwrap();
}

async fn bind_user_to_org(state: &AppState, uid: &str, oid: &str) {
    sqlx::query(
        "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) \
         VALUES (?, ?, TRUE, NOW())",
    )
    .bind(uid).bind(oid)
    .execute(&state.db).await.unwrap();
}

async fn call(app: axum::Router, path: &str, tenant_id: i64) -> (StatusCode, serde_json::Value) {
    let uri = format!("{}?tenant_id={}", path, tenant_id);
    // Phase 5-B: /iam/internal/* requires service-jwt with aud=iam-service.
    // Mint one using the test fixture private key.
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": "test-caller",
        "aud": "iam-service",
        "tid": 1i64,  // JWT must always have valid i64; the path's tid
                       // (which may be "abc" or missing) is tested separately
        "iat": now,
        "exp": now + 3600,
        "roles": ["service"],
    });
    let service_jwt = sign_token(TEST_TOKEN_PRIVATE_PEM, &claims, Duration::from_secs(3600))
        .expect("sign service-jwt");
    let resp = app
        .oneshot(
            Request::builder()
                .uri(&uri)
                .header("Authorization", format!("Bearer {}", service_jwt))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body_bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = if body_bytes.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(serde_json::json!({}))
    };
    (status, json)
}

#[tokio::test]
async fn internal_users_by_role_returns_member_user_ids() {
    let ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 1, "u2").await;
    bind_user_to_role(&ctx.state, "u1", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-role/r1", 1).await;
    assert_eq!(status, StatusCode::OK);
    let mut items: Vec<String> = body["items"]
        .as_array().unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    items.sort();
    assert_eq!(items, vec!["u1", "u2"]);
}

#[tokio::test]
async fn internal_users_by_dept_returns_member_user_ids() {
    let ctx = setup().await;
    seed_org(&ctx.state, "o1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 1, "u2").await;
    bind_user_to_org(&ctx.state, "u1", "o1").await;
    bind_user_to_org(&ctx.state, "u2", "o1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-dept/o1", 1).await;
    assert_eq!(status, StatusCode::OK);
    let count = body["items"].as_array().unwrap().len();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn internal_users_tenant_isolation() {
    let ctx = setup().await;
    seed_tenant(&ctx.state, 2, "t2").await;
    seed_role(&ctx.state, "r1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 2, "u2").await;
    bind_user_to_role(&ctx.state, "u1", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-role/r1", 2).await;
    assert_eq!(status, StatusCode::OK);
    let items: Vec<String> = body["items"]
        .as_array().unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(items, vec!["u2"]);
}

#[tokio::test]
async fn missing_tenant_id_returns_400() {
    let ctx = setup().await;
    // Phase 5-B: /iam/internal/* requires service-jwt. Mint one with tid=1,
    // but the query string is intentionally missing tenant_id to test
    // the handler's 400 response (not the middleware's 401).
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service", "sub": "test-caller", "aud": "iam-service",
        "tid": 1i64, "iat": now, "exp": now + 3600, "roles": ["service"],
    });
    let service_jwt = sign_token(TEST_TOKEN_PRIVATE_PEM, &claims, Duration::from_secs(3600)).unwrap();
    let resp = ctx.app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/iam/internal/users/by-role/r1")
                .header("Authorization", format!("Bearer {}", service_jwt))
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn non_numeric_tenant_id_returns_400() {
    let ctx = setup().await;
    // Phase 5-B: service-jwt with valid tid=1; query has tenant_id=abc
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service", "sub": "test-caller", "aud": "iam-service",
        "tid": 1i64, "iat": now, "exp": now + 3600, "roles": ["service"],
    });
    let service_jwt = sign_token(TEST_TOKEN_PRIVATE_PEM, &claims, Duration::from_secs(3600)).unwrap();
    let resp = ctx.app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/iam/internal/users/by-role/r1?tenant_id=abc")
                .header("Authorization", format!("Bearer {}", service_jwt))
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn valid_tenant_id_returns_200() {
    let ctx = setup().await;
    let (status, _) = call(ctx.app.clone(), "/iam/internal/users/by-role/r1", 1).await;
    assert_eq!(status, StatusCode::OK);
}
