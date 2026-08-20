//! Integration test helper: spawn MySQL container, create test database, ensure schema.
//!
//! Two paths:
//! 1. With TEST_MYSQL_HOST + TEST_MYSQL_PORT env vars, connect to shared MySQL container,
//!    create a unique `test_<pkg>_<pid>_<uuid>` database, run config-service ensure_schema.
//! 2. Otherwise, fall back to a local testcontainers MySQL container (single `cargo test`).
use config_service::db;
use sqlx::MySqlPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;

pub type MysqlContainer = testcontainers::ContainerAsync<Mysql>;

pub struct MysqlHandle {
    pub pool: MySqlPool,
    /// Kept alive (drops → container killed) for local-container mode;
    /// `None` means shared mode (container lifecycle managed by run-tests-docker.js).
    #[allow(dead_code)]
    pub container: Option<MysqlContainer>,
}

pub async fn spawn_mysql() -> MysqlHandle {
    match (
        std::env::var("TEST_MYSQL_HOST").ok(),
        std::env::var("TEST_MYSQL_PORT").ok(),
    ) {
        (Some(host), Some(port)) => spawn_shared(host, port).await,
        _ => spawn_local().await,
    }
}

async fn spawn_shared(host: String, port: String) -> MysqlHandle {
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
    let pool = MySqlPool::connect(&url)
        .await
        .expect("connect test database");
    db::ensure_schema(&pool).await.expect("ensure cfg schema");

    MysqlHandle {
        pool,
        container: None,
    }
}

async fn spawn_local() -> MysqlHandle {
    let c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .expect("start local mysql container");
    let host = c.get_host().await.expect("mysql host");
    let port = c.get_host_port_ipv4(3306).await.expect("mysql port");
    let url = format!("mysql://root@{host}:{port}/test");
    let pool = MySqlPool::connect(&url).await.expect("connect local mysql");
    db::ensure_schema(&pool).await.expect("ensure cfg schema");

    MysqlHandle {
        pool,
        container: Some(c),
    }
}
