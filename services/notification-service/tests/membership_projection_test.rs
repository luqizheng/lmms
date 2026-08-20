//! Phase 5B-fu3: ntf_user_projection apply/resolve/bootstrap integration tests.
//!
//! Covers:
//! - apply_role_assigned / unassigned / dept_changed idempotency + ts ordering
//! - out-of-order event dropped (spec §3.6 D6)
//! - resolve_users_by_scopes by role (JSON_CONTAINS) + by dept
//! - replace_projection (bootstrap path) — DELETE + INSERT batch

mod common;

use digua_jwt_ctx::TenantScope;
use notification_service::db::Db;
use std::collections::HashMap;
use std::collections::HashSet;

async fn setup() -> (Db, Option<common::MysqlContainer>) {
    let h = common::spawn_db().await;
    (h.db, h.container)
}

// ===== apply_* tests =====

#[tokio::test]
async fn apply_role_assigned_inserts_then_resolves_by_role() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u1", "r1", 1, 100).await.unwrap();
    db.apply_role_assigned("u2", "r1", 1, 101).await.unwrap();
    db.apply_role_assigned("u3", "r2", 1, 102).await.unwrap();

    let out = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert_eq!(out, HashSet::from(["u1".into(), "u2".into()]));
}

#[tokio::test]
async fn apply_role_unassigned_removes_role_from_json_array() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u1", "r1", 1, 100).await.unwrap();
    db.apply_role_assigned("u1", "r2", 1, 101).await.unwrap();
    db.apply_role_unassigned("u1", "r1", 1, 200).await.unwrap();

    let r1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let r2 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r2".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(r1.is_empty(), "u1 removed from r1");
    assert_eq!(r2, HashSet::from(["u1".into()]), "u1 still in r2");
}

#[tokio::test]
async fn apply_role_unassigned_on_missing_user_is_noop() {
    let (db, _c) = setup().await;
    // user never had any role events — unassign should not error
    db.apply_role_unassigned("ghost", "r1", 1, 100)
        .await
        .unwrap();
    let out = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(out.is_empty());
}

#[tokio::test]
async fn apply_dept_changed_updates_dept_id() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u1", "r1", 1, 100).await.unwrap();
    db.apply_dept_changed("u1", Some("d1"), 1, 200)
        .await
        .unwrap();
    db.apply_dept_changed("u1", Some("d2"), 1, 300)
        .await
        .unwrap();

    let d1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Dept, "d1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let d2 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Dept, "d2".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(d1.is_empty());
    assert_eq!(d2, HashSet::from(["u1".into()]));
}

#[tokio::test]
async fn apply_dept_changed_to_none_clears_dept() {
    let (db, _c) = setup().await;
    db.apply_dept_changed("u1", Some("d1"), 1, 100)
        .await
        .unwrap();
    db.apply_dept_changed("u1", None, 1, 200).await.unwrap();

    let d1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Dept, "d1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(d1.is_empty());
}

// ===== out-of-order =====

#[tokio::test]
async fn out_of_order_event_does_not_overwrite_newer_state() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u1", "r1", 1, 100).await.unwrap();
    db.apply_dept_changed("u1", Some("d_new"), 1, 200)
        .await
        .unwrap();

    // Replay older events with smaller ts — must be ignored
    db.apply_role_assigned("u1", "r_old", 1, 50).await.unwrap();
    db.apply_dept_changed("u1", Some("d_old"), 1, 50)
        .await
        .unwrap();

    let r_old = db
        .resolve_users_by_scopes(
            &[(
                notification_service::models::ScopeType::Role,
                "r_old".into(),
            )],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let r_new = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let d_old = db
        .resolve_users_by_scopes(
            &[(
                notification_service::models::ScopeType::Dept,
                "d_old".into(),
            )],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let d_new = db
        .resolve_users_by_scopes(
            &[(
                notification_service::models::ScopeType::Dept,
                "d_new".into(),
            )],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(r_old.is_empty(), "stale role event dropped");
    assert_eq!(r_new, HashSet::from(["u1".into()]), "newer role kept");
    assert!(d_old.is_empty(), "stale dept event dropped");
    assert_eq!(d_new, HashSet::from(["u1".into()]), "newer dept kept");
}

// ===== replace_projection (bootstrap path) =====

#[tokio::test]
async fn replace_projection_wipes_and_reinserts_for_tenant() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u_stale", "r1", 1, 100)
        .await
        .unwrap();
    let stale = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert_eq!(stale, HashSet::from(["u_stale".into()]));

    let mut mappings = HashMap::new();
    mappings.insert(
        "u1".to_string(),
        notification_service::models::UserMembership {
            role_ids: vec!["r1".into(), "r2".into()],
            dept_id: Some("d1".into()),
        },
    );
    mappings.insert(
        "u2".to_string(),
        notification_service::models::UserMembership {
            role_ids: vec![],
            dept_id: None,
        },
    );
    db.replace_projection(1, &mappings).await.unwrap();

    let r1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let r2 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r2".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    let d1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Dept, "d1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    assert!(!r1.contains("u_stale"), "stale user wiped");
    assert!(r1.contains("u1"));
    assert!(r2.contains("u1"));
    assert_eq!(d1, HashSet::from(["u1".into()]));
}

#[tokio::test]
async fn replace_projection_does_not_touch_other_tenants() {
    let (db, _c) = setup().await;
    db.apply_role_assigned("u_t1", "r1", 1, 100).await.unwrap();
    db.apply_role_assigned("u_t2", "r1", 2, 100).await.unwrap();

    let mut mappings = HashMap::new();
    mappings.insert(
        "u_new".to_string(),
        notification_service::models::UserMembership {
            role_ids: vec!["r1".into()],
            dept_id: None,
        },
    );
    db.replace_projection(1, &mappings).await.unwrap();

    // tenant 1: u_t1 wiped, u_new added
    let r1_t1 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(1),
        )
        .await
        .unwrap();
    // tenant 2: u_t2 untouched
    let r1_t2 = db
        .resolve_users_by_scopes(
            &[(notification_service::models::ScopeType::Role, "r1".into())],
            &TenantScope::Single(2),
        )
        .await
        .unwrap();
    assert!(!r1_t1.contains("u_t1"));
    assert!(r1_t1.contains("u_new"));
    assert_eq!(r1_t2, HashSet::from(["u_t2".into()]));
}
