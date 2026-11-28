//! 全新部署 Bootstrap 顺序可行性测试（WP-BOOTSTRAP-01-R1 / P2-01）。
//!
//! 证明「token-service 启动初始化（身份 seed + 系统 Grant seed）→
//! Bootstrap S05 readiness / S06-S07 ensure-grants」这条全新数据库部署
//! 路径可行，且身份初始化的权威责任方是 token-service 启动流程：
//!
//! 1. `bootstrap_token::seed_defaults`（env CSV / dev 默认值 →
//!    `token_bootstrap_tokens`，per-owner 幂等）；
//! 2. `service_jwt_grant::ensure_system_grants`（code-owned 幂等）；
//! 3. `validate_required_principals`（iam-service 主体缺失 → 拒绝启动，
//!    fail-closed）。
//!
//! Bootstrap 不创建/不覆盖内部身份；S05 只读 readiness，S06/S07 只 ensure
//! Grant（`system_managed = TRUE`，幂等键 = owner+audience 主键）。

mod common;

use axum::Json;
use token_service::models::{bootstrap_token as bt, service_jwt_grant as grant};
use token_service::routes::internal_bootstrap::{
    handle_ensure_grants, handle_readiness, EnsureGrantsRequest,
};

/// 模拟 token-service 启动 seed（与 main.rs 顺序一致：身份 → grants → 校验）。
async fn run_startup_seed(pool: &token_service::db::DbPool) {
    // 空 CSV → resolve_seed_pairs 走 dev 默认值（生产由 env 注入真实对）。
    let seeded = bt::seed_defaults(pool, "", "")
        .await
        .expect("seed bootstrap tokens");
    assert!(seeded > 0, "fresh deploy must seed identities");
    let grants = grant::ensure_system_grants(pool)
        .await
        .expect("ensure system grants");
    assert!(grants > 0, "fresh deploy must seed system grants");
    grant::validate_required_principals(pool)
        .await
        .expect("required principals present after seed");
}

#[tokio::test]
async fn fresh_deploy_startup_seed_then_readiness_ready() {
    let pool = common::spawn_mysql().await;
    run_startup_seed(&pool).await;

    let state = common::build_state(pool.clone());

    // S05 数据源：readiness 全绿（身份 active + grant active）。
    let readiness = handle_readiness(axum::extract::State(state.clone()))
        .await
        .expect("readiness ok");
    assert!(
        readiness
            .identities
            .iter()
            .all(|i| i.exists && i.status.as_deref() == Some("active")),
        "all required identities must be active after startup seed: {:?}",
        readiness.identities
    );
    assert!(
        readiness
            .grants
            .iter()
            .all(|g| g.exists && g.status.as_deref() == Some("active")),
        "all system grants must be active after startup seed: {:?}",
        readiness.grants
    );
    let owners: Vec<&str> = readiness
        .identities
        .iter()
        .map(|i| i.owner.as_str())
        .collect();
    assert!(owners.contains(&"iam-service"));
    assert!(owners.contains(&"notification-service"));

    // S06/S07 数据源：ensure-grants 幂等（二次调用无新增、无冲突）。
    let ensure = handle_ensure_grants(
        axum::extract::State(state),
        Json(EnsureGrantsRequest::default()),
    )
    .await
    .expect("ensure ok");
    assert!(ensure.conflicts.is_empty());
    assert_eq!(ensure.ensured.len(), grant::SYSTEM_GRANTS.len());
    assert!(ensure.ensured.iter().all(|g| g.exists
        && g.status.as_deref() == Some("active")
        && g.system_managed == Some(true)));
}

#[tokio::test]
async fn ensure_grants_reports_disabled_row_as_conflict_without_override() {
    let pool = common::spawn_mysql().await;
    run_startup_seed(&pool).await;
    let state = common::build_state(pool.clone());

    // 人工禁用一条系统 grant（system_managed 行受管理面保护，直接 SQL
    // 构造"已存在但被禁用"状态——生产中此类行只能人工 DB 处置，文档见
    // docs/bootstrap-deploy.md 冲突排查节）。
    sqlx::query(
        "UPDATE token_service_jwt_grants SET status = 'disabled'          WHERE owner = 'iam-service' AND audience = 'token-service'",
    )
    .execute(pool.as_mysql().expect("mysql pool"))
    .await
    .expect("disable grant row");

    let ensure = handle_ensure_grants(
        axum::extract::State(state),
        Json(EnsureGrantsRequest::default()),
    )
    .await
    .expect("ensure ok");
    assert!(
        ensure
            .conflicts
            .iter()
            .any(|c| c.owner == "iam-service" && c.audience == "token-service"),
        "disabled row must surface as conflict: {:?}",
        ensure.conflicts
    );
    assert!(
        !ensure
            .ensured
            .iter()
            .any(|g| g.owner == "iam-service" && g.audience == "token-service"),
        "disabled row must not be listed as ensured"
    );

    // 不覆盖：禁用状态保持（ensure 不复活被显式撤销的授权）。
    let row = grant::find_by_owner_and_audience(&pool, "iam-service", "token-service")
        .await
        .unwrap()
        .expect("row exists");
    assert_eq!(row.status, "disabled");
}

#[tokio::test]
async fn readiness_detects_missing_identity() {
    let pool = common::spawn_mysql().await;
    run_startup_seed(&pool).await;
    let state = common::build_state(pool.clone());

    // 删除 notification-service 身份行（模拟 env 漏配）。
    sqlx::query("DELETE FROM token_bootstrap_tokens WHERE owner = 'notification-service'")
        .execute(pool.as_mysql().expect("mysql pool"))
        .await
        .unwrap();

    let readiness = handle_readiness(axum::extract::State(state))
        .await
        .expect("readiness ok");
    let ntf = readiness
        .identities
        .iter()
        .find(|i| i.owner == "notification-service")
        .expect("notification identity listed");
    assert!(!ntf.exists, "missing identity must surface as not-exists");
    // iam-service 主体仍在（validate_required_principals 的 fail-closed 对象）。
    let iam = readiness
        .identities
        .iter()
        .find(|i| i.owner == "iam-service")
        .expect("iam identity listed");
    assert!(iam.exists && iam.status.as_deref() == Some("active"));
}

#[tokio::test]
async fn missing_iam_principal_fails_startup_validation() {
    let pool = common::spawn_mysql().await;
    run_startup_seed(&pool).await;

    // 删除 iam-service 主体 → 启动校验必须拒绝（fail-closed，登录链死锁防护）。
    sqlx::query("DELETE FROM token_bootstrap_tokens WHERE owner = 'iam-service'")
        .execute(pool.as_mysql().expect("mysql pool"))
        .await
        .unwrap();
    assert!(
        grant::validate_required_principals(&pool).await.is_err(),
        "startup validation must fail-closed without iam-service principal"
    );
}

#[tokio::test]
async fn startup_seed_is_idempotent_per_owner() {
    let pool = common::spawn_mysql().await;
    run_startup_seed(&pool).await;
    // 二次启动 seed：已有 owner 不覆盖（不产生重复身份/凭据变更）。
    let seeded_again = bt::seed_defaults(&pool, "", "").await.expect("second seed");
    assert_eq!(seeded_again, 0, "per-owner idempotent: no rows re-seeded");
    let grants_again = grant::ensure_system_grants(&pool)
        .await
        .expect("second grants");
    assert_eq!(grants_again, 0, "grant seed idempotent: no rows re-seeded");
}
