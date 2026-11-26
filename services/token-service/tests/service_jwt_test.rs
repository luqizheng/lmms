//! Phase 5-B: `POST /tokens/service-jwt` integration test.
//!
//! Phase 2 演进 (2026-11-09): bootstrap token 校验从内存 Vec 改为查
//! `token_bootstrap_tokens` 表，因此本测试需要真实 MySQL——用 `common::spawn_mysql()`
//! 起容器 / 连共享库，跑完 migrations 后往表里插行再打请求。
//!
//! `sub` = 命中的行的 `owner`（不再有 SERVICE_BOOTSTRAP_OWNERS 平行列表）。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::db_execute;
use digua_common_jwt::verify_token;
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;

use common::{build_state, TEST_PUBLIC_PEM};

/// 往 bootstrap token 表插行。
async fn insert_tokens(pool: &token_service::db::DbPool, rows: &[(&str, &str, &str)]) {
    for (owner, token, status) in rows {
        db_execute!(
            pool,
            "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, ?)",
            *owner,
            *token,
            *status
        )
        .expect("insert bootstrap token");
    }
}

/// 往 grant 表插一条显式授权（WP-LMMS-SERVICE-JWT-AUDIENCE-02 后，
/// 签发前必须存在 active owner→audience grant）。
async fn insert_grant(pool: &token_service::db::DbPool, owner: &str, audience: &str) {
    db_execute!(
        pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        owner,
        audience
    )
    .expect("insert grant");
}

#[tokio::test]
async fn service_jwt_with_valid_bootstrap_returns_signed_jwt() {
    let pool = crate::common::spawn_mysql().await;
    insert_tokens(
        &pool,
        &[
            ("iam-service", "bt-iam", "active"),
            ("token-service", "bt-token", "active"),
        ],
    )
    .await;
    // WP-02：iam-service → token-service 是 P0 系统 grant（登录链）
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure system grants");
    let app = token_service::service_jwt_routes(build_state(pool));

    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer bt-iam")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "token-service", "tid": 1}).to_string(),
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
    let claims = verify_token(access_token, &cache, Some("token-service"))
        .await
        .unwrap();
    // service-jwt always issued by token-service
    assert_eq!(claims["iss"], "token-service");
    // sub = 命中行的 owner（bt-iam → iam-service）

    assert_eq!(claims["sub"], "iam-service");
    assert_eq!(claims["aud"], "token-service");
    assert_eq!(claims["tid"], 1);
    assert_eq!(claims["roles"][0], "service");
    assert_eq!(claims["purpose"], "service");
}

#[tokio::test]
async fn service_jwt_with_wrong_bootstrap_returns_401() {
    let pool = crate::common::spawn_mysql().await;
    insert_tokens(&pool, &[("iam-service", "bt-iam", "active")]).await;
    let app = token_service::service_jwt_routes(build_state(pool));

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
    let pool = crate::common::spawn_mysql().await;
    insert_tokens(&pool, &[("iam-service", "bt-iam", "active")]).await;
    let app = token_service::service_jwt_routes(build_state(pool));

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
    let pool = crate::common::spawn_mysql().await;
    insert_tokens(&pool, &[("iam-service", "bt-iam", "active")]).await;
    let app = token_service::service_jwt_routes(build_state(pool));

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
async fn service_jwt_ct_claim_and_owner_from_db() {
    let pool = crate::common::spawn_mysql().await;
    insert_tokens(&pool, &[("project-manager", "bt-pm", "active")]).await;
    // WP-02：显式授予 project-manager → file-service（曾是"任意 audience 放行"
    // 的旧语义用例，现必须先有 grant 才能签发）
    insert_grant(&pool, "project-manager", "file-service").await;
    let app = token_service::service_jwt_routes(build_state(pool));

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
    let claims = verify_token(
        parsed["access_token"].as_str().unwrap(),
        &cache,
        Some("file-service"),
    )
    .await
    .unwrap();
    assert_eq!(claims["ct"], true);
    assert_eq!(claims["tid"], 42);
    // sub 来自 DB 行的 owner
    assert_eq!(claims["sub"], "project-manager");
}

/// 非 active 状态（disabled）与已过期（expires_at < NOW()）的 token 必须被拒。
#[tokio::test]
async fn service_jwt_rejects_disabled_and_expired_tokens() {
    let pool = crate::common::spawn_mysql().await;
    db_execute!(
        &pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, 'disabled')",
        "old-service",
        "bt-disabled"
    )
    .expect("insert disabled token");
    db_execute!(
        &pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status, expires_at) \
         VALUES (?, ?, 'active', NOW() - INTERVAL 1 HOUR)",
        "stale-service",
        "bt-expired"
    )
    .expect("insert expired token");

    let state = build_state(pool);
    let app = token_service::service_jwt_routes(state);

    for token in ["bt-disabled", "bt-expired"] {
        let req = Request::builder()
            .method("POST")
            .uri("/tokens/service-jwt")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"aud": "iam-service", "tid": 1}).to_string(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "token {token} should be rejected"
        );
    }
}

/// 启动 seed：env 未配置时用内置 dev 默认值灌入表。
#[tokio::test]
async fn seed_defaults_falls_back_to_built_in_dev_tokens() {
    let pool = crate::common::spawn_mysql().await;
    let inserted = token_service::models::bootstrap_token::seed_defaults(&pool, "", "").await;
    assert!(
        inserted.expect("seed") > 0,
        "should insert built-in defaults"
    );

    // 内置默认值里的 token 可以换到 service-jwt，且 sub = 对应 owner
    // WP-02：请求的必须是已授权 audience（iam-service → token-service，P0）
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure system grants");
    let state = build_state(pool);
    let app = token_service::service_jwt_routes(state);
    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, "Bearer dev-bt-iam-service")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"aud": "token-service", "tid": 1}).to_string(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
    let parsed: Value = serde_json::from_slice(&body).unwrap();
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(
        parsed["access_token"].as_str().unwrap(),
        &cache,
        Some("token-service"),
    )
    .await
    .unwrap();
    assert_eq!(claims["sub"], "iam-service");
}

/// 启动 seed：幂等 + 不覆盖已有 owner 的行（运维 rotate 过的 token 保留）。
#[tokio::test]
async fn seed_defaults_is_idempotent_and_does_not_overwrite_existing_owner() {
    let pool = crate::common::spawn_mysql().await;
    // 预置一行 owner=iam-service，token 是运维自己 rotate 过的
    db_execute!(
        &pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, 'active')",
        "iam-service",
        "rotated-token"
    )
    .expect("insert pre-existing row");

    // 第一次 seed：只补插缺失的 owner
    let first = token_service::models::bootstrap_token::seed_defaults(&pool, "", "")
        .await
        .expect("seed first");
    assert!(first > 0);

    // 第二次 seed：全部 owner 都已存在 → 0 插入（幂等）
    let second = token_service::models::bootstrap_token::seed_defaults(&pool, "", "")
        .await
        .expect("seed second");
    assert_eq!(second, 0);

    // 预置行的 token 未被覆盖
    let row: Option<(String,)> = digua_common_db::db_fetch_optional!(
        &pool,
        (String,),
        "SELECT token_hash FROM token_bootstrap_tokens WHERE owner = ?",
        "iam-service"
    )
    .expect("query token_hash");
    assert_eq!(row.map(|(t,)| t).as_deref(), Some("rotated-token"));
}

/// env 提供的 token 优先于内置默认值。
#[tokio::test]
async fn seed_defaults_prefers_env_pairs() {
    let pool = crate::common::spawn_mysql().await;
    let inserted = token_service::models::bootstrap_token::seed_defaults(
        &pool,
        "env-token-a,env-token-b",
        "svc-a,svc-b",
    )
    .await
    .expect("seed from env");
    assert_eq!(inserted, 2);

    let rows: Vec<(String, String)> = digua_common_db::db_fetch_all!(
        &pool,
        (String, String),
        "SELECT owner, token_hash FROM token_bootstrap_tokens ORDER BY owner"
    )
    .expect("query rows");
    assert_eq!(
        rows,
        vec![
            ("svc-a".to_string(), "env-token-a".to_string()),
            ("svc-b".to_string(), "env-token-b".to_string()),
        ]
    );
}

/// fail-closed：配了 tokens 但 owners 缺失 / 长度不等 → seed 报错，
/// 且**不写入任何行**（绝不能静默回退到内置 dev token）。
#[tokio::test]
async fn seed_defaults_rejects_misconfigured_env_without_writing() {
    let pool = crate::common::spawn_mysql().await;
    let res =
        token_service::models::bootstrap_token::seed_defaults(&pool, "tok-a,tok-b,tok-c", "svc-a")
            .await;
    assert!(res.is_err(), "mismatched env must be rejected");

    // 表里一行都不能有
    let count: i64 = digua_common_db::db_fetch_scalar!(
        &pool,
        i64,
        "SELECT COUNT(*) FROM token_bootstrap_tokens"
    )
    .expect("count");
    assert_eq!(count, 0, "must not write anything on config error");
}
