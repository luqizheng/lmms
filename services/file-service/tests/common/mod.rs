//! Integration test helper: spawn MySQL container, create test database, ensure schema.
//!
//! Two paths:
//! 1. With TEST_MYSQL_HOST + TEST_MYSQL_PORT env vars, connect to shared MySQL container,
//!    create a unique `test_<pkg>_<pid>_<uuid>` database, run file-service ensure_schema.
//! 2. Otherwise, fall back to a local testcontainers MySQL container (single `cargo test`).
use file_service::db;
use sqlx::mysql::MySqlPool;
use std::sync::Arc;
use tempfile::TempDir;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;

pub type MysqlContainer = testcontainers::ContainerAsync<Mysql>;

pub struct MysqlHandle {
    pub pool: MySqlPool,
    pub storage: Arc<file_service::storage::LocalStorage>,
    /// Kept alive (drops → container killed) for local-container mode;
    /// `None` means shared mode (container lifecycle managed by run-tests-docker.js).
    #[allow(dead_code)]
    pub container: Option<MysqlContainer>,
}

pub async fn spawn_mysql() -> MysqlHandle {
    let storage_dir = tempfile::tempdir().expect("tempdir");
    let storage = Arc::new(file_service::storage::LocalStorage::new(
        storage_dir.path().to_str().unwrap(),
    ));

    match (
        std::env::var("TEST_MYSQL_HOST").ok(),
        std::env::var("TEST_MYSQL_PORT").ok(),
    ) {
        (Some(host), Some(port)) => spawn_shared(host, port, storage, storage_dir).await,
        _ => spawn_local(storage, storage_dir).await,
    }
}

async fn spawn_shared(
    host: String,
    port: String,
    storage: Arc<file_service::storage::LocalStorage>,
    _storage_dir: TempDir,
) -> MysqlHandle {
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
    // ensure_schema 吃 DbPool（方言无关入口）；MySQL URL 建的就是 MySQL 池，
    // 取回底层 MySqlPool 让既有断言（MySQL 方言 SQL）原样可用。
    let dbpool = digua_common_db::init_pool_with(&url, 5)
        .await
        .expect("connect to test database");
    db::ensure_schema(&dbpool)
        .await
        .expect("ensure file schema");
    let pool = dbpool.as_mysql().cloned().expect("mysql pool from DbPool");

    MysqlHandle {
        pool,
        storage,
        container: None,
    }
}

async fn spawn_local(
    storage: Arc<file_service::storage::LocalStorage>,
    _storage_dir: TempDir,
) -> MysqlHandle {
    let c = Mysql::default()
        .with_cmd(["--innodb-use-native-aio=0"])
        .start()
        .await
        .expect("start local mysql container");
    let host = c.get_host().await.expect("mysql host");
    let port = c.get_host_port_ipv4(3306).await.expect("mysql port");
    let url = format!("mysql://root@{host}:{port}/test");
    let dbpool = digua_common_db::init_pool_with(&url, 5)
        .await
        .expect("connect local mysql");
    db::ensure_schema(&dbpool)
        .await
        .expect("ensure file schema");
    let pool = dbpool.as_mysql().cloned().expect("mysql pool from DbPool");

    MysqlHandle {
        pool,
        storage,
        container: Some(c),
    }
}
