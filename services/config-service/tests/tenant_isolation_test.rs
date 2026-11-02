//! Phase 8.3-fu2: platform_admin cross-tenant namespace isolation (HTTP path).
//!
//! Validates that the config-service correctly applies tenant_id filter:
//! - platform_admin (cross_tenant=true) sees namespaces from any tenant
//! - regular tenant-scoped user only sees namespaces in their own tenant
#![allow(deprecated)]
mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use config_service::routes::{self, create_state};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::Principal;
use jwt_simple::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
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

fn signing_key() -> &'static RS256KeyPair {
    static KEY: OnceLock<RS256KeyPair> = OnceLock::new();
    KEY.get_or_init(|| RS256KeyPair::generate(2048).unwrap())
}

fn test_public_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| signing_key().public_key().to_pem().unwrap())
}

fn build_app(pool: sqlx::MySqlPool) -> axum::Router {
    let jwks =
        Arc::new(JWKSCache::from_pem(test_public_pem()).expect("test public PEM must parse"));
    // `create_state` 自 DbPool 改造后接受方言无关句柄；MySQL 测试直接 From 转换即可。
    let state = create_state(pool.into(), jwks);
    routes::routes(state)
}

/// Insert a namespace row directly, bypassing create flow (faster, isolates HTTP-path test).
async fn seed_namespace(pool: &sqlx::MySqlPool, tenant_id: i64, app: &str) -> String {
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

    signing_key().sign(claims).unwrap()
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

/// Regression for the `query_namespaces` SQL-assembly bug (`WHERE 1=1` was
/// missing, so tenant-scoped callers got HTTP 500 "query failed").
#[tokio::test]
async fn tenant_scoped_list_filters_cross_tenant() {
    let ctx = setup().await;

    let t1_a = seed_namespace(&ctx.pool, 1, "list-t1-app-a").await;
    let t1_b = seed_namespace(&ctx.pool, 1, "list-t1-app-b").await;
    let t2_a = seed_namespace(&ctx.pool, 2, "list-t2-app").await;

    let t1_token = make_token("list-user-1", 1, false);
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/config/namespace?page=1&size=100")
                .header("Authorization", format!("Bearer {}", t1_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = resp.status();
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        panic!(
            "tenant-1 list status={} body={:?}",
            st,
            String::from_utf8_lossy(&body)
        )
    });
    assert_eq!(
        st,
        StatusCode::OK,
        "tenant-1 list must return 200 (was 500 before WHERE 1=1 fix): body={:?}",
        info
    );
    let items = info["items"].as_array().expect("items array").clone();
    assert!(
        !items.is_empty(),
        "tenant-1 list must include own seeded namespaces"
    );
    for ns in &items {
        assert_eq!(
            ns["tenant_id"].as_u64().unwrap(),
            1,
            "tenant-1 list must NOT include tenant-2 namespaces: {:?}",
            ns
        );
    }
    let ids: HashSet<String> = items
        .iter()
        .map(|ns| ns["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&t1_a),
        "tenant-1 must see own seed a ({} in {:?})",
        t1_a,
        ids
    );
    assert!(
        ids.contains(&t1_b),
        "tenant-1 must see own seed b ({} in {:?})",
        t1_b,
        ids
    );
    assert!(
        !ids.contains(&t2_a),
        "tenant-1 must NOT see tenant-2 seed ({} should be absent from {:?})",
        t2_a,
        ids
    );

    let t2_token = make_token("list-user-2", 2, false);
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/config/namespace?page=1&size=100")
                .header("Authorization", format!("Bearer {}", t2_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let st = resp.status();
    let body = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
    let info: serde_json::Value = serde_json::from_slice(&body).unwrap_or_else(|_| {
        panic!(
            "tenant-2 list status={} body={:?}",
            st,
            String::from_utf8_lossy(&body)
        )
    });
    assert_eq!(
        st,
        StatusCode::OK,
        "tenant-2 list must return 200: body={:?}",
        info
    );
    let items = info["items"].as_array().expect("items array").clone();
    assert!(
        !items.is_empty(),
        "tenant-2 list must include own seeded namespace"
    );
    for ns in &items {
        assert_eq!(
            ns["tenant_id"].as_u64().unwrap(),
            2,
            "tenant-2 list must NOT include tenant-1 namespaces: {:?}",
            ns
        );
    }
    let ids: HashSet<String> = items
        .iter()
        .map(|ns| ns["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&t2_a),
        "tenant-2 must see own seed ({} in {:?})",
        t2_a,
        ids
    );
    assert!(
        !ids.contains(&t1_a) && !ids.contains(&t1_b),
        "tenant-2 must NOT see tenant-1 seeds (got {:?})",
        ids
    );
}
