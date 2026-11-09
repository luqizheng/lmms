//! Phase 3 (web): `/v1/admin/bootstrap-tokens*` 用户态端点集成测试。
//!
//! 鉴权：user-jwt（`Principal` extractor 验签 + 解出 permissions）+ 权限码。
//! 用 `TokenIssuer::issue_user_jwt` 签带 `permissions` claim 的 token。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::db_execute;
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;

use common::{build_state, TEST_PRIVATE_PEM};

/// 签一个带指定权限码的 user-jwt。
fn user_bearer(perms: &[&str]) -> String {
    let issuer = token_service::TokenIssuer::new(TEST_PRIVATE_PEM, "test-issuer").expect("issuer");
    issuer
        .issue_user_jwt(
            "usr-1",
            "tester",
            json!({
                "tid": 1,
                "permissions": perms,
            }),
            3600,
        )
        .expect("issue user jwt")
}

fn web_app(pool: token_service::db::DbPool) -> axum::Router {
    token_service::routes::bootstrap_token_web::routes(build_state(pool))
}

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

const PERM_READ: &str = "TOKEN_BOOTSTRAP_READ";
const PERM_CREATE: &str = "TOKEN_BOOTSTRAP_CREATE";
const PERM_UPDATE: &str = "TOKEN_BOOTSTRAP_UPDATE";
const PERM_DELETE: &str = "TOKEN_BOOTSTRAP_DELETE";

#[tokio::test]
async fn web_requires_bearer_token() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(pool);
    let resp = app
        .oneshot(req("GET", "/v1/admin/bootstrap-tokens", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn web_rejects_user_without_permission() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(pool);
    // 有合法 user-jwt 但无任何权限码
    let bearer = user_bearer(&[]);
    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_list_with_read_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    insert(&pool, "audit-service", "bt-audit", "disabled").await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_READ]);

    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["total"], 2);
    // 不泄露明文
    let item = &body["items"][0];
    assert!(item.get("token").is_none());
    assert!(item.get("token_hash").is_none());
}

#[tokio::test]
async fn web_list_filters_by_keyword_and_status() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    insert(&pool, "audit-service", "bt-audit", "disabled").await;
    let app = web_app(pool.clone());
    let bearer = user_bearer(&[PERM_READ]);

    // keyword
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens?keyword=iam",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["owner"], "iam-service");

    // status
    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens?status=disabled",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["owner"], "audit-service");
}

#[tokio::test]
async fn web_create_requires_create_permission() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(pool);
    // 只有 READ 权限 → 403
    let resp = app
        .oneshot(req(
            "POST",
            "/v1/admin/bootstrap-tokens",
            Some(&user_bearer(&[PERM_READ])),
            Some(json!({"owner": "new-svc"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_create_with_create_permission() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_CREATE]);
    let resp = app
        .oneshot(req(
            "POST",
            "/v1/admin/bootstrap-tokens",
            Some(&bearer),
            Some(json!({"owner": "new-svc"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["owner"], "new-svc");
    assert_eq!(body["token"].as_str().unwrap().len(), 40);
}

#[tokio::test]
async fn web_rotate_requires_update_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "old-token", "active").await;
    let app = web_app(pool);
    // 只有 READ 权限 → 403
    let resp = app
        .oneshot(req(
            "POST",
            "/v1/admin/bootstrap-tokens/iam-service/rotate",
            Some(&user_bearer(&[PERM_READ])),
            Some(json!({"grace_period_sec": 0})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_rotate_with_update_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "old-token", "active").await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_UPDATE]);
    let resp = app
        .oneshot(req(
            "POST",
            "/v1/admin/bootstrap-tokens/iam-service/rotate",
            Some(&bearer),
            Some(json!({"grace_period_sec": 60})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_ne!(body["token"].as_str().unwrap(), "old-token");
    assert!(!body["old_token_expires_at"].is_null());
}

#[tokio::test]
async fn web_set_status_requires_update_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    let resp = app
        .oneshot(req(
            "PUT",
            "/v1/admin/bootstrap-tokens/iam-service/status",
            Some(&user_bearer(&[PERM_READ])),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_set_status_with_update_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_UPDATE]);
    let resp = app
        .oneshot(req(
            "PUT",
            "/v1/admin/bootstrap-tokens/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json_body(resp).await["status"], "disabled");
}

#[tokio::test]
async fn web_delete_requires_delete_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    // 只有 UPDATE 权限 → 403（DELETE 是独立权限码）
    let resp = app
        .oneshot(req(
            "DELETE",
            "/v1/admin/bootstrap-tokens/iam-service",
            Some(&user_bearer(&[PERM_UPDATE])),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_delete_with_delete_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_DELETE]);
    let resp = app
        .oneshot(req(
            "DELETE",
            "/v1/admin/bootstrap-tokens/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn web_get_requires_read_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens/iam-service",
            Some(&user_bearer(&[PERM_CREATE])),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn web_get_with_read_permission() {
    let pool = crate::common::spawn_mysql().await;
    insert(&pool, "iam-service", "bt-iam", "active").await;
    let app = web_app(pool);
    let bearer = user_bearer(&[PERM_READ]);
    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/bootstrap-tokens/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["owner"], "iam-service");
    assert!(body.get("token").is_none());
}
