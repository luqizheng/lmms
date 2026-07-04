//! Phase 8.1 集成测试：`POST /iam/tenants` 自助注册流程。
//!
//! 启 MySQL testcontainer（仅依赖 iam-service 自动迁移 V001 → V003），
//! 通过 `TenantService::register` 驱动业务事务，断言：
//!   1) 注册成功 → tenant 行 + admin user 行 + outbox iam.tenant.created 行
//!   2) slug 唯一 → 重复注册 DuplicateTenant
//!   3) `(tenant_id, username)` unique → 双 tenant admin 同名不冲突
//!   4) outbox envelope.tenant_id 正确填值（i64.to_string()）

use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::db;
use iam_service::dto::CreateTenantRequest;
use iam_service::error::IamError;
use iam_service::service::TenantService;
use iam_service::state::AppState;
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers_modules::{mysql::Mysql, testcontainers::runners::AsyncRunner};

const PREFIX: &str = "superpowers:";

struct Ctx {
    state: AppState,
    _mysql: testcontainers::ContainerAsync<Mysql>,
}

async fn setup() -> Ctx {
    let mysql_c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .unwrap();
    let my_port = mysql_c.get_host_port_ipv4(3306).await.unwrap();
    let my_host = mysql_c.get_host().await.unwrap();
    let url = format!("mysql://root@{my_host}:{my_port}/test");
    let pool = common::db::connect(&url, 5).await.unwrap();
    db::ensure_schema(&pool).await.unwrap();

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
            url: "redis://127.0.0.1:0".into(),
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
    let state = AppState::new(pool, cache, config, None, outbox_repo);
    Ctx {
        state,
        _mysql: mysql_c,
    }
}

fn sample_req(slug: &str, admin_username: &str) -> CreateTenantRequest {
    CreateTenantRequest {
        tenant_slug: slug.to_string(),
        display_name: format!("{slug} display"),
        admin_username: admin_username.to_string(),
        admin_password: "secret-strong-pw-123".to_string(),
        admin_email: None,
    }
}

#[tokio::test]
async fn register_tenant_creates_tenant_and_admin_and_outbox() {
    let ctx = setup().await;
    let svc = TenantService::new(ctx.state.clone());

    let resp = svc.register(sample_req("acme", "admin")).await.unwrap();
    assert!(resp.tenant_id > 1, "tenant_id must be > 1 (default=1 reserved)");
    assert!(!resp.admin_user_id.is_empty());

    // tenant row
    let (slug,): (String,) = sqlx::query_as("SELECT slug FROM iam_tenants WHERE id = ?")
        .bind(resp.tenant_id as u64)
        .fetch_one(&ctx.state.db)
        .await
        .unwrap();
    assert_eq!(slug, "acme");

    // admin user row with tenant_id
    let (tid, username): (u64, String) = sqlx::query_as(
        "SELECT tenant_id, username FROM iam_users WHERE id = ?",
    )
    .bind(&resp.admin_user_id)
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(tid as i64, resp.tenant_id);
    assert_eq!(username, "admin");

    // outbox row
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_outbox_events WHERE event_type = 'iam.tenant.created'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(n, 1);

    // envelope.tenant_id = tenant_id.to_string()
    let (env_tenant,): (String,) = sqlx::query_as(
        "SELECT CAST(JSON_UNQUOTE(JSON_EXTRACT(envelope_json, '$.tenant_id')) AS CHAR) \
         FROM iam_outbox_events WHERE event_type = 'iam.tenant.created'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(env_tenant, resp.tenant_id.to_string());
}

#[tokio::test]
async fn register_tenant_duplicate_slug_returns_conflict() {
    let ctx = setup().await;
    let svc = TenantService::new(ctx.state.clone());

    svc.register(sample_req("acme", "admin")).await.unwrap();

    let err = svc
        .register(sample_req("acme", "another"))
        .await
        .expect_err("duplicate slug must error");
    assert!(matches!(err, IamError::DuplicateTenant), "got: {err:?}");
}

#[tokio::test]
async fn two_tenants_can_have_same_admin_username() {
    let ctx = setup().await;
    let svc = TenantService::new(ctx.state.clone());

    let acme = svc.register(sample_req("acme", "admin")).await.unwrap();
    let globex = svc.register(sample_req("globex", "admin")).await.unwrap();

    assert_ne!(acme.tenant_id, globex.tenant_id);
    assert_ne!(acme.admin_user_id, globex.admin_user_id);

    // Both admin rows exist with same username but different tenant_id.
    let admins: Vec<(String, u64, String)> = sqlx::query_as(
        "SELECT id, tenant_id, username FROM iam_users WHERE username = 'admin' ORDER BY tenant_id ASC",
    )
    .fetch_all(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(admins.len(), 2);
    assert_eq!(admins[0].2, "admin");
    assert_eq!(admins[1].2, "admin");
    assert!(admins[0].1 < admins[1].1);
}

#[tokio::test]
async fn v004_migration_adds_tenant_id_and_rescopes_seed_role() {
    let ctx = setup().await;
    // iam_roles.tenant_id column exists
    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = 'iam_roles' AND column_name = 'tenant_id'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(cnt, 1, "iam_roles.tenant_id must exist after V004");

    // iam_org_units.tenant_id column exists
    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = 'iam_org_units' AND column_name = 'tenant_id'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(cnt, 1, "iam_org_units.tenant_id must exist after V004");

    // iam_login_logs.tenant_id column exists
    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = 'iam_login_logs' AND column_name = 'tenant_id'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(cnt, 1, "iam_login_logs.tenant_id must exist after V004");

    // seed tenant_admin role scoped to default tenant
    let (tid,): (i64,) = sqlx::query_as(
        "SELECT CAST(tenant_id AS SIGNED) FROM iam_roles WHERE code = 'tenant_admin'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(tid, 1, "seed tenant_admin must be scoped to default tenant");

    // seed role got all enabled permissions
    let (pn,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_role_permissions rp \
         JOIN iam_permissions p ON p.id = rp.permission_id \
         WHERE rp.role_id = 'role-tenant-admin-seed' \
         AND p.status = 'enabled' AND p.deleted_at IS NULL",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(pn, total, "seed tenant_admin must have all enabled permissions");

    // unique key rebuilt: uk_tenant_role_code (tenant_id, code) present, old code index gone
    let (uk_cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.statistics \
         WHERE table_schema = DATABASE() AND table_name = 'iam_roles' AND index_name = 'uk_tenant_role_code'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(uk_cnt, 2, "uk_tenant_role_code must be a composite (tenant_id, code) index");

    let (old_code_cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM information_schema.statistics \
         WHERE table_schema = DATABASE() AND table_name = 'iam_roles' AND index_name = 'code'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(old_code_cnt, 0, "old single-column 'code' unique index must be dropped");
}

#[tokio::test]
async fn register_creates_per_tenant_admin_role_with_permissions() {
    let ctx = setup().await;
    let svc = TenantService::new(ctx.state.clone());
    let resp = svc
        .register(sample_req("acme", "admin"))
        .await
        .unwrap();

    // per-tenant tenant_admin role exists
    let (role_id, tid): (String, i64) = sqlx::query_as(
        "SELECT id, CAST(tenant_id AS SIGNED) FROM iam_roles WHERE tenant_id = ? AND code = 'tenant_admin'",
    )
    .bind(resp.tenant_id as u64)
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(tid, resp.tenant_id);

    // role has all enabled global permissions
    let (pn,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_role_permissions WHERE role_id = ?",
    )
    .bind(&role_id)
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_permissions WHERE status = 'enabled' AND deleted_at IS NULL",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(pn, total, "per-tenant tenant_admin must have all enabled perms");

    // admin user bound to the per-tenant role
    let (bound,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM iam_user_roles WHERE user_id = ? AND role_id = ?",
    )
    .bind(&resp.admin_user_id)
    .bind(&role_id)
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    assert_eq!(bound, 1, "admin must be bound to per-tenant tenant_admin role");
}
