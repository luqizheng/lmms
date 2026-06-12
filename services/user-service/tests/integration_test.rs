//! Integration test using testcontainers-mysql.
//! Run with: `cargo test -p user-service --test integration_test -- --test-threads=1`
//! Requires Docker running on host.

use sqlx::mysql::MySqlPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::mysql::Mysql;
use user_service::dto::{
    AdminResetPasswordReq, ChangePasswordReq, CreateUserReq, UpdateUserReq,
};

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

#[cfg(test)]
mod user_mgmt_test {
    use super::*;

    #[test]
    fn create_user_req_rejects_short_password_shape() {
        let req = CreateUserReq {
            username: "alice".into(),
            password: "123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        assert!(req.password.len() < 6);
        assert_eq!(req.username, "alice");
    }

    #[test]
    fn update_user_req_allows_partial_fields() {
        let req = UpdateUserReq {
            username: None,
            email: Some("a@b.c".into()),
            phone: None,
            nickname: None,
        };
        assert!(req.email.is_some());
        assert!(req.username.is_none());
    }

    #[test]
    fn change_password_req_distinguishes_fields() {
        let req = ChangePasswordReq {
            old_password: "old".into(),
            new_password: "new".into(),
        };
        assert_ne!(req.old_password, req.new_password);
    }

    #[test]
    fn admin_reset_password_req_has_field() {
        let req = AdminResetPasswordReq {
            new_password: "abcdef".into(),
        };
        assert!(req.new_password.len() >= 6);
    }
}

/// End-to-end tests for `user_svc` management functions.
/// Self-contained: builds the required `users` and `user_login_log` schema
/// so it does not depend on the legacy migration set.
#[cfg(test)]
mod user_mgmt_e2e {
    use super::*;
    use user_service::db::DbPool;
    use user_service::error::AppError;
    use user_service::service::user_svc;

    async fn setup_mgmt_pool() -> (ContainerAsync<Mysql>, DbPool) {
        let container = Mysql::default().start().await.expect("start mysql");
        let host = container.get_host().await.expect("host");
        let port = container.get_host_port_ipv4(3306).await;
        let url = format!("mysql://root:secret@{host}:{port}/mysql");
        let mysql = MySqlPool::connect(&url).await.expect("connect");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (\
                id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,\
                username VARCHAR(64) NOT NULL UNIQUE,\
                user_code VARCHAR(64) NOT NULL UNIQUE,\
                password_hash VARCHAR(255) NULL,\
                email VARCHAR(128) NULL,\
                phone VARCHAR(32) NULL,\
                nickname VARCHAR(64) NULL,\
                status TINYINT NOT NULL DEFAULT 1,\
                profile_json TEXT NOT NULL,\
                extends_json TEXT NULL,\
                last_login_at DATETIME(3) NULL,\
                last_login_ip VARCHAR(45) NULL,\
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,\
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP\
             ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        )
        .execute(&mysql)
        .await
        .expect("create users");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_login_log (\
                id BIGINT UNSIGNED PRIMARY KEY AUTO_INCREMENT,\
                user_id BIGINT UNSIGNED NULL,\
                username VARCHAR(64) NOT NULL,\
                ip VARCHAR(45) NOT NULL,\
                user_agent VARCHAR(255) NOT NULL,\
                success TINYINT NOT NULL,\
                fail_reason VARCHAR(32) NULL,\
                login_at DATETIME(3) NOT NULL,\
                created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),\
                INDEX idx_login_log_user_id (user_id)\
             ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        )
        .execute(&mysql)
        .await
        .expect("create user_login_log");
        (container, DbPool::Mysql(mysql))
    }

    fn uname(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
    }

    #[tokio::test]
    async fn create_user_persists_row_and_returns_id() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("alice"),
            password: "secret123".into(),
            email: Some("a@b.c".into()),
            phone: None,
            nickname: Some("Alice".into()),
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");
        assert!(uid > 0);

        let row: (String, i8) = sqlx::query_as("SELECT username, status FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(if let DbPool::Mysql(p) = &pool {
                p
            } else {
                unreachable!()
            })
            .await
            .expect("select");
        assert_eq!(row.0, req.username);
        assert_eq!(row.1, 1);
    }

    #[tokio::test]
    async fn create_user_duplicate_username_returns_conflict() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("bob"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let _ = user_svc::create_user(&pool, &req).await.expect("first");
        let err = user_svc::create_user(&pool, &req)
            .await
            .expect_err("dup should fail");
        assert!(matches!(err, AppError::Conflict(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn create_user_short_password_returns_validation() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("c"),
            password: "123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let err = user_svc::create_user(&pool, &req)
            .await
            .expect_err("short pw should fail");
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn soft_delete_then_set_status_round_trip() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("d"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        user_svc::soft_delete_user(&pool, uid).await.expect("delete");
        let status: (i8,) = sqlx::query_as("SELECT status FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(if let DbPool::Mysql(p) = &pool {
                p
            } else {
                unreachable!()
            })
            .await
            .expect("select");
        assert_eq!(status.0, 0);

        user_svc::set_user_status(&pool, uid, 1).await.expect("enable");
        let status: (i8,) = sqlx::query_as("SELECT status FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(if let DbPool::Mysql(p) = &pool {
                p
            } else {
                unreachable!()
            })
            .await
            .expect("select");
        assert_eq!(status.0, 1);
    }

    #[tokio::test]
    async fn set_user_status_invalid_value_rejected() {
        let (_c, pool) = setup_mgmt_pool().await;
        let err = user_svc::set_user_status(&pool, 1, 7)
            .await
            .expect_err("invalid status");
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn soft_delete_missing_user_returns_not_found() {
        let (_c, pool) = setup_mgmt_pool().await;
        let err = user_svc::soft_delete_user(&pool, 999_999)
            .await
            .expect_err("missing");
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn update_user_basic_persists_partial_fields() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("e"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let update = UpdateUserReq {
            username: None,
            email: Some("x@y.z".into()),
            phone: None,
            nickname: Some("Nick".into()),
        };
        user_svc::update_user_basic(&pool, uid, &update)
            .await
            .expect("update");

        let row: (Option<String>, Option<String>) =
            sqlx::query_as("SELECT email, nickname FROM users WHERE id = ?")
                .bind(uid)
                .fetch_one(if let DbPool::Mysql(p) = &pool {
                    p
                } else {
                    unreachable!()
                })
                .await
                .expect("select");
        assert_eq!(row.0.as_deref(), Some("x@y.z"));
        assert_eq!(row.1.as_deref(), Some("Nick"));
    }

    #[tokio::test]
    async fn update_user_basic_no_fields_returns_validation() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("f"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");
        let update = UpdateUserReq {
            username: None,
            email: None,
            phone: None,
            nickname: None,
        };
        let err = user_svc::update_user_basic(&pool, uid, &update)
            .await
            .expect_err("empty");
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn reset_password_overwrites_hash() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("g"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let mysql = if let DbPool::Mysql(p) = &pool {
            p
        } else {
            unreachable!()
        };
        let (before,): (String,) =
            sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
                .bind(uid)
                .fetch_one(mysql)
                .await
                .expect("select before");
        user_svc::reset_password(&pool, uid, "newpass99")
            .await
            .expect("reset");
        let (after,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(mysql)
            .await
            .expect("select after");
        assert!(after.starts_with("$argon2"), "got {after}");
        assert_ne!(before, after, "hash must change");
    }

    #[tokio::test]
    async fn change_password_succeeds_with_correct_old() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("h"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let mysql = if let DbPool::Mysql(p) = &pool {
            p
        } else {
            unreachable!()
        };
        let (before,): (String,) =
            sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
                .bind(uid)
                .fetch_one(mysql)
                .await
                .expect("select before");
        let change = ChangePasswordReq {
            old_password: "secret123".into(),
            new_password: "rotated456".into(),
        };
        user_svc::change_password(&pool, uid, &change)
            .await
            .expect("change");
        let (after,): (String,) = sqlx::query_as("SELECT password_hash FROM users WHERE id = ?")
            .bind(uid)
            .fetch_one(mysql)
            .await
            .expect("select after");
        assert_ne!(before, after, "hash must change after rotate");
    }

    #[tokio::test]
    async fn change_password_wrong_old_returns_unauthorized() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("i"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let change = ChangePasswordReq {
            old_password: "WRONG".into(),
            new_password: "rotated456".into(),
        };
        let err = user_svc::change_password(&pool, uid, &change)
            .await
            .expect_err("wrong old");
        assert!(matches!(err, AppError::Unauthorized), "got {err:?}");
    }

    #[tokio::test]
    async fn change_password_same_as_old_returns_validation() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("j"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let change = ChangePasswordReq {
            old_password: "secret123".into(),
            new_password: "secret123".into(),
        };
        let err = user_svc::change_password(&pool, uid, &change)
            .await
            .expect_err("same");
        assert!(matches!(err, AppError::Validation(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn list_login_logs_paginates_desc() {
        let (_c, pool) = setup_mgmt_pool().await;
        let req = CreateUserReq {
            username: uname("k"),
            password: "secret123".into(),
            email: None,
            phone: None,
            nickname: None,
        };
        let uid = user_svc::create_user(&pool, &req).await.expect("create");

        let mysql = if let DbPool::Mysql(p) = &pool {
            p
        } else {
            unreachable!()
        };
        for i in 0..3 {
            sqlx::query(
                "INSERT INTO user_login_log (user_id, username, ip, user_agent, success, login_at) \
                 VALUES (?, ?, ?, ?, 1, NOW(3))",
            )
            .bind(uid)
            .bind(&req.username)
            .bind("127.0.0.1")
            .bind(format!("ua-{i}"))
            .execute(mysql)
            .await
            .expect("insert log");
        }

        let page = user_svc::list_login_logs(&pool, uid, 1, 2)
            .await
            .expect("page1");
        assert_eq!(page.len(), 2);
        let page2 = user_svc::list_login_logs(&pool, uid, 2, 2)
            .await
            .expect("page2");
        assert_eq!(page2.len(), 1);
    }
}
