//! Phase 4.4-fu1: RoleService::assign_user_roles integration tests.
//!
//! Each case starts fresh MySQL + Redis containers, seeds users/roles via
//! raw SQL, then drives the SUT and asserts on outbox rows + Redis
//! perm_epoch. Serial execution via --test-threads=1 (see Cargo.toml or
//! run-tests-docker.js).

mod common;

use digua_common_cache::MemoryCache;
use digua_common_event::perm_epoch::{get_perm_epoch, perm_epoch_key};
use digua_common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::error::IamError;
use iam_service::service::RoleService;
use iam_service::state::AppState;
use digua_jwt_ctx::TenantScope;
use std::collections::BTreeSet;
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers_modules::{redis::Redis, testcontainers::runners::AsyncRunner};

const PREFIX: &str = "superpowers:";

struct Ctx {
    state: AppState,
    redis: redis::aio::ConnectionManager,
    _mysql: Option<common::MysqlContainer>,
    _redis: testcontainers::ContainerAsync<Redis>,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let url = mysql_h.url;

    let redis_c = Redis::default().start().await.unwrap();
    let r_port = redis_c.get_host_port_ipv4(6379).await.unwrap();
    let r_host = redis_c.get_host().await.unwrap();
    let client = redis::Client::open(format!("redis://{r_host}:{r_port}")).unwrap();
    let mgr = redis::aio::ConnectionManager::new(client).await.unwrap();

    let config = Config {
        app: AppConfig { name: "iam-service".into(), host: "0.0.0.0".into(), port: 8080 },
        database: DatabaseConfig { url: url.clone(), max_connections: 5, min_connections: 1 },
        redis: RedisConfig { url: format!("redis://{r_host}:{r_port}"), ttl_sec: 86400, prefix: PREFIX.into() },
        jwt: JwtConfig { private_key_path: "/app/keys/private.pem".into(), public_key_path: "/app/keys/public.pem".into(), access_token_ttl_minutes: 15, refresh_token_ttl_days: 7 },
        consul: ConsulConfig { host: "consul".into(), port: 8500, service_name: "iam-service".into() },
    };

    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let jwks = std::sync::Arc::new(digua_common_jwt::JWKSCache::new(
        digua_common_jwt::JwksConfig {
            url: "http://localhost:0/.well-known/jwks.json".into(),
            refresh_interval: std::time::Duration::from_secs(3600),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
    ));
    let service_jwt = std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));    let state = AppState::new(pool, cache, config, Some(mgr.clone()), outbox_repo, jwks, service_jwt);
    Ctx { state, redis: mgr, _mysql: mysql_h.container, _redis: redis_c }
}

// ---- seed helpers (copied from permission_changed_test.rs) ----

async fn seed_user(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_users (id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES (?, ?, 'h', 's', ?, 'active', NOW(), NOW())",
    ).bind(id).bind(id).bind(id).execute(&state.db).await.unwrap();
}

async fn seed_role(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'self', 'active', NOW(), NOW())",
    ).bind(id).bind(id).bind(id).execute(&state.db).await.unwrap();
}

async fn seed_disabled_role(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'self', 'disabled', NOW(), NOW())",
    ).bind(id).bind(id).bind(id).execute(&state.db).await.unwrap();
}

async fn assign_user_to_role(state: &AppState, user_id: &str, role_id: &str) {
    sqlx::query("INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())")
        .bind(user_id).bind(role_id).execute(&state.db).await.unwrap();
}

// ---- outbox helpers ----

async fn outbox_count(state: &AppState) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    ).fetch_one(&state.db).await.unwrap();
    n
}

async fn single_outbox_reason_and_users(state: &AppState) -> (String, BTreeSet<String>) {
    assert_eq!(outbox_count(state).await, 1, "expected exactly one iam.permission.changed row");
    let (reason, users_json): (String, String) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.payload.reason')) AS CHAR), \
                CAST(JSON_EXTRACT(envelope_json, '$.payload.user_ids') AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    ).fetch_one(&state.db).await.unwrap();
    let users: Vec<String> = serde_json::from_str(&users_json).unwrap();
    (reason, users.into_iter().collect())
}

async fn user_roles(state: &AppState, user_id: &str) -> BTreeSet<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT role_id FROM iam_user_roles WHERE user_id = ?"
    ).bind(user_id).fetch_all(&state.db).await.unwrap();
    rows.into_iter().map(|(r,)| r).collect()
}

async fn assert_epoch_fresh(ctx: &mut Ctx, uid: &str, before: i64) {
    let epoch = get_perm_epoch(&mut ctx.redis, PREFIX, uid).await.unwrap()
        .unwrap_or_else(|| panic!("perm_epoch missing for {uid}"));
    let after = chrono::Utc::now().timestamp();
    assert!(epoch >= before && epoch <= after, "epoch {epoch} for {uid} not in [{before}, {after}]");
    let key = perm_epoch_key(PREFIX, uid);
    let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut ctx.redis).await.unwrap();
    assert!(ttl > 0, "expected TTL > 0 for {key}, got {ttl}");
}

async fn assert_no_epoch(ctx: &mut Ctx, uid: &str) {
    let epoch = get_perm_epoch(&mut ctx.redis, PREFIX, uid).await.unwrap();
    assert_eq!(epoch, None, "expected no perm_epoch for {uid}");
}

fn set_of(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

// ============ cases ============

#[tokio::test]
async fn assign_user_roles_replaces_and_emits_event() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    seed_role(&ctx.state, "r2").await;
    seed_role(&ctx.state, "r3").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;

    let before = chrono::Utc::now().timestamp();
    let ret = RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec!["r1".into(), "r2".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    assert_eq!(user_roles(&ctx.state, "u1").await, set_of(&["r1", "r2"]));
    let ret_set: BTreeSet<String> = ret.into_iter().collect();
    assert_eq!(ret_set, set_of(&["r1", "r2"]));

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "user_roles_changed");
    assert_eq!(users, set_of(&["u1"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}

#[tokio::test]
async fn assign_user_roles_empty_clears_and_emits() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    seed_role(&ctx.state, "r2").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    assign_user_to_role(&ctx.state, "u1", "r2").await;

    let before = chrono::Utc::now().timestamp();
    RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec![], None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    assert_eq!(user_roles(&ctx.state, "u1").await, BTreeSet::new());
    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "user_roles_changed");
    assert_eq!(users, set_of(&["u1"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}

#[tokio::test]
async fn assign_user_roles_no_change_skips_write() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    seed_role(&ctx.state, "r2").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    assign_user_to_role(&ctx.state, "u1", "r2").await;

    // Reorder input; result should be identical set.
    RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec!["r2".into(), "r1".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    assert_eq!(user_roles(&ctx.state, "u1").await, set_of(&["r1", "r2"]));
    assert_eq!(outbox_count(&ctx.state).await, 0, "no-op must not enqueue");
    assert_no_epoch(&mut ctx, "u1").await;
}

#[tokio::test]
async fn assign_user_roles_rejects_invalid_role_id() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;

    let err = RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec!["r1".into(), "r_ghost".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap_err();
    match err {
        IamError::InvalidRoleIds(v) => assert_eq!(v, vec!["r_ghost".to_string()]),
        other => panic!("expected InvalidRoleIds, got {other:?}"),
    }

    assert_eq!(user_roles(&ctx.state, "u1").await, set_of(&["r1"]));
    assert_eq!(outbox_count(&ctx.state).await, 0);
    assert_no_epoch(&mut ctx, "u1").await;
}

#[tokio::test]
async fn assign_user_roles_rejects_disabled_role() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    seed_disabled_role(&ctx.state, "r2").await;

    let err = RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec!["r1".into(), "r2".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap_err();
    match err {
        IamError::InvalidRoleIds(v) => assert_eq!(v, vec!["r2".to_string()]),
        other => panic!("expected InvalidRoleIds, got {other:?}"),
    }
    assert_eq!(user_roles(&ctx.state, "u1").await, BTreeSet::new());
    assert_eq!(outbox_count(&ctx.state).await, 0);
    assert_no_epoch(&mut ctx, "u1").await;
}

#[tokio::test]
async fn assign_user_roles_user_not_found() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;

    let err = RoleService::new(ctx.state.clone())
        .assign_user_roles("ghost", vec!["r1".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap_err();
    assert!(matches!(err, IamError::UserNotFound), "got {err:?}");
    assert_eq!(outbox_count(&ctx.state).await, 0);
    assert_no_epoch(&mut ctx, "ghost").await;
}

#[tokio::test]
async fn assign_user_roles_dedups_input() {
    let mut ctx = setup().await;
    seed_user(&ctx.state, "u1").await;
    seed_role(&ctx.state, "r1").await;
    seed_role(&ctx.state, "r2").await;

    let before = chrono::Utc::now().timestamp();
    let ret = RoleService::new(ctx.state.clone())
        .assign_user_roles("u1", vec!["r1".into(), "r1".into(), "r2".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    assert_eq!(user_roles(&ctx.state, "u1").await, set_of(&["r1", "r2"]));
    let ret_set: BTreeSet<String> = ret.into_iter().collect();
    assert_eq!(ret_set, set_of(&["r1", "r2"]));
    assert_eq!(outbox_count(&ctx.state).await, 1);
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}
