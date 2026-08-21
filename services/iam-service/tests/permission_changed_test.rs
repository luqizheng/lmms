//! Phase 4.4 集成测试：iam 在权限变更时 (1) 同事务写 `iam.permission.changed`
//! outbox 行，(2) commit 后 best-effort 直写 Redis `perm_epoch:{uid}`。
//!
//! 每个用例独立起 MySQL + Redis testcontainer，建 iam schema，通过
//! RoleService / PermissionService 驱动，断言 outbox 行（reason / user_ids）
//! 与 Redis perm_epoch（值≈now、TTL>0）。
//!
//! 注意：多 MySQL 容器并发易 StartupTimeout，用 `--test-threads=1` 串行。
mod common;

use digua_common_cache::MemoryCache;
use digua_common_crypto::password::hash as hash_password;
use digua_common_event::perm_epoch::{get_perm_epoch, perm_epoch_key};
use digua_common_event::MySqlOutboxRepo;
use digua_jwt_ctx::TenantScope;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::dto::LoginRequest;
use iam_service::service::token_forward::TokenForwardClient;
use iam_service::service::{AuthService, PermissionService, RoleService};
use iam_service::state::AppState;
use std::collections::BTreeSet;
use std::sync::Arc;
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

    let redis_c = Redis.start().await.unwrap();
    let r_port = redis_c.get_host_port_ipv4(6379).await.unwrap();
    let r_host = redis_c.get_host().await.unwrap();
    let client = redis::Client::open(format!("redis://{r_host}:{r_port}")).unwrap();
    let mgr = redis::aio::ConnectionManager::new(client).await.unwrap();

    let config = Config {
        app: AppConfig {
            name: "iam-service".into(),
            host: "0.0.0.0".into(),
            port: 8080,
        },
        database: DatabaseConfig {
            url: url.clone(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: RedisConfig {
            url: format!("redis://{r_host}:{r_port}"),
            ttl_sec: 86400,
            prefix: PREFIX.into(),
        },
        jwt: JwtConfig {
            private_key_path: "/app/keys/private.pem".into(),
            public_key_path: "/app/keys/public.pem".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig {
            host: "consul".into(),
            port: 8500,
            service_name: "iam-service".into(),
        },
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
    let (server_uri, _) = common::stub_token_service("u_tenant2", 2, false).await;
    let service_jwt = std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: server_uri.clone(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let token_forward = Arc::new(TokenForwardClient::new(
        service_jwt.clone(),
        server_uri.clone(),
    ));
    let state = AppState::new(
        pool,
        cache,
        config,
        Some(mgr.clone()),
        outbox_repo,
        jwks,
        service_jwt,
        token_forward,
    );
    Ctx {
        state,
        redis: mgr,
        _mysql: mysql_h.container,
        _redis: redis_c,
    }
}

// ---- seed helpers ----

async fn seed_user(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_users (id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES (?, ?, 'h', 's', ?, 'active', NOW(), NOW())",
    )
    .bind(id)
    .bind(id)
    .bind(id)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn seed_role(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'self', 'active', NOW(), NOW())",
    )
    .bind(id)
    .bind(id)
    .bind(id)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn seed_tenant_role(state: &AppState, tenant: i64, id: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, tenant_id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 'self', 'active', NOW(), NOW())",
    )
    .bind(id).bind(tenant).bind(id).bind(id)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn seed_permission(state: &AppState, id: &str) {
    sqlx::query(
        "INSERT INTO iam_permissions (id, code, name, category, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'api', 'enabled', NOW(), NOW())",
    )
    .bind(id)
    .bind(id)
    .bind(id)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn assign_user_to_role(state: &AppState, user_id: &str, role_id: &str) {
    sqlx::query("INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())")
        .bind(user_id)
        .bind(role_id)
        .execute(&state.db)
        .await
        .unwrap();
}

async fn seed_tenant(state: &AppState, id: i64, slug: &str) {
    sqlx::query(
        "INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'active', NOW(), NOW()) \
         ON DUPLICATE KEY UPDATE id = id",
    )
    .bind(id)
    .bind(slug)
    .bind(slug)
    .execute(&state.db)
    .await
    .unwrap();
}

async fn seed_tenant_user(state: &AppState, tenant: i64, uid: &str, password_hash: &str) {
    sqlx::query(
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 's', ?, 'active', NOW(), NOW())",
    )
    .bind(uid).bind(tenant).bind(uid).bind(password_hash).bind(uid)
    .execute(&state.db).await.unwrap();
}

async fn bind_permission_to_role(state: &AppState, role_id: &str, permission_id: &str) {
    sqlx::query(
        "INSERT INTO iam_role_permissions (role_id, permission_id, created_at) VALUES (?, ?, NOW())",
    )
    .bind(role_id)
    .bind(permission_id)
    .execute(&state.db)
    .await
    .unwrap();
}

// ---- outbox assertions ----

async fn outbox_count(state: &AppState) -> i64 {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    n
}

/// 唯一一条 iam.permission.changed 行的 (reason, user_ids-set)。断言恰好一行。
async fn single_outbox_reason_and_users(state: &AppState) -> (String, BTreeSet<String>) {
    assert_eq!(
        outbox_count(state).await,
        1,
        "expected exactly one iam.permission.changed outbox row"
    );
    let (reason, users_json): (String, String) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.payload.reason')) AS CHAR), \
                CAST(JSON_EXTRACT(envelope_json, '$.payload.user_ids') AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let users: Vec<String> = serde_json::from_str(&users_json).unwrap();
    (reason, users.into_iter().collect())
}

// ---- redis assertions ----

async fn assert_epoch_fresh(ctx: &mut Ctx, uid: &str, before: i64) {
    let epoch = get_perm_epoch(&mut ctx.redis, PREFIX, uid)
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("perm_epoch missing for {uid}"));
    let after = chrono::Utc::now().timestamp();
    assert!(
        epoch >= before && epoch <= after,
        "epoch {epoch} for {uid} not in [{before}, {after}]"
    );
    // TTL > 0
    let key = perm_epoch_key(PREFIX, uid);
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut ctx.redis)
        .await
        .unwrap();
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

// assign_permissions on a role with N users → one outbox row (reason
// role_permissions_changed, user_ids = those N) + epoch stamped for each.
#[tokio::test]
async fn assign_permissions_emits_event_and_stamps_epochs() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    seed_user(&ctx.state, "u2").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    assign_user_to_role(&ctx.state, "u2", "r1").await;
    seed_permission(&ctx.state, "p1").await;
    seed_permission(&ctx.state, "p2").await;

    let before = chrono::Utc::now().timestamp();
    RoleService::new(ctx.state.clone())
        .assign_permissions(
            "r1",
            vec!["p1".into(), "p2".into()],
            None,
            &TenantScope::Single(1i64),
        )
        .await
        .unwrap();

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "role_permissions_changed");
    assert_eq!(users, set_of(&["u1", "u2"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
    assert_epoch_fresh(&mut ctx, "u2", before).await;
}

// role delete → reason role_deleted, user_ids expanded.
#[tokio::test]
async fn role_delete_emits_role_deleted() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;

    let before = chrono::Utc::now().timestamp();
    RoleService::new(ctx.state.clone())
        .delete("r1", None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "role_deleted");
    assert_eq!(users, set_of(&["u1"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}

// role update status → disabled → reason role_disabled.
#[tokio::test]
async fn role_disable_emits_role_disabled() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;

    let before = chrono::Utc::now().timestamp();
    let req = iam_service::dto::UpdateRoleRequest {
        name: None,
        description: None,
        data_scope: None,
        status: Some("disabled".into()),
    };
    RoleService::new(ctx.state.clone())
        .update("r1", req, None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "role_disabled");
    assert_eq!(users, set_of(&["u1"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}

// permission delete → reason permission_deleted, user_ids via role_permissions JOIN.
#[tokio::test]
async fn permission_delete_emits_permission_deleted() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    seed_user(&ctx.state, "u2").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    assign_user_to_role(&ctx.state, "u2", "r1").await;
    seed_permission(&ctx.state, "p1").await;
    bind_permission_to_role(&ctx.state, "r1", "p1").await;

    let before = chrono::Utc::now().timestamp();
    PermissionService::new(ctx.state.clone())
        .delete("p1", None)
        .await
        .unwrap();

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "permission_deleted");
    assert_eq!(users, set_of(&["u1", "u2"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
    assert_epoch_fresh(&mut ctx, "u2", before).await;
}

// permission update status → disabled → reason permission_disabled.
#[tokio::test]
async fn permission_disable_emits_permission_disabled() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    seed_permission(&ctx.state, "p1").await;
    bind_permission_to_role(&ctx.state, "r1", "p1").await;

    let before = chrono::Utc::now().timestamp();
    let req = iam_service::dto::UpdatePermissionRequest {
        name: None,
        description: None,
        sort_order: None,
        status: Some("disabled".into()),
    };
    PermissionService::new(ctx.state.clone())
        .update("p1", req, None)
        .await
        .unwrap();

    let (reason, users) = single_outbox_reason_and_users(&ctx.state).await;
    assert_eq!(reason, "permission_disabled");
    assert_eq!(users, set_of(&["u1"]));
    assert_epoch_fresh(&mut ctx, "u1", before).await;
}

// empty case: role with no users → NO outbox row, NO epoch write.
#[tokio::test]
async fn assign_permissions_with_no_users_emits_nothing() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_permission(&ctx.state, "p1").await;

    RoleService::new(ctx.state.clone())
        .assign_permissions("r1", vec!["p1".into()], None, &TenantScope::Single(1i64))
        .await
        .unwrap();

    assert_eq!(
        outbox_count(&ctx.state).await,
        0,
        "no outbox row for empty user set"
    );
    // role_permissions still applied (assignment itself succeeds)
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM iam_role_permissions WHERE role_id = 'r1'")
            .fetch_one(&ctx.state.db)
            .await
            .unwrap();
    assert_eq!(n, 1);
    assert_no_epoch(&mut ctx, "u1").await;
}

// transaction rollback: duplicate permission_id violates the role_permissions
// PK mid-tx → assign_permissions fails → role_permissions unchanged + no outbox row.
#[tokio::test]
async fn assign_permissions_rolls_back_on_pk_violation() {
    let mut ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_user(&ctx.state, "u1").await;
    assign_user_to_role(&ctx.state, "u1", "r1").await;
    seed_permission(&ctx.state, "p1").await;
    seed_permission(&ctx.state, "p_existing").await;
    // pre-existing binding that must survive a rolled-back assign
    bind_permission_to_role(&ctx.state, "r1", "p_existing").await;

    // duplicate p1 → second INSERT hits PK (role_id, permission_id)
    let res = RoleService::new(ctx.state.clone())
        .assign_permissions(
            "r1",
            vec!["p1".into(), "p1".into()],
            None,
            &TenantScope::Single(1i64),
        )
        .await;
    assert!(res.is_err(), "duplicate permission_id must fail the tx");

    // role_permissions unchanged: only the original p_existing remains
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT permission_id FROM iam_role_permissions WHERE role_id = 'r1'")
            .fetch_all(&ctx.state.db)
            .await
            .unwrap();
    assert_eq!(
        rows.into_iter().collect::<BTreeSet<_>>(),
        set_of(&["p_existing"]),
        "rolled-back tx must leave role_permissions untouched"
    );
    assert_eq!(
        outbox_count(&ctx.state).await,
        0,
        "no outbox row on rollback"
    );
    assert_no_epoch(&mut ctx, "u1").await;
}

// ---- 4.4-fu2 ----

#[tokio::test]
async fn permission_changed_event_carries_actor_id() {
    let ctx = setup().await;
    // Seed: admin + role + a target user under that role.
    // The admin user_id = "admin_actor".
    seed_user(&ctx.state, "admin_actor").await;
    seed_user(&ctx.state, "target").await;
    seed_role(&ctx.state, "r1").await;
    assign_user_to_role(&ctx.state, "target", "r1").await;
    seed_permission(&ctx.state, "p1").await;

    // act: assign p1 to r1 (this is the path that emits iam.permission.changed)
    RoleService::new(ctx.state.clone())
        .assign_permissions(
            "r1",
            vec!["p1".into()],
            Some("admin_actor"),
            &TenantScope::Single(1i64),
        )
        .await
        .unwrap();

    // assert: exactly one outbox row, with actor_id set in payload.
    assert_eq!(outbox_count(&ctx.state).await, 1);
    let (actor,): (Option<String>,) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.payload.actor_id')) AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    // mysql JSON_UNQUOTE on JSON null returns the string "null"; distinguish via the raw SELECT.
    let raw: (serde_json::Value,) = sqlx::query_as(
        "SELECT JSON_EXTRACT(envelope_json, '$.payload.actor_id') \
         FROM iam_outbox_events WHERE event_type = 'iam.permission.changed'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    let _ = actor; // suppress unused warning if first call returns "null"
    assert_eq!(raw.0, serde_json::json!("admin_actor"));
}

#[tokio::test]
async fn permission_changed_event_carries_real_tenant_id() {
    let ctx = setup().await;
    seed_tenant(&ctx.state, 2, "tenant-2").await;
    let hash = hash_password("password").unwrap();
    seed_tenant_user(&ctx.state, 2, "u_tenant2", &hash).await;
    seed_tenant_role(&ctx.state, 2, "r1").await;
    assign_user_to_role(&ctx.state, "u_tenant2", "r1").await;
    seed_permission(&ctx.state, "p1").await;

    RoleService::new(ctx.state.clone())
        .assign_permissions("r1", vec!["p1".into()], None, &TenantScope::Single(2i64))
        .await
        .unwrap();

    assert_eq!(outbox_count(&ctx.state).await, 1);
    let (env_tenant,): (String,) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.tenant_id')) AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.permission.changed' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(
        env_tenant, "2",
        "permission.changed envelope must carry real tenant_id"
    );
}

#[tokio::test]
async fn login_succeeded_event_carries_real_tenant_id() {
    let ctx = setup().await;
    seed_tenant(&ctx.state, 2, "tenant-2").await;
    let hash = hash_password("password").unwrap();
    seed_tenant_user(&ctx.state, 2, "u_tenant2", &hash).await;

    let auth = AuthService::new(ctx.state.clone());
    let req = LoginRequest {
        username: "u_tenant2".into(),
        password: "password".into(),
        tenant_slug: Some("tenant-2".into()),
    };
    auth.login(req, "127.0.0.1", "test-agent").await.unwrap();

    let (env_tenant,): (String,) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.tenant_id')) AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.login.succeeded' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(
        env_tenant, "2",
        "login.succeeded envelope must carry real tenant_id"
    );
}
