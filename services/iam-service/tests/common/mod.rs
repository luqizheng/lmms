//! Integration test helper: 启动/连接 MySQL，建测试 schema。
//!
//! 两条路径：
//! 1. 环境变量 `TEST_MYSQL_HOST` + `TEST_MYSQL_PORT` 存在时，连共享 MySQL 容器，
//!    建独立 `test_<svc>_<pid>_<uuid>` database，跑 `iam_service::db::ensure_schema`。
//!    用于 `scripts/run-tests-docker.js` 的共享模式（一个 mysql 容器供所有 service）。
//! 2. 否则回退到本地 testcontainers 自己起一个 mysql 容器（保留 `cargo test` 单跑体验）。
use iam_service::db;
use sqlx::mysql::MySqlPool;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;

pub type MysqlContainer = testcontainers::ContainerAsync<Mysql>;

pub struct MysqlHandle {
    pub pool: MySqlPool,
    pub url: String,
    /// `None` 表示共享模式（容器由 `run-tests-docker.js` 管理生命周期）。
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
    sqlx::query(&format!("CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4"))
        .execute(&admin)
        .await
        .expect("create test database");
    admin.close().await;

    let url = format!("mysql://root@{host}:{port}/{db_name}");
    let pool = common::db::connect(&url, 5)
        .await
        .expect("connect to test database");
    db::ensure_schema(&pool).await.expect("ensure iam schema");

    MysqlHandle {
        pool,
        url,
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
    let pool = common::db::connect(&url, 5)
        .await
        .expect("connect local mysql");
    db::ensure_schema(&pool).await.expect("ensure iam schema");

    MysqlHandle {
        pool,
        url,
        container: Some(c),
    }
}