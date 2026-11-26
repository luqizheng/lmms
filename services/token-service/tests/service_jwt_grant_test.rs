//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01: service JWT audience grant
//! 管理面 + 签发联动集成测试。
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
    let issuer =
        token_service::TokenIssuer::new(TEST_PRIVATE_PEM, "test-issuer").expect("issuer");
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
    db_execute!(pool, sql, owner, aud, status)
        .unwrap_or_else(|e| panic!("insert grant raw: {e}"));
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
    let summary: Vec<String> = rows
        .iter()
        .map(|(a, r, _)| format!("{a}/{r}"))
        .collect();
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
    assert!(rows.is_empty(), "no grant may exist after injection attempts");
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
    let claims = verify_token(body["access_token"].as_str().unwrap(), &cache, Some("iam-service"))
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

    // grant 判定本身也必须把 DB 错误上抛（而非静默当作"无授权"以外的值）：
    // is_active_grant 的 Err 传播是 handler 500 分支的来源。
    let res = token_service::models::service_jwt_grant::is_active_grant(
        &pool,
        "svc-a",
        "iam-service",
    )
    .await;
    assert!(res.is_err(), "grant lookup must propagate db errors");
}

#[tokio::test]
async fn seed_system_grants_is_idempotent_and_protected() {
    let pool = crate::common::spawn_mysql().await;

    // 首次 seed：7 owner × 7 audience = 49 行 system_managed
    let first = token_service::models::service_jwt_grant::seed_system_grants(&pool, "")
        .await
        .expect("first seed");
    assert_eq!(first, 49);

    // 二次 seed：幂等（表非空 → 不再灌内置行）
    let second = token_service::models::service_jwt_grant::seed_system_grants(&pool, "")
        .await
        .expect("second seed");
    assert_eq!(second, 0);

    // env seed：补插普通行
    let env_seeded = token_service::models::service_jwt_grant::seed_system_grants(
        &pool,
        "svc-x:iam-service,svc-x:file-service",
    )
    .await
    .expect("env seed");
    assert_eq!(env_seeded, 2);

    let row: Option<(bool,)> = db_fetch_optional!(
        &pool,
        (bool,),
        "SELECT system_managed FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ?",
        "svc-x",
        "iam-service"
    )
    .expect("query grant");
    assert_eq!(row.map(|(f,)| f), Some(false));

    // 内置行是保护行
    let row: Option<(bool,)> = db_fetch_optional!(
        &pool,
        (bool,),
        "SELECT system_managed FROM token_service_jwt_grants \
         WHERE owner = ? AND audience = ?",
        "iam-service",
        "iam-service"
    )
    .expect("query grant");
    assert_eq!(row.map(|(f,)| f), Some(true));

    // seed 后签发可用（dev 开箱即用）
    insert_bootstrap_token(&pool, "svc-x", "bt-svc-x").await;
    let issuer = issuer_app(&pool);
    let resp = issuer
        .oneshot(req(
            "POST",
            "/tokens/service-jwt",
            Some("bt-svc-x"),
            Some(json!({"aud": "iam-service", "tid": 1})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn seed_rejects_malformed_grant_env() {
    let pool = crate::common::spawn_mysql().await;
    let res = token_service::models::service_jwt_grant::seed_system_grants(
        &pool,
        "svc-without-colon",
    )
    .await;
    assert!(res.is_err(), "malformed SERVICE_JWT_GRANT_SEED must be rejected");
}
