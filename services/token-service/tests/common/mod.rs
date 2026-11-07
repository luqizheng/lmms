//! 集成测试公共 helper：起 MySQL 容器 + 跑 migrations
//!
//! 两条路径：
//! 1. 环境变量 `TEST_MYSQL_HOST` + `TEST_MYSQL_PORT` 存在时，连共享 MySQL，
//!    建独立 `test_<svc>_<pid>_<uuid>` database，跑 migrations。
//!    用于 `scripts/run-tests-docker.js` 的共享模式。
//! 2. 否则回退到本地 testcontainers 自己起一个 mysql 容器（保留 `cargo test` 单跑体验）。
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;
use token_service::db::DbPool;

/// 按序执行的迁移列表。直接复用生产代码的 `db::MIGRATIONS`，避免两份清单漂移。
///
/// 测试库每次都是新建的，所以这里无条件按序执行全部迁移，不需要 marker 判定
/// （生产路径的 marker 逻辑由 `tests/migration_marker_test.rs` 单独覆盖）。
pub use token_service::db::MIGRATIONS;

pub async fn spawn_mysql() -> DbPool {
    if let (Ok(host), Ok(port)) = (
        std::env::var("TEST_MYSQL_HOST"),
        std::env::var("TEST_MYSQL_PORT"),
    ) {
        return spawn_shared(host, port).await;
    }

    // 关闭 InnoDB native AIO，规避宿主 aio-max-nr 不足导致的 io_setup() EAGAIN
    let container = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .expect("start mysql");
    let host = container.get_host().await.expect("mysql host");
    let port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("mysql port");
    let url = format!("mysql://root@{}:{}/test", host, port);

    let pool = build_pool(&url).await;
    run_migrations(&pool).await;
    // 防止容器在函数返回后被 drop 回收，导致后续查询 PoolTimedOut
    std::mem::forget(container);
    DbPool::from(pool)
}

async fn spawn_shared(host: String, port: String) -> DbPool {
    let port: u16 = port.parse().expect("TEST_MYSQL_PORT must be u16");
    let db_name = format!(
        "test_{}_{}_{}",
        env!("CARGO_PKG_NAME"),
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );

    let admin_url = format!("mysql://root@{host}:{port}/");
    let admin = MySqlPool::connect(&admin_url)
        .await
        .expect("admin connect to shared mysql");
    sqlx::query(&format!(
        "CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4"
    ))
    .execute(&admin)
    .await
    .expect("create test database");
    admin.close().await;

    let url = format!("mysql://root@{host}:{port}/{db_name}");
    let pool = build_pool(&url).await;
    run_migrations(&pool).await;
    DbPool::from(pool)
}

async fn build_pool(url: &str) -> MySqlPool {
    MySqlPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET time_zone = '+00:00'")
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(url)
        .await
        .expect("connect mysql")
}

async fn run_migrations(pool: &MySqlPool) {
    for m in MIGRATIONS {
        // 先剥离整行注释，避免以 `--` 开头的语句被整条跳过导致建表缺失
        let cleaned = m
            .sql_mysql
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        for stmt in cleaned.split(';') {
            let s = stmt.trim();
            if s.is_empty() {
                continue;
            }
            sqlx::query(s).execute(pool).await.expect("run migration");
        }
    }
}

/// 测试用 PEM（与 workspace 的 libs/common-jwt fixtures 同源）。
pub const TEST_PRIVATE_PEM: &str =
    include_str!("../../../../libs/common-jwt/tests/fixtures/test_private.pem");
pub const TEST_PUBLIC_PEM: &str =
    include_str!("../../../../libs/common-jwt/tests/fixtures/test_public.pem");

/// 构造带真实 DB pool 的 TokenServerState（HTTP 级测试用，oneshot 房规）。
/// `user_service_url` 指向不可达地址 → claims 拉取走降级路径。
pub fn build_state(pool: DbPool) -> token_service::TokenServerState {
    let jwks = std::sync::Arc::new(
        digua_common_jwt::JWKSCache::from_pem(TEST_PUBLIC_PEM).expect("jwks from pem"),
    );
    token_service::TokenServerState {
        db: pool,
        issuer: std::sync::Arc::new(
            token_service::TokenIssuer::new(TEST_PRIVATE_PEM, "test-issuer").expect("issuer"),
        ),
        issuer_str: "test-issuer".into(),
        jwt_public_key: TEST_PUBLIC_PEM.into(),
        default_resource_ttl_sec: 3600,
        http_client: std::sync::Arc::new(digua_common_http::ResilientClient::new(
            reqwest::Client::new(),
            digua_common_http::ResilienceConfig::default(),
            std::sync::Arc::new(digua_common_http::NoopObserver),
        )),
        user_service_url: "http://iam-service:8081".into(),
        private_key_pem: TEST_PRIVATE_PEM.into(),
        bootstrap_tokens: vec![],
        bootstrap_owners: vec![],
        jwks,
    }
}
