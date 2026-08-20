//! Phase 5B-fu3 T3: GET /iam/internal/membership-bootstrap endpoint tests.
//!
//! Verifies:
//! - happy path: full user→{role_ids, dept_id} mapping returned for service-jwt's tid
//! - tenant isolation: only users in the JWT tid tenant are returned
//! - dept mapping uses primary org_unit (is_primary=TRUE)
//! - role_ids empty when user has no role bindings
//! - 401 when service-jwt is missing or invalid (aud mismatch)

mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use digua_common_cache::MemoryCache;
use digua_common_event::MySqlOutboxRepo;
use digua_common_jwt::{sign_token, JWKSCache, ServiceJwtCache, ServiceJwtConfig};
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::routes::build_router;
use iam_service::state::AppState;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

const TEST_TOKEN_PRIVATE_PEM: &str =
    include_str!("../../token-service/tests/fixtures/test_token_private.pem");
const TEST_TOKEN_PUBLIC_PEM: &str =
    include_str!("../../token-service/tests/fixtures/test_token_public.pem");

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
    let jwks = Arc::new(JWKSCache::from_pem(TEST_TOKEN_PUBLIC_PEM).expect("test public key must parse"));
    let service_jwt = Arc::new(ServiceJwtCache::new(
        ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let state = AppState::new(pool, cache, config, None, outbox_repo, jwks, service_jwt);
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

async fn seed_user(state: &AppState, tenant: i64, uid: &str) {
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

async fn bind_user_to_org(state: &AppState, uid: &str, oid: &str, is_primary: bool) {
    sqlx::query(
        "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) \
         VALUES (?, ?, ?, NOW())",
    )
    .bind(uid).bind(oid).bind(is_primary)
    .execute(&state.db).await.unwrap();
}

fn mint_service_jwt(tid: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": "test-caller",
        "aud": "iam-service",
        "tid": tid,
        "iat": now,
        "exp": now + 3600,
        "roles": ["service"],
    });
    sign_token(TEST_TOKEN_PRIVATE_PEM, &claims, Duration::from_secs(3600))
        .expect("sign service-jwt")
}

async fn call(app: axum::Router, tid: i64) -> (StatusCode, serde_json::Value) {
    let jwt = mint_service_jwt(tid);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/iam/internal/membership-bootstrap")
                .header("Authorization", format!("Bearer {}", jwt))
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
async fn membership_bootstrap_returns_full_mapping() {
    let ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_role(&ctx.state, "r2").await;
    seed_org(&ctx.state, "d1").await;
    seed_org(&ctx.state, "d2").await;
    seed_user(&ctx.state, 1, "u1").await;
    seed_user(&ctx.state, 1, "u2").await;
    seed_user(&ctx.state, 1, "u3").await;
    bind_user_to_role(&ctx.state, "u1", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r2").await;
    bind_user_to_org(&ctx.state, "u1", "d1", true).await;
    bind_user_to_org(&ctx.state, "u2", "d2", true).await;

    let (status, body) = call(ctx.app, 1).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tenant_id"], 1);
    let mappings = body["mappings"].as_object().expect("mappings is an object");
    assert_eq!(mappings.len(), 3);

    let u1 = &mappings["u1"];
    let mut u1_roles: Vec<String> = u1["role_ids"].as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    u1_roles.sort();
    assert_eq!(u1_roles, vec!["r1"]);
    assert_eq!(u1["dept_id"], "d1");

    let u2 = &mappings["u2"];
    let mut u2_roles: Vec<String> = u2["role_ids"].as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    u2_roles.sort();
    assert_eq!(u2_roles, vec!["r1", "r2"]);
    assert_eq!(u2["dept_id"], "d2");

    // u3 has no role / no dept
    let u3 = &mappings["u3"];
    let u3_roles: Vec<String> = u3["role_ids"].as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(u3_roles.is_empty());
    assert!(u3["dept_id"].is_null());
}

#[tokio::test]
async fn membership_bootstrap_tenant_isolation() {
    let ctx = setup().await;
    seed_tenant(&ctx.state, 2, "t2").await;
    seed_user(&ctx.state, 1, "u_t1").await;
    seed_user(&ctx.state, 2, "u_t2").await;

    let (status, body) = call(ctx.app.clone(), 1).await;
    assert_eq!(status, StatusCode::OK);
    let mappings_t1 = body["mappings"].as_object().unwrap();
    assert!(mappings_t1.contains_key("u_t1"));
    assert!(!mappings_t1.contains_key("u_t2"));

    let (status2, body2) = call(ctx.app, 2).await;
    assert_eq!(status2, StatusCode::OK);
    let mappings_t2 = body2["mappings"].as_object().unwrap();
    assert!(mappings_t2.contains_key("u_t2"));
    assert!(!mappings_t2.contains_key("u_t1"));
}

#[tokio::test]
async fn membership_bootstrap_uses_primary_dept() {
    let ctx = setup().await;
    seed_org(&ctx.state, "primary_dept").await;
    seed_org(&ctx.state, "secondary_dept").await;
    seed_user(&ctx.state, 1, "u1").await;
    bind_user_to_org(&ctx.state, "u1", "secondary_dept", false).await;
    bind_user_to_org(&ctx.state, "u1", "primary_dept", true).await;

    let (status, body) = call(ctx.app, 1).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["mappings"]["u1"]["dept_id"], "primary_dept");
}

#[tokio::test]
async fn membership_bootstrap_skips_deleted_users() {
    let ctx = setup().await;
    seed_user(&ctx.state, 1, "u_alive").await;
    seed_user(&ctx.state, 1, "u_dead").await;
    sqlx::query("UPDATE iam_users SET deleted_at = NOW() WHERE id = ?")
        .bind("u_dead")
        .execute(&ctx.state.db).await.unwrap();

    let (status, body) = call(ctx.app, 1).await;
    assert_eq!(status, StatusCode::OK);
    let mappings = body["mappings"].as_object().unwrap();
    assert!(mappings.contains_key("u_alive"));
    assert!(!mappings.contains_key("u_dead"));
}

#[tokio::test]
async fn membership_bootstrap_no_users_returns_empty_mappings() {
    let ctx = setup().await;
    let (status, body) = call(ctx.app, 1).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tenant_id"], 1);
    let mappings = body["mappings"].as_object().unwrap();
    assert!(mappings.is_empty());
}

#[tokio::test]
async fn membership_bootstrap_rejects_wrong_aud() {
    let ctx = setup().await;
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service", "sub": "test-caller",
        "aud": "wrong-service",  // not iam-service
        "tid": 1, "iat": now, "exp": now + 3600, "roles": ["service"],
    });
    let bad_jwt = sign_token(TEST_TOKEN_PRIVATE_PEM, &claims, Duration::from_secs(3600)).unwrap();
    let resp = ctx.app
        .oneshot(
            Request::builder()
                .uri("/iam/internal/membership-bootstrap")
                .header("Authorization", format!("Bearer {}", bad_jwt))
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn membership_bootstrap_rejects_missing_auth() {
    let ctx = setup().await;
    let resp = ctx.app
        .oneshot(
            Request::builder()
                .uri("/iam/internal/membership-bootstrap")
                .body(Body::empty()).unwrap()
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// Suppress unused-import warnings for HashMap (test fixture consistency)
#[allow(dead_code)]
fn _unused() -> HashMap<String, String> { HashMap::new() }