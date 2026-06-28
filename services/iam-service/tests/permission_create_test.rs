//! Phase 4.4-fu4 integration test: covers `PermissionService::create` —
//! the INSERT...RETURNING MySQL-8.0 incompatibility that 4.4 task 8 missed.
//!
//! Same testcontainer pattern as `permission_changed_test.rs` (4.4 task 8):
//! real MySQL via testcontainers, apply iam schema, drive
//! `PermissionService::create`, assert the round-trip succeeds and the
//! returned row matches.
//!
//! Multi-MySQL-container concurrent startup is flaky; run with
//! `--test-threads=1` (the existing permission_changed_test.rs shares the
//! same constraint — see its module-level note).
//!
//! Critical: the `_mysql` container field MUST be held for the whole test
//! lifetime; dropping it kills MySQL and every pooled connection goes stale
//! (sqlx returns PoolTimedOut instead of reconnecting). This is the same
//! gotcha that `permission_changed_test.rs` works around with `Ctx`.

use common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::db;
use iam_service::dto::permission::CreatePermissionRequest;
use iam_service::service::permission_svc::PermissionService;
use iam_service::state::AppState;
use common_cache::MemoryCache;
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
        app: AppConfig {
            name: "iam-service".into(),
            host: "0.0.0.0".into(),
            port: 8080,
        },
        database: DatabaseConfig {
            url: "unused".into(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: RedisConfig {
            url: "redis://127.0.0.1:6379".into(),
            ttl_sec: 86400,
            prefix: "superpowers:".into(),
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
    // pass `None` for redis mgr — create path doesn't touch perm_epoch.
    let state = AppState::new(pool, cache, config, None, outbox_repo);
    Ctx {
        state,
        _mysql: mysql_c,
    }
}

#[tokio::test]
async fn create_permission_round_trip() {
    let ctx = setup().await;
    let svc = PermissionService::new(ctx.state.clone());

    let req = CreatePermissionRequest {
        code: format!("test.perm.{}", uuid::Uuid::new_v4()),
        name: "Test permission".to_string(),
        description: Some("Phase 4.4-fu4 round-trip".to_string()),
        category: 0,
        resource: Some("test".to_string()),
        action: Some("read".to_string()),
        sort_order: Some(0),
    };

    let resp = svc.create(req).await.expect("create should succeed under MySQL 8.0");
    assert!(!resp.id.is_empty(), "id must be populated");
    assert_eq!(resp.status, "enabled");
    assert_eq!(resp.name, "Test permission");
    assert!(resp.code.starts_with("test.perm."));
}