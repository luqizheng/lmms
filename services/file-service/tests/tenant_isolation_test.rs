//! Phase 8.3-fu2: platform_admin cross-tenant file isolation (HTTP path).
//!
//! Validates that the file-service correctly applies tenant_id filter:
//! - platform_admin (cross_tenant=true) sees files from any tenant
//! - regular tenant-scoped user only sees files in their own tenant
#![allow(deprecated)]
mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use digua_common_jwt::JWKSCache;
use digua_jwt_ctx::Principal;
use file_service::routes::{self, create_state};
use file_service::storage::ObjectStore;
use jwt_simple::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

struct Ctx {
    app: axum::Router,
    pool: sqlx::MySqlPool,
    key_pair: Arc<RS256KeyPair>,
}

async fn setup() -> Ctx {
    let h = common::spawn_mysql().await;
    let pool = h.pool;
    let key_pair = Arc::new(RS256KeyPair::generate(2048).unwrap());
    // 注册公钥到 JWKSCache：HTTP 路径上 Principal::from_request_parts 会
    // verify_token，cache 找不到公钥就 401。
    let public_pem = key_pair.public_key().to_pem().unwrap();
    let jwks = Arc::new(JWKSCache::from_pem(&public_pem).unwrap());
    let app = build_app(pool.clone(), h.storage, jwks);
    Ctx {
        app,
        pool,
        key_pair,
    }
}

fn build_app(
    pool: sqlx::MySqlPool,
    storage: Arc<dyn ObjectStore>,
    jwks: Arc<JWKSCache>,
) -> axum::Router {
    let state = create_state(
        pool,
        storage,
        "http://test/file/".into(),
        100 * 1024 * 1024,
        jwks,
    );
    routes::routes(state)
}

/// Insert a file row directly, bypassing upload flow (faster, isolates HTTP-path test).
async fn seed_file(pool: &sqlx::MySqlPool, tenant_id: i64) -> String {
    let id = Uuid::new_v4().to_string();
    let storage_path = format!("test/{}-seed.txt", id);
    sqlx::query(
        r#"INSERT INTO file_files
           (id, filename, content_type, size, hash, storage_path, tenant_id, user_id, is_temporary, expires_at, created_at, updated_at)
           VALUES (?, 'seed.txt', 'text/plain', 0, NULL, ?, ?, 'seed-user', 0, NULL, NOW(), NOW())"#,
    )
    .bind(&id)
    .bind(&storage_path)
    .bind(tenant_id)
    .execute(pool)
    .await
    .expect("seed file row");
    id
}

/// Build a JWT with the given tenant_id and cross_tenant flag.
fn make_token(
    key_pair: &RS256KeyPair,
    user_id: &str,
    tenant_id: i64,
    cross_tenant: bool,
) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut custom = HashMap::new();
    custom.insert("tid".to_string(), serde_json::json!(tenant_id));
    custom.insert("ct".to_string(), serde_json::json!(cross_tenant));

    let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
    claims.subject = Some(user_id.to_string());
    claims.issued_at = Some(coarsetime::Duration::from_secs(now));
    claims.expires_at = Some(coarsetime::Duration::from_secs(now + 3600));
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
async fn platform_admin_cross_tenant_file() {
    let ctx = setup().await;

    let t1_file = seed_file(&ctx.pool, 1).await;
    let t2_file = seed_file(&ctx.pool, 2).await;

    // 1. platform_admin (cross_tenant=true) reads both files
    let admin_token = make_token(&ctx.key_pair, "platform_admin", 1, true);
    verify_principal(&admin_token, 1, true);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/{}/info", t1_file))
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
        "platform_admin must read t1 file: body={:?}",
        info
    );
    assert_eq!(info["tenant_id"], 1);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/{}/info", t2_file))
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
    assert_eq!(st, StatusCode::OK, "platform_admin must read t2 file");
    assert_eq!(info["tenant_id"], 2);

    // 2. tenant-1 user cannot read t2 file (and_tenant filter)
    let t1_token = make_token(&ctx.key_pair, "user-1", 1, false);
    verify_principal(&t1_token, 1, false);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/{}/info", t2_file))
                .header("Authorization", format!("Bearer {}", t1_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "tenant-1 user must not see tenant-2 file"
    );

    // 3. tenant-1 user CAN read their own file
    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/{}/info", t1_file))
                .header("Authorization", format!("Bearer {}", t1_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "tenant-1 user reads own file"
    );

    // 4. tenant-2 user cannot read t1 file
    let t2_token = make_token(&ctx.key_pair, "user-2", 2, false);
    verify_principal(&t2_token, 2, false);

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/file/{}/info", t1_file))
                .header("Authorization", format!("Bearer {}", t2_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "tenant-2 user must not see tenant-1 file"
    );
}
