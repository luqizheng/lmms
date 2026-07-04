//! Phase 8.2: cross-tenant isolation for iam business tables (roles / orgs / users).
use iam_service::service::RoleService;
use iam_service::dto::{CreateRoleRequest, RoleQuery};
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::db;
use iam_service::state::AppState;
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers_modules::{mysql::Mysql, testcontainers::runners::AsyncRunner};

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
        app: AppConfig { name: "iam-service".into(), host: "0.0.0.0".into(), port: 8080 },
        database: DatabaseConfig { url: url.clone(), max_connections: 5, min_connections: 1 },
        redis: RedisConfig { url: "redis://x".into(), ttl_sec: 86400, prefix: "test:".into() },
        jwt: JwtConfig {
            private_key_path: "/dev/null".into(),
            public_key_path: "/dev/null".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig { host: "consul".into(), port: 8500, service_name: "iam-service".into() },
    };
    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let state = AppState::new(pool, cache, config, None, outbox_repo);

    Ctx { state, _mysql: mysql_c }
}

async fn seed_tenant(state: &AppState, id: i64, slug: &str) {
    sqlx::query(
        "INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'active', NOW(), NOW()) \
         ON DUPLICATE KEY UPDATE id = id",
    )
    .bind(id).bind(slug).bind(slug)
    .execute(&state.db).await.unwrap();
}

#[tokio::test]
async fn roles_are_tenant_isolated() {
    let ctx = setup().await;
    let svc = RoleService::new(ctx.state.clone());

    // seed tenant 2
    seed_tenant(&ctx.state, 2, "tenant-2").await;

    // create role in tenant 2
    let created = svc
        .create(
            CreateRoleRequest {
                name: "Acme Admin".into(),
                code: "acme-admin".into(),
                description: None,
                data_scope: 3,
            },
            2,
        )
        .await
        .unwrap();

    // tenant 2 list sees it
    let list_t2 = svc.list(RoleQuery { page: 1, size: 50, keyword: None, status: None }, 2).await.unwrap();
    assert!(list_t2.items.iter().any(|r| r.id == created.id));

    // tenant 3 list does NOT see it
    let list_t3 = svc.list(RoleQuery { page: 1, size: 50, keyword: None, status: None }, 3).await.unwrap();
    assert!(!list_t3.items.iter().any(|r| r.id == created.id), "cross-tenant role leak");

    // tenant 3 get by id -> RoleNotFound
    let err = svc.get(&created.id, 3).await.unwrap_err();
    assert!(matches!(err, iam_service::error::IamError::RoleNotFound));
}
