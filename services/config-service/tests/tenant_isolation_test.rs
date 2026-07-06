//! Phase 8.3-fu2: platform_admin cross-tenant namespace isolation (HTTP path).
//!
//! Validates that the config-service correctly applies tenant_id filter:
//! - platform_admin (cross_tenant=true) sees namespaces from any tenant
//! - regular tenant-scoped user only sees namespaces in their own tenant
mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use config_service::routes::{self, create_state};
use jwt_ctx::Principal;
use jwt_simple::prelude::*;
use std::collections::HashMap;
use tower::ServiceExt;
use uuid::Uuid;

struct Ctx {
    app: axum::Router,
    pool: sqlx::MySqlPool,
}

async fn setup() -> Ctx {
    let h = common::spawn_mysql().await;
    let pool = h.pool;
    let app = build_app(pool.clone());
    Ctx { app, pool }
}

fn build_app(pool: sqlx::MySqlPool) -> axum::Router {
    let state = create_state(pool);
    routes::routes(state)
}

/// Insert a namespace row directly, bypassing create flow (faster, isolates HTTP-path test).
async fn seed_namespace(pool: &sqlx::MySqlPool, tenant_id: u64, app: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let profile = format!("profile-{}", id);
    sqlx::query(
        r#"INSERT INTO cfg_namespaces
           (id, tenant_id, app, env, profile, description, created_at, updated_at)
           VALUES (?, ?, ?, 'prod', ?, NULL, NOW(), NOW())"#,
    )
    .bind(&id)
    .bind(tenant_id)
    .bind(app)
    .bind(&profile)
    .execute(pool)
    .await
    .expect("seed namespace row");
    id
}

/// Build a JWT with the given tenant_id and cross_tenant flag.
fn make_token(user_id: &str, tenant_id: i64, cross_tenant: bool) -> String {
    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut custom = HashMap::new();
    custom.insert("tid".to_string(), serde_json::json!(tenant_id));
    custom.insert("ct".to_string(), serde_json::json!(cross_tenant));

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(user_id.to_string());
    claims.issued_at = Some(Duration::from_secs(now));
    claims.expires_at = Some(Duration::from_secs(now + 3600));
    claims.custom = custom;

    key_pair.sign(claims).unwrap()
}

/// Verify the JWT roundtrips through Principal correctly (sanity check on test setup).
fn verify_principal(token: &str, expected_tenant: i64, expected_cross: bool) {
    let p = Principal::from_jwt_payload(token).expect("decode principal");
    assert_eq!(p.tenant_id, expected_tenant, "tenant_id mismatch");
    assert_eq!(p.cross_tenant, expected_cross, "cross_tenant mismatch");
}

#[tokio::test]
async fn platform_admin_cross_tenant_namespace() {
    let ctx = setup().await;

    let t1_ns = seed_namespace(&ctx.pool, 1, "acme-app").await;
    let t2_ns = seed_namespace(&ctx.pool, 2, "globex-app").await;

    // 1. platform_admin (cross_tenant=true) reads both namespaces
    let admin_token = make_token("platform_admin", 1, true);
    verify_principal(&admin_token, 1, true);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t1_ns))
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = resp.status();
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        panic!(
            "t1 GET status={} body={:?}",
            st,
            String::from_utf8_lossy(&body)
        )
    });
    assert_eq!(
        st,
        StatusCode::OK,
        "platform_admin must read t1 namespace: body={:?}",
        info
    );
    assert_eq!(info["tenant_id"], 1);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t2_ns))
                .header("Authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = resp.status();
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        panic!(
            "t2 GET status={} body={:?}",
            st,
            String::from_utf8_lossy(&body)
        )
    });
    assert_eq!(st, StatusCode::OK, "platform_admin must read t2 namespace");
    assert_eq!(info["tenant_id"], 2);

    // 2. tenant-1 user cannot read t2 namespace (and_tenant filter)
    let t1_token = make_token("user-1", 1, false);
    verify_principal(&t1_token, 1, false);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t2_ns))
                .header("Authorization", format!("Bearer {}", t1_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "tenant-1 user must not see tenant-2 namespace"
    );

    // 3. tenant-1 user CAN read their own namespace
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t1_ns))
                .header("Authorization", format!("Bearer {}", t1_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "tenant-1 user reads own namespace"
    );

    // 4. tenant-2 user cannot read t1 namespace
    let t2_token = make_token("user-2", 2, false);
    verify_principal(&t2_token, 2, false);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t1_ns))
                .header("Authorization", format!("Bearer {}", t2_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "tenant-2 user must not see tenant-1 namespace"
    );

    // 5. unauthenticated request returns 401 (Principal extractor requires Authorization header)
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/config/namespace/{}", t1_ns))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "missing Authorization must 401"
    );
}
