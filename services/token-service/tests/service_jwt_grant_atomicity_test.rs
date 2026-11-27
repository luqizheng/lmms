//! WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-02 (P1-02)：grant 变更与审计写入的
//! **原子性**验证（真实事务 + 故障注入 + 并发，双方言）。
//!
//! 覆盖任务书故障注入矩阵：
//! 1. CREATE：grant INSERT 成功、审计 INSERT 失败 → 最终 grant 不存在；
//! 2. UPDATE：grant UPDATE 成功、审计 INSERT 失败 → 状态保持原值；
//! 3. DELETE：grant DELETE 成功、审计 INSERT 失败 → grant 仍然存在；
//! 4. 正常操作：变更与审计同时成功，before/after/actor/action/request_id
//!    逐字段断言；
//! 5. 并发：启停串行化且恰一次 success 审计；对抗性启停的最终状态与最后
//!    一条 success 审计一致（业务与审计不可分离）；并发删除恰一成功；
//!    system_managed 行的并发启停全部 409 且无 success 审计。
//!
//! 注入方式：`ALTER TABLE token_service_jwt_grant_audit RENAME TO <唯一名>`
//! （双方言语法一致）→ 事务内审计 INSERT 必失败 → 断言 grant 终态后还原。
//! 每个测试建独立数据库（shared-MySQL 模式 / PG 独立 CREATE DATABASE），
//! 注入与还原都发生在本测试自己的库内，不外溢。
//!
//! MySQL 走 `TEST_MYSQL_HOST/PORT` 共享库（无变量时回退 testcontainers）；
//! PostgreSQL 走 `TEST_PG_URL` 外部容器模式，无变量时打印 `[skip]` 放行
//!（与 pg_roundtrip_test 同约定）。

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use digua_common_db::{db_execute, db_fetch_all, db_fetch_one, db_fetch_optional, DbPool};
use serde_json::{json, Value};
use tokio::task::JoinSet;
use tower::ServiceExt;

mod common;

use common::{build_state_with_admins, spawn_mysql, TEST_PRIVATE_PEM};

// ── scaffolding（双方言共享）─────────────────────────────────────────────────

fn admin_app(pool: &DbPool) -> axum::Router {
    token_service::routes::service_jwt_grant_admin::routes(build_state_with_admins(
        pool.clone(),
        vec!["token-service"],
    ))
}

/// 清单内管理主体的 service-jwt（sub=token-service，aud=token-service）。
fn admin_bearer() -> String {
    token_service::mint_service_bearer(TEST_PRIVATE_PEM, "token-service", 3600)
        .expect("mint service bearer")
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

/// 固定 request-id（合法 UUID），供审计字段断言。
const AUDIT_RID: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

fn req_with_rid(method: &str, uri: &str, bearer: &str, body: Option<Value>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header("X-Request-Id", AUDIT_RID)
        .body(match body {
            Some(v) => Body::from(v.to_string()),
            None => Body::empty(),
        })
        .expect("build request")
}

async fn body_text(resp: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("read body");
    String::from_utf8_lossy(&bytes).into_owned()
}

/// 故障注入：把审计表改名（双方言一致 DDL），返回备份表名。
/// 名字带 UUID 后缀防重名（总长 ≤64：MySQL 标识符上限）。
async fn inject_audit_table_failure(pool: &DbPool) -> String {
    let backup = format!("token_audit_inject_{}", uuid::Uuid::new_v4().simple());
    let sql = format!("ALTER TABLE token_service_jwt_grant_audit RENAME TO {backup}");
    db_execute!(pool, sql).expect("rename audit table (inject audit failure)");
    backup
}

/// 还原审计表（备份名由 [`inject_audit_table_failure`] 返回）。
async fn restore_audit_table(pool: &DbPool, backup: &str) {
    let sql = format!("ALTER TABLE {backup} RENAME TO token_service_jwt_grant_audit");
    db_execute!(pool, sql).expect("restore audit table name");
}

async fn insert_grant_raw(pool: &DbPool, owner: &str, aud: &str, status: &str, sys: bool) {
    let flag = if sys { "TRUE" } else { "FALSE" };
    let sql = format!(
        "INSERT INTO token_service_jwt_grants (owner, audience, status, system_managed) \
         VALUES (?, ?, ?, {flag})"
    );
    db_execute!(pool, sql, owner, aud, status).expect("insert grant raw");
}

async fn grant_exists(pool: &DbPool, owner: &str, aud: &str) -> bool {
    let n: Option<(i64,)> = db_fetch_optional!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        aud
    )
    .expect("count grant");
    n.map(|(c,)| c).unwrap_or(0) > 0
}

async fn grant_status(pool: &DbPool, owner: &str, aud: &str) -> Option<String> {
    let row: Option<(String,)> = db_fetch_optional!(
        pool,
        (String,),
        "SELECT status FROM token_service_jwt_grants WHERE owner = ? AND audience = ?",
        owner,
        aud
    )
    .expect("query grant status");
    row.map(|(s,)| s)
}

/// 全部 success 审计行：(action, after_status, before_status, detail)。
/// after/before 列可 NULL（create 的 before、delete 的 after）。
async fn success_audits(pool: &DbPool) -> Vec<(String, Option<String>, Option<String>, String)> {
    let rows: Vec<(String, Option<String>, Option<String>, String)> = db_fetch_all!(
        pool,
        (String, Option<String>, Option<String>, String),
        "SELECT action, after_status, before_status, detail FROM token_service_jwt_grant_audit \
         WHERE result = 'success'"
    )
    .expect("query success audits");
    rows
}

async fn success_audit_count(pool: &DbPool) -> i64 {
    let n: Option<(i64,)> = db_fetch_optional!(
        pool,
        (i64,),
        "SELECT COUNT(*) FROM token_service_jwt_grant_audit WHERE result = 'success'"
    )
    .expect("count success audits");
    n.map(|(c,)| c).unwrap_or(0)
}

// ── P1-02 故障注入核心用例（双方言共享）──────────────────────────────────────

/// 1. CREATE：grant INSERT 成功、审计 INSERT 失败 → grant 不存在。
async fn case_create_rolls_back_when_audit_fails(pool: &DbPool) {
    let app = admin_app(pool);
    assert!(!grant_exists(pool, "atomic-create", "iam-service").await);

    let backup = inject_audit_table_failure(pool).await;
    let resp = app
        .oneshot(req(
            "POST",
            "/admin/service-jwt-grants",
            Some(&admin_bearer()),
            Some(json!({"owner": "atomic-create", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    let text = body_text(resp).await;
    assert_eq!(
        text, "audit write failed",
        "audit failure must surface as the fail-closed 500 body"
    );
    restore_audit_table(pool, &backup).await;

    // 真实数据库终态：grant 必须不存在（审计失败 → 业务回滚），无 success 审计。
    assert!(
        !grant_exists(pool, "atomic-create", "iam-service").await,
        "grant must NOT exist after audit INSERT failure (tx rolled back)"
    );
    assert_eq!(
        success_audit_count(pool).await,
        0,
        "no success audit may survive a rolled-back create"
    );
    // 表还原本身也被验证过：后续正常请求可用（信息完整性）。
    let resp = admin_app(pool)
        .oneshot(req(
            "GET",
            "/admin/service-jwt-grants",
            Some(&admin_bearer()),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// 2. UPDATE：grant UPDATE 成功、审计 INSERT 失败 → 状态保持原值。
async fn case_update_rolls_back_when_audit_fails(pool: &DbPool) {
    insert_grant_raw(pool, "atomic-upd", "iam-service", "active", false).await;
    let app = admin_app(pool);

    let backup = inject_audit_table_failure(pool).await;
    let resp = app
        .oneshot(req(
            "PUT",
            "/admin/service-jwt-grants/atomic-upd/iam-service/status",
            Some(&admin_bearer()),
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    let text = body_text(resp).await;
    assert_eq!(text, "audit write failed");
    restore_audit_table(pool, &backup).await;

    assert_eq!(
        grant_status(pool, "atomic-upd", "iam-service")
            .await
            .as_deref(),
        Some("active"),
        "status must stay `active` after audit INSERT failure (tx rolled back)"
    );
    assert_eq!(success_audit_count(pool).await, 0);
}

/// 3. DELETE：grant DELETE 成功、审计 INSERT 失败 → grant 仍然存在。
async fn case_delete_rolls_back_when_audit_fails(pool: &DbPool) {
    insert_grant_raw(pool, "atomic-del", "iam-service", "active", false).await;
    let app = admin_app(pool);

    let backup = inject_audit_table_failure(pool).await;
    let resp = app
        .oneshot(req(
            "DELETE",
            "/admin/service-jwt-grants/atomic-del/iam-service",
            Some(&admin_bearer()),
            None,
        ))
        .await
        .unwrap();
    let text = body_text(resp).await;
    assert_eq!(text, "audit write failed");
    restore_audit_table(pool, &backup).await;

    assert!(
        grant_exists(pool, "atomic-del", "iam-service").await,
        "grant must still exist after audit INSERT failure (tx rolled back)"
    );
    assert_eq!(success_audit_count(pool).await, 0);
}

/// 4. 正常操作：变更与审计同时提交，审计字段逐项断言。
async fn case_success_path_audit_context(pool: &DbPool) {
    let app = admin_app(pool);
    let bearer = admin_bearer();

    // CREATE：before=NULL、after=active、request_id 原样落库。
    let resp = app
        .clone()
        .oneshot(req_with_rid(
            "POST",
            "/admin/service-jwt-grants",
            &bearer,
            Some(json!({"owner": "atomic-ok", "audience": "iam-service"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let row: (
        String,
        String,
        Option<String>,
        Option<String>,
        String,
        String,
        String,
    ) = db_fetch_one!(
        pool,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            String,
            String,
            String
        ),
        "SELECT actor_id, actor_type, before_status, after_status, request_id, action, detail \
             FROM token_service_jwt_grant_audit \
             WHERE owner = 'atomic-ok' AND action = 'create'"
    )
    .expect("create audit row");
    let (actor_id, actor_type, before, after, request_id, _action, _detail) = row;
    assert_eq!(actor_id, "token-service");
    assert_eq!(actor_type, "service");
    assert_eq!(before, None, "create has no before state");
    assert_eq!(after.as_deref(), Some("active"));
    assert_eq!(
        request_id, AUDIT_RID,
        "client request-id must be persisted verbatim"
    );

    // DISABLE：before=active → after=disabled。
    let resp = app
        .clone()
        .oneshot(req_with_rid(
            "PUT",
            "/admin/service-jwt-grants/atomic-ok/iam-service/status",
            &bearer,
            Some(json!({"status": "disabled"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let row: (String, Option<String>, Option<String>, String) = db_fetch_one!(
        pool,
        (String, Option<String>, Option<String>, String),
        "SELECT action, after_status, before_status, result \
         FROM token_service_jwt_grant_audit WHERE owner = 'atomic-ok' AND action = 'disable'"
    )
    .expect("disable audit row");
    let (action, after, before, result) = row;
    assert_eq!(action, "disable");
    assert_eq!(before.as_deref(), Some("active"));
    assert_eq!(after.as_deref(), Some("disabled"));
    assert_eq!(result, "success");

    // ENABLE：before=disabled → after=active。
    let resp = app
        .clone()
        .oneshot(req_with_rid(
            "PUT",
            "/admin/service-jwt-grants/atomic-ok/iam-service/status",
            &bearer,
            Some(json!({"status": "active"})),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let row: (Option<String>, Option<String>) = db_fetch_one!(
        pool,
        (Option<String>, Option<String>),
        "SELECT before_status, after_status FROM token_service_jwt_grant_audit \
         WHERE owner = 'atomic-ok' AND action = 'enable'"
    )
    .expect("enable audit row");
    assert_eq!(row, (Some("disabled".into()), Some("active".into())));

    // DELETE：before=active（恢复后的值）、after=NULL，与行一起提交。
    let resp = app
        .clone()
        .oneshot(req_with_rid(
            "DELETE",
            "/admin/service-jwt-grants/atomic-ok/iam-service",
            &bearer,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let row: (Option<String>, Option<String>) = db_fetch_one!(
        pool,
        (Option<String>, Option<String>),
        "SELECT before_status, after_status FROM token_service_jwt_grant_audit \
         WHERE owner = 'atomic-ok' AND action = 'delete' AND result = 'success'"
    )
    .expect("delete audit row");
    assert_eq!(row.1, None, "delete has no after state");
    assert!(!grant_exists(pool, "atomic-ok", "iam-service").await);
    // 变更与审计的原子共存：grant 没了，但 delete success 审计在。
    // create + disable + enable + delete = 4 条 success 审计。
    assert_eq!(
        success_audit_count(pool).await,
        4,
        "create/disable/enable/delete"
    );
}

// ── P1-02 并发用例（双方言共享）──────────────────────────────────────────────

/// 5a. 并发 disable 同一行：行锁串行化 → 全部 200（1 次真写 + 其余幂等），
/// success 审计**恰 1 行**，终态 disabled。
async fn case_concurrent_disable_single_success_audit(pool: &DbPool) {
    insert_grant_raw(pool, "race-disable", "iam-service", "active", false).await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    let mut set = JoinSet::new();
    for _ in 0..8 {
        let app = app.clone();
        let bearer = bearer.clone();
        set.spawn(async move {
            app.oneshot(req(
                "PUT",
                "/admin/service-jwt-grants/race-disable/iam-service/status",
                Some(&bearer),
                Some(json!({"status": "disabled"})),
            ))
            .await
            .unwrap()
        });
    }
    let mut ok = 0usize;
    while let Some(res) = set.join_next().await {
        let resp = res.expect("join race task");
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "concurrent disable must all be idempotent-200"
        );
        ok += 1;
    }
    assert_eq!(ok, 8);

    assert_eq!(
        grant_status(pool, "race-disable", "iam-service")
            .await
            .as_deref(),
        Some("disabled")
    );
    let rows = success_audits(pool).await;
    assert_eq!(
        rows.len(),
        1,
        "exactly one success audit for 8 concurrent disables (row lock serializes): {rows:?}"
    );
    assert_eq!(rows[0].0, "disable");
    assert_eq!(rows[0].1.as_deref(), Some("disabled"));
}

/// 5b. 对抗性并发启停：每个真实翻转恰一行 success 审计，最终状态与
/// **最后一条 success 审计的 after_status** 一致（业务与审计不可分离）。
async fn case_concurrent_opposing_toggles_end_state_matches_last_audit(pool: &DbPool) {
    insert_grant_raw(pool, "race-toggle", "iam-service", "active", false).await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    let mut set = JoinSet::new();
    for i in 0..10 {
        let app = app.clone();
        let bearer = bearer.clone();
        let target = if i % 2 == 0 { "disabled" } else { "active" };
        set.spawn(async move {
            app.oneshot(req(
                "PUT",
                "/admin/service-jwt-grants/race-toggle/iam-service/status",
                Some(&bearer),
                Some(json!({ "status": target })),
            ))
            .await
            .unwrap()
        });
    }
    let mut ok = 0usize;
    while let Some(res) = set.join_next().await {
        let resp = res.expect("join toggle task");
        assert_eq!(resp.status(), StatusCode::OK);
        ok += 1;
    }
    assert_eq!(ok, 10);

    // 审计翻转序列（occurred_at 同秒会并列，按自增 id 排序——id 单调递增
    // 且与提交顺序一致，双方言）。
    let flips: Vec<(String, Option<String>)> = db_fetch_all!(
        pool,
        (String, Option<String>),
        "SELECT action, after_status FROM token_service_jwt_grant_audit \
         WHERE owner = 'race-toggle' AND result = 'success' ORDER BY id"
    )
    .expect("query flip audits");
    assert!(
        !flips.is_empty(),
        "at least one real toggle must happen among 10 opposing requests"
    );
    // 每条翻转记录的 after 都与 action 匹配（enable→active / disable→disabled）。
    for (action, after) in &flips {
        let expect = if action == "enable" {
            "active"
        } else {
            "disabled"
        };
        assert_eq!(
            after.as_deref(),
            Some(expect),
            "flip audit {action} must record after={expect}"
        );
    }
    // 最终状态 == 最后一条 success 审计的 after（业务与审计一致）。
    let last_after = flips.last().unwrap().1.clone();
    let final_status = grant_status(pool, "race-toggle", "iam-service").await;
    assert_eq!(
        final_status.as_deref(),
        last_after.as_deref(),
        "final grant status must equal the last success audit's after_status"
    );
}

/// 5c. 并发 delete 同一行：恰一次真删（204），其余 404（幂等语义下
/// 不可能两个都删成功），success delete 审计恰 1 行。
async fn case_concurrent_delete_single_success_audit(pool: &DbPool) {
    insert_grant_raw(pool, "race-del", "iam-service", "active", false).await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    let mut set = JoinSet::new();
    for _ in 0..6 {
        let app = app.clone();
        let bearer = bearer.clone();
        set.spawn(async move {
            app.oneshot(req(
                "DELETE",
                "/admin/service-jwt-grants/race-del/iam-service",
                Some(&bearer),
                None,
            ))
            .await
            .unwrap()
        });
    }
    let mut deleted = 0usize;
    let mut not_found = 0usize;
    while let Some(res) = set.join_next().await {
        let resp = res.expect("join delete task");
        match resp.status() {
            StatusCode::NO_CONTENT => deleted += 1,
            StatusCode::NOT_FOUND => not_found += 1,
            other => panic!("unexpected status {other} in concurrent delete"),
        }
    }
    assert_eq!(deleted, 1, "exactly one delete may win the row lock");
    assert_eq!(not_found, 5, "losers must observe the row gone (404)");
    assert!(!grant_exists(pool, "race-del", "iam-service").await);
    let rows = success_audits(pool).await;
    assert_eq!(rows.len(), 1, "exactly one success delete audit");
    assert_eq!(rows[0].0, "delete");
    assert_eq!(rows[0].1, None, "delete audit has no after state");
}

/// 5d. system_managed 行的并发 disable：全部 409、行不动、零 success 审计。
async fn case_concurrent_disable_on_system_managed_all_denied(pool: &DbPool) {
    insert_grant_raw(pool, "race-sys", "iam-service", "active", true).await;
    let app = admin_app(pool);
    let bearer = admin_bearer();

    let mut set = JoinSet::new();
    for _ in 0..8 {
        let app = app.clone();
        let bearer = bearer.clone();
        set.spawn(async move {
            app.oneshot(req(
                "PUT",
                "/admin/service-jwt-grants/race-sys/iam-service/status",
                Some(&bearer),
                Some(json!({"status": "disabled"})),
            ))
            .await
            .unwrap()
        });
    }
    while let Some(res) = set.join_next().await {
        let resp = res.expect("join sys task");
        assert_eq!(
            resp.status(),
            StatusCode::CONFLICT,
            "system_managed row must refuse every concurrent disable"
        );
    }
    let row: (String, bool) = db_fetch_one!(
        pool,
        (String, bool),
        "SELECT status, system_managed FROM token_service_jwt_grants \
         WHERE owner = 'race-sys' AND audience = 'iam-service'"
    )
    .expect("query sys row");
    assert_eq!(row, ("active".into(), true), "system row untouched");
    assert_eq!(
        success_audit_count(pool).await,
        0,
        "denials must not produce success audits"
    );
}

// ── MySQL 入口 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn mysql_create_rolls_back_when_audit_insert_fails() {
    let pool = spawn_mysql().await;
    case_create_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
async fn mysql_update_rolls_back_when_audit_insert_fails() {
    let pool = spawn_mysql().await;
    case_update_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
async fn mysql_delete_rolls_back_when_audit_insert_fails() {
    let pool = spawn_mysql().await;
    case_delete_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
async fn mysql_success_path_audits_full_context() {
    let pool = spawn_mysql().await;
    case_success_path_audit_context(&pool).await;
}

#[tokio::test]
async fn mysql_concurrent_disable_yields_single_success_audit() {
    let pool = spawn_mysql().await;
    case_concurrent_disable_single_success_audit(&pool).await;
}

#[tokio::test]
async fn mysql_concurrent_opposing_toggles_match_last_audit() {
    let pool = spawn_mysql().await;
    case_concurrent_opposing_toggles_end_state_matches_last_audit(&pool).await;
}

#[tokio::test]
async fn mysql_concurrent_delete_yields_single_success_audit() {
    let pool = spawn_mysql().await;
    case_concurrent_delete_single_success_audit(&pool).await;
}

#[tokio::test]
async fn mysql_concurrent_system_managed_disables_all_denied() {
    let pool = spawn_mysql().await;
    case_concurrent_disable_on_system_managed_all_denied(&pool).await;
}

// ── PostgreSQL 入口（TEST_PG_URL 外部容器模式；无变量时 [skip]）──────────────

#[cfg(feature = "postgres")]
fn test_pg_url() -> Option<String> {
    match std::env::var("TEST_PG_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => None,
    }
}

#[cfg(feature = "postgres")]
async fn spawn_pg() -> Option<DbPool> {
    let url = test_pg_url()?;
    let db_name = format!(
        "test_token_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let (base, _old) = url.rsplit_once('/').expect("TEST_PG_URL must contain /db");

    let admin = sqlx::PgPool::connect(url.as_str())
        .await
        .expect("admin connect pg");
    sqlx::query(&format!("CREATE DATABASE \"{db_name}\""))
        .execute(&admin)
        .await
        .expect("create pg test database");
    admin.close().await;

    let pool = digua_common_db::init_pool(&format!("{base}/{db_name}"))
        .await
        .expect("init pg pool");
    token_service::db::ensure_schema(&pool)
        .await
        .expect("pg ensure_schema");
    Some(pool)
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_create_rolls_back_when_audit_insert_fails() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_create_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_update_rolls_back_when_audit_insert_fails() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_update_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_delete_rolls_back_when_audit_insert_fails() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_delete_rolls_back_when_audit_fails(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_success_path_audits_full_context() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_success_path_audit_context(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_concurrent_disable_yields_single_success_audit() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_concurrent_disable_single_success_audit(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_concurrent_opposing_toggles_match_last_audit() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_concurrent_opposing_toggles_end_state_matches_last_audit(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_concurrent_delete_yields_single_success_audit() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_concurrent_delete_single_success_audit(&pool).await;
}

#[tokio::test]
#[cfg(feature = "postgres")]
async fn pg_concurrent_system_managed_disables_all_denied() {
    let Some(pool) = spawn_pg().await else {
        eprintln!("[pg grant atomicity] TEST_PG_URL not set — skipped");
        return;
    };
    case_concurrent_disable_on_system_managed_all_denied(&pool).await;
}
