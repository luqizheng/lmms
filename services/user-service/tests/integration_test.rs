//! Integration test using testcontainers-mysql.
//! Run with: `cargo test -p user-service --test integration_test -- --test-threads=1`
//! Requires Docker running on host.

use sqlx::mysql::MySqlPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::mysql::Mysql;

async fn setup_pool() -> (ContainerAsync<Mysql>, MySqlPool) {
    let container = Mysql::default().start().await.expect("start mysql");
    let host = container.get_host().await.expect("host");
    let port = container.get_host_port_ipv4(3306).await;
    let url = format!("mysql://root:secret@{host}:{port}/mysql");
    let pool = MySqlPool::connect(&url).await.expect("connect");
    // Apply migrations
    let _ = sqlx::query(include_str!(
        "../../migrations/2026_06_10_add_user_code.sql"
    ))
    .execute(&pool)
    .await;
    (container, pool)
}

#[tokio::test]
async fn create_and_fetch_user() {
    let (_c, pool) = setup_pool().await;
    sqlx::query("INSERT INTO users (username, user_code) VALUES (?, ?)")
        .bind("alice")
        .bind("USR-001")
        .execute(&pool)
        .await
        .unwrap();
    let row: (String,) = sqlx::query_as("SELECT username FROM users WHERE user_code = ?")
        .bind("USR-001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.0, "alice");
}

#[tokio::test]
async fn role_create_and_assign() {
    let (_c, pool) = setup_pool().await;
    sqlx::query("INSERT INTO roles (code, name) VALUES (?, ?)")
        .bind("admin")
        .bind("Admin")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (username, user_code) VALUES (?, ?)")
        .bind("bob")
        .bind("USR-002")
        .execute(&pool)
        .await
        .unwrap();
    let role_id: (u64,) = sqlx::query_as("SELECT id FROM roles WHERE code = ?")
        .bind("admin")
        .fetch_one(&pool)
        .await
        .unwrap();
    let user_id: (u64,) = sqlx::query_as("SELECT id FROM users WHERE user_code = ?")
        .bind("USR-002")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_roles (user_id, role_id) VALUES (?, ?)")
        .bind(user_id.0)
        .bind(role_id.0)
        .execute(&pool)
        .await
        .unwrap();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM user_roles WHERE user_id = ?")
        .bind(user_id.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1);
}

mod multi_db {
    use user_service::config::{Config, DbType};

    #[test]
    fn dameng_yaml_parses() {
        let yaml = r#"
server: { host: "0.0.0.0", port: 8082 }
database:
  db_type: dameng
  url: "Driver={DM8 ODBC DRIVER};Server=127.0.0.1;Port=5236;UID=SYSDBA;PWD=x;Schema=DMHR"
  max_connections: 1
redis: { url: "redis://localhost:6379" }
auth: { service_token_issuer: "x", service_token_audience: "y" }
"#;
        let cfg: Config = serde_yaml::from_str(yaml).expect("dameng cfg parse");
        assert_eq!(cfg.database.db_type, DbType::Dameng);
    }

    #[test]
    fn unsupported_db_response_is_501() {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;
        use user_service::error::AppError;

        let resp = AppError::UnsupportedDb(DbType::Dameng).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
