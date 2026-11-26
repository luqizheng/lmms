//! Phase 3: `/admin/bootstrap-tokens*` 管理端点集成测试。
//!
//! 覆盖：list / get / create / rotate / set_status / delete + 鉴权。
//! 鉴权走 `verify_aud_principal_middleware("token-service")` —— 必须携带
//! `aud=token-service` 的 service-jwt，用 `token_service::mint_service_bearer` 签。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::db_execute;
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;

use common::{build_state, TEST_PRIVATE_PEM};

/// 构造带鉴权的 admin router（与 main.rs 挂载方式一致）。
fn admin_app(pool: token_service::db::DbPool) -> axum::Router {
    token_service::routes::bootstrap_token_admin::routes(build_state(pool))
}

/// 签一个 aud=token-service 的 service-jwt。
fn admin_bearer() -> String {
    token_service::mint_service_bearer(TEST_PRIVATE_PEM, "token-service", 3600)
        .expect("mint service bearer")
}

/// 往表里插一行。
async fn insert(pool: &token_service::db::DbPool, owner: &str, token: &str, status: &str) {
    db_execute!(
        pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, ?)",
        owner,
        token,
        status
    )
    .expect("insert bootstrap token");
}

fn req(method: &str, uri: &str, bearer: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = bearer {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    b.body(match body {
        Some(v) => Body::from(v.to_string()),
        None => Body::empty(),
    })
    .expect("build request")
}

async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

#[tokio::test]
async fn list_requires_service_jwt() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    let resp = app
        .oneshot(req("GET", "/admin/bootstrap-tokens", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_rejects_wrong_audience() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    // aud=iam-service 的 token 不能访问 aud=token-service 的端点
    let wrong = token_service::mint_service_bearer(TEST_PRIVATE_PEM, "iam-service", 3600).unwrap();
    let resp = app
        .oneshot(req("GET", "/admin/bootstrap-tokens", Some(&wrong), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn create_then_list_and_get() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    // create
    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/admin/bootstrap-tokens",
            Some(&bearer),
            Some(json!({"owner": "my-new-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["owner"], "my-new-service");
    let token = body["token"].as_str().expect("token").to_string();
    assert_eq!(token.len(), 40, "generated token should be 40 chars");

    // list —— 必须**不含**明文 token
    let resp = app
        .clone()
        .oneshot(req("GET", "/admin/bootstrap-tokens", Some(&bearer), None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["total"], 1);
    let item = &body["items"][0];
    assert_eq!(item["owner"], "my-new-service");
    assert_eq!(item["status"], "active");
    assert_eq!(item["is_grace_row"], false);
    // 列表里不能出现 token 字段
    assert!(
        item.get("token").is_none(),
        "list must not leak plaintext token"
    );
    assert!(
        item.get("token_hash").is_none(),
        "list must not leak token_hash"
    );

    // get 单条
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/bootstrap-tokens/my-new-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["owner"], "my-new-service");
    assert!(body.get("token").is_none());
}

#[tokio::test]
async fn create_rejects_duplicate_owner() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();

    let resp = app
        .oneshot(req(
            "POST",
            "/admin/bootstrap-tokens",
            Some(&bearer),
            Some(json!({"owner": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_rejects_invalid_owner() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    for bad in ["", "has space", "has#hash", "has'quote"] {
        let resp = app
            .clone()
            .oneshot(req(
                "POST",
                "/admin/bootstrap-tokens",
                Some(&bearer),
                Some(json!({"owner": bad})),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "owner={bad:?}");
    }
}

/// rotate 后旧 token 立即失效，新 token 可用。
#[tokio::test]
async fn rotate_without_grace_invalidates_old_token_immediately() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "old-token", "active").await;
    // 签发联动断言需要 grant（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01 起默认拒绝）；
    // 内置系统 seed 覆盖 iam-service → iam-service。
    token_service::models::service_jwt_grant::seed_system_grants(&pool, "")
        .await
        .expect("seed system grants");
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();

    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/admin/bootstrap-tokens/iam-service/rotate",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let new_token = body["token"].as_str().expect("token").to_string();
    assert_ne!(new_token, "old-token");
    assert!(
        body["old_token_expires_at"].is_null(),
        "no grace period -> old token expires immediately"
    );

    // 旧 token 不再能换到 service-jwt
    let resp = token_service::service_jwt_routes(build_state(pool.clone()))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("old-token"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 新 token 可以
    let resp = token_service::service_jwt_routes(build_state(pool))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some(&new_token),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// rotate 带宽限期：旧 token 在宽限期内仍可用。
#[tokio::test]
async fn rotate_with_grace_period_keeps_old_token_valid() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "old-token", "active").await;
    // 签发联动断言需要 grant（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01 起默认拒绝）；
    // 内置系统 seed 覆盖 iam-service → iam-service。
    token_service::models::service_jwt_grant::seed_system_grants(&pool, "")
        .await
        .expect("seed system grants");
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();

    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/admin/bootstrap-tokens/iam-service/rotate?grace_period_sec=300",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let new_token = body["token"].as_str().expect("token").to_string();
    assert!(
        !body["old_token_expires_at"].is_null(),
        "grace period -> old token has expiry"
    );

    // 旧 token 在宽限期内仍可换 service-jwt
    let resp = token_service::service_jwt_routes(build_state(pool.clone()))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("old-token"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "old token still valid in grace period"
    );

    // 新 token 也可换
    let resp = token_service::service_jwt_routes(build_state(pool))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some(&new_token),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn rotate_404_when_owner_missing() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    let bearer = admin_bearer();
    let resp = app
        .oneshot(req(
            "POST",
            "/admin/bootstrap-tokens/ghost/rotate",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// 禁用后 token 立即不可用；启用后恢复。
#[tokio::test]
async fn set_status_disable_then_enable() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    // 签发联动断言需要 grant（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01 起默认拒绝）；
    // 内置系统 seed 覆盖 iam-service → iam-service。
    token_service::models::service_jwt_grant::seed_system_grants(&pool, "")
        .await
        .expect("seed system grants");
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();

    // disable
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/bootstrap-tokens/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "disabled");

    // 禁用后不能换 service-jwt
    let resp = token_service::service_jwt_routes(build_state(pool.clone()))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-iam"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // enable
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/bootstrap-tokens/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "active"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["status"], "active");

    // 恢复后可以换
    let resp = token_service::service_jwt_routes(build_state(pool))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-iam"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn set_status_rejects_invalid_status() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();
    let resp = app
        .oneshot(req(
            "PUT",
            "/admin/bootstrap-tokens/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "whatever"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_removes_token() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = admin_app(pool.clone());
    let bearer = admin_bearer();

    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            "/admin/bootstrap-tokens/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 删掉后不能换 service-jwt
    let resp = token_service::service_jwt_routes(build_state(pool))
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-iam"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_404_when_owner_missing() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(pool);
    let bearer = admin_bearer();
    let resp = app
        .oneshot(req(
            "GET",
            "/admin/bootstrap-tokens/ghost",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
