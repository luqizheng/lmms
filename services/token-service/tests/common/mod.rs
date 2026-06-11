//! 集成测试公共 helper：起 MySQL 容器 + 跑 migrations
use sqlx::mysql::{MySqlPool, MySqlPoolOptions};
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

pub const MIGRATION_SQL: &str = include_str!("../../migrations/20260611000001_init.sql");

pub async fn spawn_mysql() -> MySqlPool {
    let container = Mysql::default().start().await.expect("start mysql");
    let host = container.get_host().await.expect("mysql host");
    let port = container
        .get_host_port_ipv4(3306)
        .await
        .expect("mysql port");
    let url = format!("mysql://root:root@{}:{}/test", host, port);

    let pool = MySqlPoolOptions::new()
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
        .connect(&url)
        .await
        .expect("connect mysql");

    for stmt in MIGRATION_SQL.split(';') {
        let s = stmt.trim();
        if s.is_empty() || s.starts_with("--") {
            continue;
        }
        sqlx::query(s)
            .execute(&pool)
            .await
            .expect("run migration");
    }
    pool
}
