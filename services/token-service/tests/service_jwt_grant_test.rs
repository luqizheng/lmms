//! WP-LMMS-SERVICE-JWT-AUDIENCE-02: Service JWT owner→audience grant 集成测试。
//!
//! 覆盖实施任务书 §4 测试矩阵的签发/负向授权/启动引导/rotation 场景。
//! 需要 MySQL（testcontainers 或共享库，见 common::spawn_mysql）。
//!
//! 同步纪律：现有测试中"任意 owner 请求任意 audience"的旧语义已随
//! fail-closed 实施全部修正（service_jwt_test.rs）。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::{db_execute, db_fetch_all, db_fetch_scalar};
use digua_common_jwt::verify_token;
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;

use common::{build_state, TEST_PUBLIC_PEM};

async fn insert_token(pool: &token_service::db::DbPool, owner: &str, token: &str, status: &str) {
    db_execute!(
        pool,
        "INSERT INTO token_bootstrap_tokens (owner, token_hash, status) VALUES (?, ?, ?)",
        owner,
        token,
        status
    )
    .expect("insert bootstrap token");
}

/// 插入一条显式 grant（非系统默认，模拟人工/迁移授予）。
#[allow(dead_code)] // 供后续按用例扩展显式授权场景；当前负向用例走系统 grant
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

async fn post_service_jwt(app: axum::Router, bearer: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/tokens/service-jwt")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
    let parsed: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, parsed)
}

fn app(pool: token_service::db::DbPool) -> axum::Router {
    token_service::service_jwt_routes(build_state(pool))
}

// ── 正向授权 ────────────────────────────────────────────────────────────────

/// #9 P0：iam-service 可以获得 aud=token-service（登录链必需）。
#[tokio::test]
async fn iam_service_can_obtain_token_service_jwt() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, body) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(
        body["access_token"].as_str().unwrap(),
        &cache,
        Some("token-service"),
    )
    .await
    .unwrap();
    assert_eq!(claims["sub"], "iam-service");
    assert_eq!(claims["aud"], "token-service");
    assert_eq!(claims["purpose"], "service");
}

/// #11 P1：notification-service 可以获得 aud=iam-service。
#[tokio::test]
async fn notification_service_can_obtain_iam_service_jwt() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "notification-service", "bt-ntf", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, body) =
        post_service_jwt(app(pool), "bt-ntf", json!({"aud": "iam-service", "tid": 1})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["access_token"].as_str().unwrap().is_empty());
}

/// #22：claims 契约不变——tid/ct 按请求原样签入，grant 不改变 claims 语义。
#[tokio::test]
async fn claims_tid_and_ct_unchanged_by_grant_gate() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, body) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "token-service", "tid": 42, "ct": true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(
        body["access_token"].as_str().unwrap(),
        &cache,
        Some("token-service"),
    )
    .await
    .unwrap();
    assert_eq!(claims["tid"], 42);
    assert_eq!(claims["ct"], true);
    assert_eq!(claims["roles"][0], "service");
}

// ── 负向授权（fail-closed）──────────────────────────────────────────────────

/// #10：iam-service 不能获得未授权 audience（file-service → 403）。
#[tokio::test]
async fn iam_service_denied_unrelated_audience() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, body) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "file-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["access_token"].is_null(), "被拒绝时绝不能返回 token");
}

/// #12：notification-service 不能获得未授权 audience。
#[tokio::test]
async fn notification_service_denied_unrelated_audience() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "notification-service", "bt-ntf", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, _) = post_service_jwt(
        app(pool),
        "bt-ntf",
        json!({"aud": "audit-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// #13：token-service 拥有 Bootstrap Token 身份，但没有隐式出站 grant。
#[tokio::test]
async fn token_service_owner_has_no_implicit_grant() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "token-service", "bt-token", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let (status, _) = post_service_jwt(
        app(pool),
        "bt-token",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// #14：未知 owner（401）与已知 owner + 未知 audience（403）。
#[tokio::test]
async fn unknown_owner_and_unknown_audience_denied() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    // 未知 owner → 401（认证失败，不进入授权阶段）
    let (status, _) = post_service_jwt(
        app(pool.clone()),
        "bt-nobody",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // 未知 audience（语法合法但无 grant）→ 403
    let (status, _) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "some-unregistered-svc", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// disabled grant 拒绝签发（#3：审计矩阵）。
#[tokio::test]
async fn disabled_grant_is_denied() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    db_execute!(
        &pool,
        "UPDATE token_service_jwt_grants SET status = 'disabled' \
         WHERE owner = 'iam-service' AND audience = 'token-service'"
    )
    .expect("disable grant");

    let (status, _) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// #15：grant 存储故障 fail-closed——表不可用时返回 5xx 且绝不签发。
#[tokio::test]
async fn grant_store_db_failure_fails_closed() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    // 模拟存储故障：drop 掉 grant 表
    db_execute!(&pool, "DROP TABLE token_service_jwt_grants").expect("drop grants table");

    let (status, body) = post_service_jwt(
        app(pool),
        "bt-iam",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert!(
        status.is_server_error(),
        "DB 故障必须 fail-closed，得到 {status}"
    );
    assert!(body["access_token"].is_null(), "故障时绝不签发 token");
}

/// aud 语法校验：非法字符 / 超长 → 400。
#[tokio::test]
async fn malformed_audience_rejected_before_grant_lookup() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    let state_pool = pool.clone();
    let (status, _) = post_service_jwt(
        app(state_pool),
        "bt-iam",
        json!({"aud": "bad aud!", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let long_aud = "a".repeat(65);
    let (status, _) =
        post_service_jwt(app(pool), "bt-iam", json!({"aud": long_aud, "tid": 1})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── 启动引导（seed / 幂等 / 并发 / 修复 / 重启）─────────────────────────────

/// #4/#7：启动 seed 幂等；重复启动不重复插行。
#[tokio::test]
async fn ensure_system_grants_is_idempotent() {
    let pool = common::spawn_mysql().await;
    let first = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("first ensure");
    assert_eq!(first, 2, "应插入两条内置 grant");

    let second = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("second ensure");
    assert_eq!(second, 0, "重复启动必须幂等");

    let count: i64 = db_fetch_scalar!(&pool, i64, "SELECT COUNT(*) FROM token_service_jwt_grants")
        .expect("count grants");
    assert_eq!(count, 2);
}

/// #3/#8：存量库缺 grant 行时启动修复（补缺失行，不动已有行）。
#[tokio::test]
async fn ensure_system_grants_repairs_missing_row() {
    let pool = common::spawn_mysql().await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("initial ensure");
    db_execute!(
        &pool,
        "DELETE FROM token_service_jwt_grants WHERE owner = 'notification-service'"
    )
    .expect("delete one grant");

    let repaired = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("repair ensure");
    assert_eq!(repaired, 1, "缺失的系统行应被补插");

    let row = token_service::models::service_jwt_grant::find_by_owner_and_audience(
        &pool,
        "notification-service",
        "iam-service",
    )
    .await
    .expect("query")
    .expect("row restored");
    assert_eq!(row.status, "active");
    assert!(row.system_managed);
}

/// #7：并发多副本同时 seed → 无重复行、无启动失败。
#[tokio::test]
async fn concurrent_ensure_grants_do_not_duplicate() {
    let pool = common::spawn_mysql().await;
    let mut handles = Vec::new();
    for _ in 0..4 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            token_service::models::service_jwt_grant::ensure_system_grants(&p).await
        }));
    }
    for h in handles {
        h.await.expect("join").expect("ensure ok");
    }
    let count: i64 = db_fetch_scalar!(&pool, i64, "SELECT COUNT(*) FROM token_service_jwt_grants")
        .expect("count");
    assert_eq!(count, 2, "并发 seed 不得产生重复行");
}

/// #18/#19：重启后授权保持；seed 不复活被显式撤销的自定义 grant。
#[tokio::test]
async fn grants_survive_restart_and_seed_does_not_reactivate_disabled() {
    let pool = common::spawn_mysql().await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("initial ensure");
    // 运维显式禁用系统 grant
    db_execute!(
        &pool,
        "UPDATE token_service_jwt_grants SET status = 'disabled' \
         WHERE owner = 'iam-service'"
    )
    .expect("disable");
    // 重启（再次 seed）：修复语义 = 只补缺失行，不复活被撤销的授权
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("restart ensure");
    let status: (String,) = digua_common_db::db_fetch_optional!(
        &pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants WHERE owner = 'iam-service'"
    )
    .expect("query")
    .expect("row exists");
    assert_eq!(status.0, "disabled", "启动 seed 不得复活被显式禁用的 grant");
}

/// #5：生产风格 env 注入密钥时，系统 grant 照常 seed（授权引导与密钥来源解耦）。
#[tokio::test]
async fn system_grants_seeded_regardless_of_env_config() {
    let pool = common::spawn_mysql().await;
    let inserted = token_service::models::bootstrap_token::seed_defaults(
        &pool,
        "prod-secret-iam,prod-secret-ntf",
        "iam-service,notification-service",
    )
    .await
    .expect("seed from env");
    assert_eq!(inserted, 2, "生产 env 只插 env 声明的 owner");

    let seeded = token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    assert_eq!(seeded, 2, "env 注入不得抑制系统 grant seed");

    // dev 默认 token 未被插入（生产安全：不静默回退 dev secret）
    let dev_count: i64 = db_fetch_scalar!(
        &pool,
        i64,
        "SELECT COUNT(*) FROM token_bootstrap_tokens WHERE token_hash = 'dev-bt-audit-service'"
    )
    .expect("count");
    assert_eq!(dev_count, 0);
}

// ── principal 校验（启动 fail-closed）───────────────────────────────────────

/// #6：P0 principal（iam-service）缺失 → validate_required_principals 报错，
/// main.rs 会拒绝启动（listener 永不 bind）。
#[tokio::test]
async fn missing_p0_principal_fails_startup_validation() {
    let pool = common::spawn_mysql().await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    // iam-service 的 bootstrap token 不存在
    let res = token_service::models::service_jwt_grant::validate_required_principals(&pool).await;
    assert!(res.is_err(), "P0 principal 缺失必须 fail-closed");
    let err = res.unwrap_err();
    assert!(err.contains("iam-service"));
    assert!(!err.contains("dev-bt"), "诊断信息不得包含 secret 材料");
}

/// NTF principal 缺失 → 仅 warn，校验通过（可选部署的显式决策）。
#[tokio::test]
async fn optional_ntf_principal_absent_passes_validation() {
    let pool = common::spawn_mysql().await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    // notification-service 未配置 → Ok（P1 条件性强制）
    token_service::models::service_jwt_grant::validate_required_principals(&pool)
        .await
        .expect("NTF 缺失不应阻塞启动");
}

// ── rotation / 宽限期 ──────────────────────────────────────────────────────

/// #18：宽限期旧 token（owner#old 行）解析到同一 canonical owner、
/// 同一 grant 集合；签出的 sub 也是 canonical owner。
#[tokio::test]
async fn grace_period_token_resolves_to_canonical_owner_grants() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam-current", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    // rotate with grace：当前行换新 token，旧 token 进 `iam-service#old` 行
    let (new_plain, old_expiry) =
        token_service::models::bootstrap_token::rotate(&pool, "iam-service", 3600)
            .await
            .expect("rotate")
            .expect("owner exists");
    assert!(old_expiry.is_some());
    assert_ne!(new_plain, "bt-iam-current");

    // 旧 token（宽限期内）仍可换取 aud=token-service —— grant 按 canonical owner
    let (status, body) = post_service_jwt(
        app(pool),
        "bt-iam-current",
        json!({"aud": "token-service", "tid": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let cache = digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).unwrap();
    let claims = verify_token(
        body["access_token"].as_str().unwrap(),
        &cache,
        Some("token-service"),
    )
    .await
    .unwrap();
    assert_eq!(
        claims["sub"], "iam-service",
        "宽限 token 签出的 sub 必须是 canonical owner，而非 iam-service#old"
    );
}

/// #19：rotate 不丢失授权（grant 属于 canonical owner，与密钥解耦）。
#[tokio::test]
async fn rotation_does_not_lose_grants() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");
    token_service::models::bootstrap_token::rotate(&pool, "iam-service", 0)
        .await
        .expect("rotate")
        .expect("owner exists");

    let grants = token_service::models::service_jwt_grant::list_all(&pool)
        .await
        .expect("list grants");
    assert_eq!(grants.len(), 2, "rotate 不得增删 grant 行");
}

// ── system-managed 保护 ────────────────────────────────────────────────────

/// #20：普通管理面（bootstrap token admin 的 delete/set_status）不触碰
/// grant 表——系统 grant 不可通过既有管理端点删除或禁用。
#[tokio::test]
async fn admin_token_ops_cannot_touch_system_grants() {
    let pool = common::spawn_mysql().await;
    insert_token(&pool, "iam-service", "bt-iam", "active").await;
    token_service::models::service_jwt_grant::ensure_system_grants(&pool)
        .await
        .expect("ensure grants");

    // 现有管理操作只作用于 token_bootstrap_tokens 表
    token_service::models::bootstrap_token::set_status(&pool, "iam-service", "disabled")
        .await
        .expect("disable owner");
    token_service::models::bootstrap_token::delete(&pool, "iam-service")
        .await
        .expect("delete owner");

    let grants = token_service::models::service_jwt_grant::list_all(&pool)
        .await
        .expect("list grants");
    assert_eq!(grants.len(), 2, "bootstrap token 管理操作不得影响 grant 表");
    assert!(grants.iter().all(|g| g.status == "active"));
}
// ── WP-02-R1 #7 MySQL 半边：grant 表主键恰为 (owner, audience) ──────────────

/// 与 pg_grant_unique_test.rs::pg_grant_pk_is_owner_audience 对齐：
/// 双方言 schema 的主键列必须一致。
#[derive(sqlx::FromRow)]
struct PkColumn {
    column_name: String,
}

#[tokio::test]
async fn mysql_grant_pk_is_owner_audience() {
    let pool = common::spawn_mysql().await;

    let cols: Vec<PkColumn> = db_fetch_all!(
        &pool,
        PkColumn,
        "SELECT COLUMN_NAME AS column_name FROM information_schema.STATISTICS \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'token_service_jwt_grants' \
           AND INDEX_NAME = 'PRIMARY' ORDER BY SEQ_IN_INDEX"
    )
    .expect("query primary key columns");

    let names: Vec<&str> = cols.iter().map(|c| c.column_name.as_str()).collect();
    assert_eq!(
        names,
        vec!["owner", "audience"],
        "MySQL grant table pk must be exactly (owner, audience)"
    );
}
