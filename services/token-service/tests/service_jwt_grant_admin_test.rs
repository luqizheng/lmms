//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: grant **管理面**集成测试。
//!
//! （运行时签发检查的测试在 `service_jwt_grant_test.rs`——上游 WP-02；
//!  本文件覆盖管理 API 的鉴权 / CRUD / 保护 / 审计 / 签发联动。）
//!
//! 覆盖：
//! - 双入口鉴权（service-jwt `aud=token-service` / user-jwt + 权限码）
//! - CRUD + 409/404/400 语义 + system_managed 保护
//! - 并发创建仅一次成功
//! - 签发联动：create → 签发成功；disable → 签发 403；delete → 签发 403
//! - 审计落行（append-only，无凭据字段）
//! - DB 不可用时签发 fail-closed（500，不放行）

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::{db_execute, db_fetch_all, db_fetch_optional};
use digua_common_jwt::verify_token;
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;

use common::{build_state, TEST_PRIVATE_PEM, TEST_PUBLIC_PEM};

// ── scaffolding ──────────────────────────────────────────────────────────────

fn admin_app(pool: &token_service::db::DbPool) -> axum::Router {
    token_service::routes::service_jwt_grant_admin::routes(build_state(pool.clone()))
}

fn web_app(pool: &token_service::db::DbPool) -> axum::Router {
    token_service::routes::service_jwt_grant_web::routes(build_state(pool.clone()))
}

fn issuer_app(pool: &token_service::db::DbPool) -> axum::Router {
    token_service::service_jwt_routes(build_state(pool.clone()))
}

/// 签 aud=token-service 的 service-jwt（管理面准入）。
fn admin_bearer() -> String {
    token_service::mint_service_bearer(TEST_PRIVATE_PEM, "token-service", 3600)
        .expect("mint service bearer")
}

/// 签 aud=<其他服务> 的 service-jwt（应被管理面拒绝）。
fn wrong_aud_bearer() -> String {
    token_service::mint_service_bearer(TEST_PRIVATE_PEM, "file-service", 3600)
        .expect("mint service bearer")
}

/// 签带指定权限码的 user-jwt（web 入口）。
fn user_bearer(perms: &[&str]) -> String {
    let issuer = token_service::TokenIssuer::new(TEST_PRIVATE_PEM, "test-issuer").expect("issuer");
    issuer
        .issue_user_jwt(
            "usr-1",
            "tester",
            json!({"tid": 1, "permissions": perms}),
            3600,
        )
        .expect("issue user jwt")
}

async fn insert_bootstrap_token(pool: &token_service::db::DbPool, owner: &str, token: &str) {
    db_execute!(
        pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, 'active')",
        owner,
        token
    )
    .expect("insert bootstrap token");
}

async fn insert_grant_raw(
    pool: &token_service::db::DbPool,
    owner: &str,
    aud: &str,
    status: &str,
    system_managed: bool,
) {
    // MySQL/PG 通用写法：BOOLEAN 用 TRUE/FALSE 字面量内联（比绑定 + 方言
    // 分支更直观；测试辅助行无注入面）。
    let flag = if system_managed { "TRUE" } else { "FALSE" };
    let sql = format!(
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, ?, {flag})"
    );
    db_execute!(pool, sql, owner, aud, status).unwrap_or_else(|e| panic!("insert grant raw: {e}"));
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

async fn audit_count(pool: &token_service::db::DbPool) -> i64 {
    let row: Option<(i64,)> = db_fetch_optional!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM token_service_jwt_grant_audit"
    )
    .expect("count audit");
    row.map(|(c,)| c).unwrap_or(0)
}

// ── service 入口鉴权 ─────────────────────────────────────────────────────────

#[tokio::test]
async fn service_entry_requires_bearer() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let resp = app
        .oneshot(req("GET", "/admin/service-jwt-grants", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn service_entry_rejects_wrong_audience_jwt() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let resp = app
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants",
            Some(&wrong_aud_bearer()),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // R1-05：认证拒绝发生在鉴权中间件（业务层之前），只写 tracing、
    // 不落业务拒绝审计——审计表必须保持为空。
    assert_eq!(
        audit_count(&pool).await,
        0,
        "auth rejection must not be audited"
    );
}

// ── web 入口鉴权 ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn web_entry_requires_bearer() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(&pool);
    let resp = app
        .oneshot(req("GET", "/v1/admin/service-jwt-grants", None, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn web_entry_rejects_user_without_permission() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(&pool);
    let bearer = user_bearer(&[]); // 无任何权限码
    let resp = app
        .oneshot(req(
            "POST",
            "/v1/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "svc-a", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    // 权限拒绝不产生任何 grant
    let rows: Vec<(String,)> = db_fetch_all!(
        &pool,
        (String,),
        "SELECT owner FROM token_service_jwt_grants"
    )
    .expect("query grants");
    assert!(rows.is_empty());
    // R1-05：变更类（CREATE）权限拒绝必须持久化业务拒绝审计。
    let audit: Vec<(String, String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String, String),
        "SELECT action, result, detail, actor_type FROM token_service_jwt_grant_audit"
    )
    .expect("query audit");
    assert_eq!(
        audit.len(),
        1,
        "create perm denial must be audited: {audit:?}"
    );
    assert_eq!(
        audit[0],
        (
            "create".into(),
            "denied".into(),
            "permission denied".into(),
            "user".into()
        )
    );
}

#[tokio::test]
async fn web_entry_read_perm_cannot_write() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(&pool);
    let bearer = user_bearer(&["TOKEN_SERVICE_JWT_GRANT_READ"]);
    let resp = app
        .oneshot(req(
            "DELETE",
            "/v1/admin/service-jwt-grants/svc-a/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    // R1-05：DELETE 属变更类，权限拒绝必须持久化业务拒绝审计。
    let audit: Vec<(String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String),
        "SELECT action, result, detail FROM token_service_jwt_grant_audit"
    )
    .expect("query audit");
    assert_eq!(
        audit.len(),
        1,
        "write perm denial must be audited: {audit:?}"
    );
    assert_eq!(
        audit[0],
        ("delete".into(), "denied".into(), "permission denied".into())
    );
}

#[tokio::test]
async fn web_list_without_read_permission_writes_no_audit() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(&pool);
    // 无 READ 权限查列表 → 403；查询类拒绝只写 tracing，不落审计（现状契约）。
    let resp = app
        .oneshot(req(
            "GET",
            "/v1/admin/service-jwt-grants",
            Some(&user_bearer(&[])),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        audit_count(&pool).await,
        0,
        "read denial must not be audited"
    );
}

// ── service 入口 CRUD + 业务规则 ─────────────────────────────────────────────

#[tokio::test]
async fn service_entry_full_crud_flow_with_audit() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // create
    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "svc-a", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["owner"], "svc-a");
    assert_eq!(body["audience"], "iam-service");
    assert_eq!(body["status"], "active");
    assert_eq!(body["system_managed"], false);
    assert_eq!(audit_count(&pool).await, 1, "create must be audited");

    // duplicate create → 409（并记 denied 审计）
    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "svc-a", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // get
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants/svc-a/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // get missing → 404
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants/ghost/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // disable
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "disabled");

    // 非法状态 → 400
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "paused"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // re-disable（幂等：已是目标状态）
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // enable
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "active"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // delete
    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/svc-a/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // 重复 delete → 404
    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/svc-a/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 审计行：create(1) + duplicate denied(1) + disable(1) + enable(1) + delete(1)
    // = 5（重复 disable 幂等不写审计；404 get/delete 各写 denied）
    let rows: Vec<(String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String),
        "SELECT action, result, detail FROM token_service_jwt_grant_audit \
         ORDER BY id"
    )
    .expect("query audit rows");
    let summary: Vec<String> = rows.iter().map(|(a, r, _)| format!("{a}/{r}")).collect();
    assert!(
        summary.contains(&"create/success".to_string()),
        "audit: {summary:?}"
    );
    assert!(
        summary.contains(&"create/denied".to_string()),
        "audit: {summary:?}"
    );
    assert!(
        summary.contains(&"disable/success".to_string()),
        "audit: {summary:?}"
    );
    assert!(
        summary.contains(&"enable/success".to_string()),
        "audit: {summary:?}"
    );
    assert!(
        summary.contains(&"delete/success".to_string()),
        "audit: {summary:?}"
    );
}

#[tokio::test]
async fn create_rejects_invalid_owner_and_audience() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    for bad in [
        json!({"owner": "", "audience": "iam"}),
        json!({"owner": "a b", "audience": "iam"}),
        json!({"owner": "svc", "audience": "a;b"}),
        json!({"owner": "svc", "audience": "'--"}),
    ] {
        let resp = app
            .clone()
            .oneshot(req(
                "POST",
                "/admin/service-jwt-grants",
                Some(&bearer),
                Some(bad.clone()),
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "payload should be rejected: {bad}"
        );
    }
    // 注入尝试未产生任何行
    let rows: Vec<(String,)> = db_fetch_all!(
        &pool,
        (String,),
        "SELECT owner FROM token_service_jwt_grants"
    )
    .expect("query grants");
    assert!(
        rows.is_empty(),
        "no grant may exist after injection attempts"
    );
}

#[tokio::test]
async fn list_pagination_boundaries() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?page=0",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?size=0",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?size=101",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 非法 status 过滤 → 400
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?status=weird",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 正常分页返回结构
    for i in 0..3 {
        app.clone()
            .oneshot(req(
                "POST",
                "/admin/service-jwt-grants",
                Some(&bearer),
                Some(json!({"owner": format!("svc-{i}"), "audience": "iam-service"})),
            ))
            .await
            .unwrap();
    }
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?page=1&size=2",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["total"], 3);
    assert_eq!(body["page"], 1);
    assert_eq!(body["size"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 2);

    // 稳定排序：第 2 页拿到剩下的（owner 升序）
    let resp = app
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants?page=2&size=2",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    assert_eq!(body["items"][0]["owner"], "svc-2");
}

#[tokio::test]
async fn system_managed_grants_are_protected() {
    let pool = crate::common::spawn_mysql().await;
    insert_grant_raw(&pool, "sys-owner", "iam-service", "active", true).await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // disable → 409
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/sys-owner/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // enable → 409（system_managed 保护优先于幂等返回；R1-02 必测分支）
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/sys-owner/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "active"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // delete → 409
    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/sys-owner/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // 行仍在，状态未变
    let row: Option<(String,)> = db_fetch_optional!(
        &pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        "sys-owner",
        "iam-service"
    )
    .expect("query grant");
    assert_eq!(row.map(|(s,)| s).as_deref(), Some("active"));

    // R1-05：系统保护拒绝必须持久化业务拒绝审计（disable/enable/delete 各一行）。
    let audit: Vec<(String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String),
        "SELECT action, result, detail FROM token_service_jwt_grant_audit ORDER BY id"
    )
    .expect("query audit");
    assert_eq!(
        audit.len(),
        3,
        "each protected rejection must be audited: {audit:?}"
    );
    for (action, result, detail) in &audit {
        assert_eq!(result, "denied");
        assert_eq!(detail, "system-managed grant is protected");
        assert!(
            ["disable", "enable", "delete"].contains(&action.as_str()),
            "unexpected action: {action}"
        );
    }
}

#[tokio::test]
async fn concurrent_create_succeeds_exactly_once() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    let make = || {
        req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "race-svc", "audience": "iam-service"})),
        )
    };
    let (a, b) = tokio::join!(app.clone().oneshot(make()), app.oneshot(make()));
    let sa = a.unwrap().status();
    let sb = b.unwrap().status();
    let ok = |s: StatusCode| s == StatusCode::OK;
    assert!(
        (ok(sa) && sb == StatusCode::CONFLICT) || (ok(sb) && sa == StatusCode::CONFLICT),
        "exactly one create may succeed: a={sa} b={sb}"
    );

    // 库里只有一行
    let count: i64 = digua_common_db::db_fetch_scalar!(
        &pool,
        i64,
        "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ?",
        "race-svc"
    )
    .expect("count");
    assert_eq!(count, 1);
}

// ── web 入口 CRUD（有权限）───────────────────────────────────────────────────

#[tokio::test]
async fn web_entry_full_crud_with_permission() {
    let pool = crate::common::spawn_mysql().await;
    let app = web_app(&pool);
    let read = user_bearer(&["TOKEN_SERVICE_JWT_GRANT_READ"]);
    let write = user_bearer(&[
        "TOKEN_SERVICE_JWT_GRANT_READ",
        "TOKEN_SERVICE_JWT_GRANT_CREATE",
        "TOKEN_SERVICE_JWT_GRANT_UPDATE",
        "TOKEN_SERVICE_JWT_GRANT_DELETE",
    ]);

    let resp = app
        .clone()
        .oneshot(req(
            "POST",
            "/v1/admin/service-jwt-grants",
            Some(&write),
            Some(json!({"owner": "web-svc", "audience": "file-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // read 权限可见（含 service 入口建的行——两入口业务结果一致）
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            "/v1/admin/service-jwt-grants/web-svc/file-service",
            Some(&read),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(body["status"], "active");

    // disable
    let resp = app
        .clone()
        .oneshot(req(
            "PUT",
            "/v1/admin/service-jwt-grants/web-svc/file-service/status",
            Some(&write),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // delete
    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            "/v1/admin/service-jwt-grants/web-svc/file-service",
            Some(&write),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// ── 签发联动 ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn grant_lifecycle_controls_issuance() {
    let pool = crate::common::spawn_mysql().await;
    insert_bootstrap_token(&pool, "svc-a", "bt-svc-a").await;

    let admin = admin_app(&pool);
    let issuer = issuer_app(&pool);
    let bearer = admin_bearer();

    // 无 grant → 签发 403（默认拒绝）
    let resp = issuer
        .clone()
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 创建 grant → 签发成功
    let resp = admin
        .clone()
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "svc-a", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = issuer
        .clone()
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(
        body["access_token"].as_str().unwrap(),
        &cache,
        Some("iam-service"),
    )
    .await
    .unwrap();
    assert_eq!(claims["sub"], "svc-a");

    // 未授权的 audience 仍拒绝（owner 只能签发已授权 audience）
    let resp = issuer
        .clone()
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "file-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // disable → 签发立即 403
    let resp = admin
        .clone()
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(&bearer),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = issuer
        .clone()
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // delete → 签发仍 403
    let resp = admin
        .clone()
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/svc-a/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = issuer
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn two_entries_share_business_state() {
    let pool = crate::common::spawn_mysql().await;
    let admin = admin_app(&pool);
    let web = web_app(&pool);
    let svc_bearer = admin_bearer();
    let web_bearer = user_bearer(&["TOKEN_SERVICE_JWT_GRANT_READ"]);

    // service 入口创建
    let resp = admin
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&svc_bearer),
            Some(json!({"owner": "x-svc", "audience": "audit-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // web 入口立即可见（同一份 DB 状态）
    let resp = web
        .clone()
        .oneshot(req(
            "GET",
            "/v1/admin/service-jwt-grants/x-svc/audit-service",
            Some(&web_bearer),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // web 入口重复创建 → 409（业务规则一致）
    let resp = web
        .oneshot(req(
            "POST",
            "/v1/admin/service-jwt-grants",
            Some(&user_bearer(&["TOKEN_SERVICE_JWT_GRANT_CREATE"])),
            Some(json!({"owner": "x-svc", "audience": "audit-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ── fail-closed ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn issuance_fails_closed_when_db_unavailable() {
    // 连接到必然失败的地址（lazy pool）：签发路径上的 DB 查询都会报错。
    let bad = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy("mysql://127.0.0.1:1/none")
        .expect("lazy pool");
    let pool = token_service::db::DbPool::from(bad);

    let issuer = issuer_app(&pool);
    let resp = issuer
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-a"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    // fail-closed：绝不因 DB 异常放行（bootstrap 查询失败 → 500）
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    // grant 判定本身也必须把 DB 错误上抛（而非静默当作"无授权"）：
    // is_allowed 的 Err 传播是 handler 500 分支的来源。
    let res =
        token_service::models::service_jwt_grant::is_allowed(&pool, "svc-a", "iam-service").await;
    assert!(res.is_err(), "grant lookup must propagate db errors");
}

#[tokio::test]
async fn upstream_system_seed_is_idempotent_and_its_rows_are_protected() {
    let pool = crate::common::spawn_mysql().await;

    // 上游 WP-02 的 code-owned seed：精准 SYSTEM_GRANTS（2 条），幂等
    let first = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("first seed");
    assert_eq!(first, 2);
    let second = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("second seed");
    assert_eq!(second, 0, "seed must be idempotent");

    // 系统行是保护行
    let row: Option<(bool,)> = db_fetch_optional!(
        &pool,
        (bool,),
        "SELECT system_managed FROM token_service_jwt_grants          WHERE owner = ? AND audience = ?",
        "iam-service",
        "token-service"
    )
    .expect("query grant");
    assert_eq!(row.map(|(f,)| f), Some(true));

    // 管理面不能改删系统行（409）
    let app = admin_app(&pool);
    let bearer = admin_bearer();
    for (method, uri, body) in [
        (
            "PUT",
            "/admin/service-jwt-grants/iam-service/token-service/status",
            Some(json!({"status": "disabled"})),
        ),
        (
            "DELETE",
            "/admin/service-jwt-grants/iam-service/token-service",
            None,
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(req(method, uri, Some(&bearer), body))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::CONFLICT,
            "system grant must be protected: {method} {uri}"
        );
    }
    // 行仍是 active
    let st: Option<(String,)> = db_fetch_optional!(
        &pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        "iam-service",
        "token-service"
    )
    .expect("query status");
    assert_eq!(st.map(|(x,)| x).as_deref(), Some("active"));
}

// ── R1-01：X-Request-Id 信任边界（审计最终态验证）────────────────────────────

/// 审计 request_id 必须始终是严格 UUID（内容与长度由 UUID 语法约束）。
fn assert_uuid(s: &str) {
    uuid::Uuid::parse_str(s)
        .unwrap_or_else(|e| panic!("audit request_id must be a strict uuid, got {s:?}: {e}"));
}

/// 按 owner 取该 owner 的全部审计 request_id。
async fn audit_request_ids(pool: &token_service::db::DbPool, owner: &str) -> Vec<String> {
    let rows: Vec<(String,)> = db_fetch_all!(
        pool,
        (String,),
        "SELECT request_id FROM token_service_jwt_grant_audit WHERE owner = ?",
        owner
    )
    .expect("query audit request ids");
    rows.into_iter().map(|(r,)| r).collect()
}

#[tokio::test]
async fn valid_request_id_is_persisted_for_correlation() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // 合法 UUID 被原样接受（规范化为 hyphenated 小写），保持关联能力
    let rid = "0b4f9c5a-1f2e-3d4c-5b6a-7c8d9e0f1a2b";
    let mut b = req(
        "POST",
        "/admin/service-jwt-grants",
        Some(&bearer),
        Some(json!({"owner": "rid-ok", "audience": "iam-service"})),
    );
    b.headers_mut()
        .insert("X-Request-Id", rid.parse().expect("valid header"));
    let resp = app.oneshot(b).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let ids = audit_request_ids(&pool, "rid-ok").await;
    assert_eq!(ids.len(), 1);
    assert_uuid(&ids[0]);
    assert_eq!(ids[0], rid, "valid uuid must be preserved verbatim");
}

#[tokio::test]
async fn hostile_request_ids_never_reach_audit() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // 攻击矩阵：注入串 / Authorization 材料 / bootstrap secret / 空串 / 超长。
    // 任何值都不得把自由文本带进审计表；请求本身不得因非法 Request-ID 失败。
    let oversized = "x".repeat(65);
    let hostile = [
        "rid-inject",
        "abc'; DROP TABLE token_service_jwt_grant_audit--",
        "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ",
        "bootstrap-secret-ak-7f3c9e1d2b4a6c8e",
        "",
        oversized.as_str(),
    ];
    for (i, raw) in hostile.iter().enumerate() {
        let owner = format!("rid-bad-{i}");
        let mut b = req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": owner, "audience": "iam-service"})),
        );
        b.headers_mut()
            .insert("X-Request-Id", raw.parse().expect("valid header value"));
        let resp = app.clone().oneshot(b).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "hostile request-id must not fail the request: {raw:?}"
        );

        let ids = audit_request_ids(&pool, &owner).await;
        assert_eq!(ids.len(), 1, "create must still be audited: {raw:?}");
        assert_uuid(&ids[0]);
        assert_ne!(
            ids[0], *raw,
            "client-supplied value must not be stored verbatim: {raw:?}"
        );
        assert!(
            ids[0].len() <= 36,
            "stored request_id must be uuid-sized: {:?}",
            ids[0]
        );
    }
}

#[tokio::test]
async fn every_audit_request_id_is_a_uuid() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // 走一遍会产生多类审计（success/denied/404-denied）的流程，
    // 断言全部行的 request_id 都是严格 UUID。
    app.clone()
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "rid-all", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({"owner": "rid-all", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    app.clone()
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/ghost/iam-service",
            Some(&bearer),
            None,
        ))
        .await
        .unwrap();

    let rows: Vec<(String, String)> = db_fetch_all!(
        &pool,
        (String, String),
        "SELECT request_id, result FROM token_service_jwt_grant_audit ORDER BY id"
    )
    .expect("query audit");
    assert!(
        rows.len() >= 3,
        "expected success/denied/not-found rows: {rows:?}"
    );
    for (rid, _) in &rows {
        assert_uuid(rid);
    }
}

// ── R1-02：system_managed 原子保护（TOCTOU 竞态窗口）─────────────────────────

#[tokio::test]
async fn status_update_race_cannot_touch_system_managed_row() {
    let pool = crate::common::spawn_mysql().await;

    // 确定性构造 TOCTOU 窗口（无 sleep）：
    // 1) ops 前置查询读到的行（此时未受保护）
    insert_grant_raw(&pool, "race-st", "iam-service", "active", false).await;
    let before = token_service::models::service_jwt_grant::find_by_owner_and_audience(
        &pool,
        "race-st",
        "iam-service",
    )
    .await
    .expect("before query")
    .expect("row exists");
    assert!(!before.system_managed, "前置查询应读到普通行");

    // 2) 窗口内另一请求把行标记为 system_managed
    db_execute!(
        &pool,
        "UPDATE token_service_jwt_grants SET system_managed = TRUE \
         WHERE owner = ? AND audience = ?",
        "race-st",
        "iam-service"
    )
    .expect("flip system_managed");

    // 3) 原 UPDATE 继续执行——保护条件在 SQL 内原子生效，写不命中
    let affected = token_service::models::service_jwt_grant::set_grant_status(
        &pool,
        "race-st",
        "iam-service",
        "disabled",
    )
    .await
    .expect("conditional update");
    assert_eq!(affected, 0, "system-flipped row must not be updated");

    let row: Option<(String, bool)> = db_fetch_optional!(
        &pool,
        (String, bool),
        "SELECT status, system_managed FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ?",
        "race-st",
        "iam-service"
    )
    .expect("query row");
    assert_eq!(row.map(|(s, f)| (s, f)), Some(("active".into(), true)));

    // 4) HTTP 层：affected=0 → 重查归类为系统保护 → 409 + denied 审计
    let app = admin_app(&pool);
    let resp = app
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/race-st/iam-service/status",
            Some(&admin_bearer()),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let audit: Vec<(String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String),
        "SELECT action, result, detail FROM token_service_jwt_grant_audit"
    )
    .expect("query audit");
    assert!(
        audit.contains(&(
            "disable".into(),
            "denied".into(),
            "system-managed grant is protected".into()
        )),
        "race-window protection denial must be audited: {audit:?}"
    );
}

#[tokio::test]
async fn delete_race_cannot_touch_system_managed_row() {
    let pool = crate::common::spawn_mysql().await;

    // 同 status_update_race_cannot_touch_system_managed_row 的确定性窗口构造
    insert_grant_raw(&pool, "race-del", "iam-service", "active", false).await;
    let before = token_service::models::service_jwt_grant::find_by_owner_and_audience(
        &pool,
        "race-del",
        "iam-service",
    )
    .await
    .expect("before query")
    .expect("row exists");
    assert!(!before.system_managed);

    db_execute!(
        &pool,
        "UPDATE token_service_jwt_grants SET system_managed = TRUE \
         WHERE owner = ? AND audience = ?",
        "race-del",
        "iam-service"
    )
    .expect("flip system_managed");

    // 原 DELETE 继续执行——条件 DELETE 不命中，行保留
    let affected =
        token_service::models::service_jwt_grant::delete_grant(&pool, "race-del", "iam-service")
            .await
            .expect("conditional delete");
    assert_eq!(affected, 0, "system-flipped row must not be deleted");
    let row: Option<(String,)> = db_fetch_optional!(
        &pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        "race-del",
        "iam-service"
    )
    .expect("query row");
    assert!(row.is_some(), "row must survive the race");

    // HTTP 层：affected=0 → 重查归类为系统保护 → 409 + denied 审计
    let app = admin_app(&pool);
    let resp = app
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/race-del/iam-service",
            Some(&admin_bearer()),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let audit: Vec<(String, String, String)> = db_fetch_all!(
        &pool,
        (String, String, String),
        "SELECT action, result, detail FROM token_service_jwt_grant_audit"
    )
    .expect("query audit");
    assert!(
        audit.contains(&(
            "delete".into(),
            "denied".into(),
            "system-managed grant is protected".into()
        )),
        "race-window protection denial must be audited: {audit:?}"
    );
}

#[tokio::test]
async fn create_payload_cannot_inject_system_managed() {
    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // 请求体携带 system_managed=true（未知字段）必须被忽略——
    // 客户端没有任何路径能创建受保护授权。
    let resp = app
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&bearer),
            Some(json!({
                "owner": "inject-svc",
                "audience": "iam-service",
                "system_managed": true
            })),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp).await;
    assert_eq!(
        body["system_managed"], false,
        "dto must report unprotected row"
    );

    let row: Option<(bool,)> = db_fetch_optional!(
        &pool,
        (bool,),
        "SELECT system_managed FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ?",
        "inject-svc",
        "iam-service"
    )
    .expect("query row");
    assert_eq!(row.map(|(f,)| f), Some(false), "db row must be unprotected");
}

// ── R1-04：并发创建——唯一键竞争的确定性覆盖 ─────────────────────────────────

#[tokio::test]
async fn concurrent_insert_race_resolves_via_unique_key() {
    let pool = crate::common::spawn_mysql().await;

    // 确定性同步点：两个"请求"的前置查询都完成（都读到不存在）之后，
    // 才允许执行 INSERT——时序由测试顺序固定，不依赖 sleep 或调度运气。
    for label in ["select a", "select b"] {
        assert!(
            token_service::models::service_jwt_grant::find_by_owner_and_audience(
                &pool,
                "uniq-race",
                "iam-service"
            )
            .await
            .expect(label)
            .is_none(),
            "{label} must see no row"
        );
    }

    // 两个 INSERT 先后落库：恰一次成功，一次撞唯一键（V005 PK 兜底）
    let first = digua_common_db::db_execute!(
        &pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        "uniq-race",
        "iam-service"
    );
    let second = digua_common_db::db_execute!(
        &pool,
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, 'active', FALSE)",
        "uniq-race",
        "iam-service"
    );
    assert!(
        first.is_ok(),
        "first insert must win: {:?}",
        first.as_ref().err()
    );
    let err = second.expect_err("second insert must hit the unique key");
    assert!(
        token_service::db::is_duplicate_key(&err),
        "must be classified as duplicate-key: {err}"
    );

    // 恰好一行
    let count: i64 = digua_common_db::db_fetch_scalar!(
        &pool,
        i64,
        "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ?",
        "uniq-race"
    )
    .expect("count");
    assert_eq!(count, 1);

    // ops 层语义：撞键的 INSERT 被 create_grant 归类为 Duplicate（映射 409），
    // 不泄露 DB 内部错误
    let outcome =
        token_service::models::service_jwt_grant::create_grant(&pool, "uniq-race", "iam-service")
            .await
            .expect("create_grant must classify duplicate, not error");
    assert_eq!(
        outcome,
        token_service::models::service_jwt_grant::CreateOutcome::Duplicate
    );
}

/// HTTP 端到端的确定性并发竞争（R1-04）。
///
/// 可控同步机制：测试连接持 `LOCK TABLES ... READ`——前置 SELECT 不受阻、
/// INSERT 全部排队。事件驱动等待（轮询 `information_schema.processlist` 的
/// 锁等待状态，非 sleep 猜测时序）确认**两个请求都已越过前置查询、双双
/// 卡在 INSERT** 后放行，让两个 INSERT 精确同时竞争唯一键。
/// 测试专用 DB 操作，不触碰生产代码。
#[tokio::test]
async fn concurrent_http_creates_race_insert_after_both_preflights() {
    use std::time::{Duration, Instant};

    let pool = crate::common::spawn_mysql().await;
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    let mysql = pool.as_mysql().expect("mysql pool");

    // 屏障连接：持表读锁（SELECT 不受阻、INSERT 排队）
    let mut locker = mysql.acquire().await.expect("locker conn");
    let db_name: (Option<String>,) = sqlx::query_as("SELECT DATABASE()")
        .fetch_one(&mut *locker)
        .await
        .expect("db name");
    let db_name = db_name.0.expect("test db selected");
    // LOCK TABLES 不支持 prepared statement 协议（ER 1295），必须走 raw_sql
    sqlx::raw_sql("LOCK TABLES token_service_jwt_grants READ")
        .execute(&mut *locker)
        .await
        .expect("lock tables");

    // 独立轮询连接（持锁会话只能访问已锁表，不能查 processlist）
    let poller_url = {
        // 复用 locker 的连接参数：从 pool 内部拿不到 URL，直接用共享环境变量重建
        let host = std::env::var("TEST_MYSQL_HOST").expect("TEST_MYSQL_HOST");
        let port = std::env::var("TEST_MYSQL_PORT").expect("TEST_MYSQL_PORT");
        format!("mysql://root@{host}:{port}/{db_name}")
    };
    let poller = sqlx::MySqlPool::connect(&poller_url)
        .await
        .expect("poller conn");

    // 两个独立创建请求（相同 owner/audience）。**后台 spawn**：请求会卡在
    // INSERT 锁等待上，主线程必须腾出来做轮询与放行（join 前置会死锁）。
    let make_request = |bearer: &String| {
        req(
            "POST",
            "/admin/service-jwt-grants",
            Some(bearer),
            Some(json!({"owner": "barrier-race", "audience": "iam-service"})),
        )
    };
    let (bearer1, bearer2) = (bearer.clone(), bearer.clone());
    let app1 = app.clone();
    let h1 = tokio::spawn(async move { app1.oneshot(make_request(&bearer1)).await });
    let h2 = tokio::spawn(async move { app.oneshot(make_request(&bearer2)).await });

    // 事件驱动等待：两个 INSERT 都进入 MDL 等待（= 双双完成前置 SELECT）。
    // MySQL 8.0 下 LOCK TABLES READ 阻塞写入时 STATE 是
    // "Waiting for table metadata lock"（InnoDB 用 metadata lock 实现）。
    let mut both_waiting = false;
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM information_schema.processlist \
             WHERE DB = ? AND STATE LIKE '%metadata lock%'",
        )
        .bind(&db_name)
        .fetch_one(&poller)
        .await
        .expect("poll processlist");
        if n >= 2 {
            both_waiting = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // 放行两个 INSERT，精确同时竞争唯一键（同 LOCK TABLES，走 raw_sql）
    sqlx::raw_sql("UNLOCK TABLES")
        .execute(&mut *locker)
        .await
        .expect("unlock tables");
    assert!(
        both_waiting,
        "both requests must have passed the preflight SELECT before INSERT"
    );

    let r1 = tokio::time::timeout(Duration::from_secs(60), async {
        h1.await.unwrap().unwrap()
    })
    .await
    .expect("request 1 must finish after unlock");
    let sb = tokio::time::timeout(Duration::from_secs(60), async {
        h2.await.unwrap().unwrap()
    })
    .await
    .expect("request 2 must finish after unlock")
    .status();
    let sa = r1.status();
    let ok = |s: StatusCode| s == StatusCode::OK;
    assert!(
        (ok(sa) && sb == StatusCode::CONFLICT) || (ok(sb) && sa == StatusCode::CONFLICT),
        "exactly one create may succeed: a={sa} b={sb}"
    );

    // 恰好一行；且没有任何 500（DB 内部错误不泄露）
    let count: i64 = digua_common_db::db_fetch_scalar!(
        &pool,
        i64,
        "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ?",
        "barrier-race"
    )
    .expect("count");
    assert_eq!(count, 1, "unique key must keep exactly one row");

    // 审计恰一 success 一 denied（撞键方走 Duplicate → 409 + best-effort 审计）
    let audit: Vec<(String, String)> = db_fetch_all!(
        &pool,
        (String, String),
        "SELECT action, result FROM token_service_jwt_grant_audit WHERE owner = ?",
        "barrier-race"
    )
    .expect("query audit");
    let mut successes = 0;
    let mut denials = 0;
    for (action, result) in &audit {
        assert_eq!(action, "create");
        match result.as_str() {
            "success" => successes += 1,
            "denied" => denials += 1,
            other => panic!("unexpected audit result: {other}"),
        }
    }
    assert_eq!(successes, 1, "exactly one audited success: {audit:?}");
    assert_eq!(
        denials, 1,
        "exactly one audited duplicate denial: {audit:?}"
    );
}

// ── 管理面 fail-closed（R1-05：DB 故障时审计无法持久化 → 500，不放行）────────

#[tokio::test]
async fn admin_ops_fail_closed_when_db_unavailable() {
    let bad = sqlx::mysql::MySqlPoolOptions::new()
        .connect_lazy("mysql://127.0.0.1:1/none")
        .expect("lazy pool");
    let pool = token_service::db::DbPool::from(bad);
    let app = admin_app(&pool);
    let bearer = admin_bearer();

    // 全部变更/查询操作：DB 不可用 → 500（审计与业务同库，不存在
    // "变更成功但审计丢失"的中间态——要么都成，要么 500 fail-closed）。
    for (method, uri, body) in [
        (
            "POST",
            "/admin/service-jwt-grants",
            Some(json!({"owner": "svc-a", "audience": "iam-service"})),
        ),
        (
            "PUT",
            "/admin/service-jwt-grants/svc-a/iam-service/status",
            Some(json!({"status": "disabled"})),
        ),
        (
            "DELETE",
            "/admin/service-jwt-grants/svc-a/iam-service",
            None,
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(req(method, uri, Some(&bearer), body))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{method} {uri} must fail closed"
        );
    }
}
