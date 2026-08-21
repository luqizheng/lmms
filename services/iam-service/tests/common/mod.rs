//! Integration test helper: 启动/连接 MySQL，建测试 schema。
//!
//! 两条路径：
//! 1. 环境变量 `TEST_MYSQL_HOST` + `TEST_MYSQL_PORT` 存在时，连共享 MySQL 容器，
//!    建独立 `test_<svc>_<pid>_<uuid>` database，跑 `iam_service::db::ensure_schema`。
//!    用于 `scripts/run-tests-docker.js` 的共享模式（一个 mysql 容器供所有 service）。
//! 2. 否则回退到本地 testcontainers 自己起一个 mysql 容器（保留 `cargo test` 单跑体验）。
use digua_common_jwt::sign_token;
use iam_service::db;
use sqlx::mysql::MySqlPool;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::ImageExt;
use testcontainers_modules::mysql::Mysql;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test private key for service-jwt signing (token-service fixtures).
const TEST_TOKEN_PRIVATE_PEM: &str =
    include_str!("../../../token-service/tests/fixtures/test_token_private.pem");

pub type MysqlContainer = testcontainers::ContainerAsync<Mysql>;

pub struct MysqlHandle {
    pub pool: MySqlPool,
    #[allow(dead_code)]
    // exposed for integration tests that need to assert on the URL; not all suites read it.
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
    sqlx::query(&format!(
        "CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4"
    ))
    .execute(&admin)
    .await
    .expect("create test database");
    admin.close().await;

    let url = format!("mysql://root@{host}:{port}/{db_name}");
    let pool = digua_common::db::connect(&url, 5)
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
    let pool = digua_common::db::connect(&url, 5)
        .await
        .expect("connect local mysql");
    db::ensure_schema(&pool).await.expect("ensure iam schema");

    MysqlHandle {
        pool,
        url,
        container: Some(c),
    }
}

/// 起 wiremock stub token-service 的两个端点（`/tokens/service-jwt` 与
/// `/tokens/user-jwt`），返回 `(server_uri, user_jwt)`。**调用方必须把
/// `server_uri` 同时填进 `ServiceJwtCache.config.token_service_url` 与
/// `TokenForwardClient` 的 url**，两段链路才会打 wiremock；只填后者会
/// 漏 service-jwt mint，auth.login() 仍报 `localhost:0/tokens/service-jwt`。
///
/// `sub` 是 stub 返回 user-jwt 的 `sub` claim，应该 = 测试用例 seed
/// 出的 `iam_users.id`（不是 username）。
pub async fn stub_token_service(sub: &str, tenant_id: i64, cross_tenant: bool) -> (String, String) {
    use serde_json::json;
    let now = chrono::Utc::now().timestamp();

    let svc_claims = json!({
        "iss": "token-service",
        "sub": "iam-service",
        "aud": "token-service",
        "tid": tenant_id,
        "iat": now,
        "exp": now + 3600,
        "roles": ["service"],
    });
    let service_jwt = sign_token(
        TEST_TOKEN_PRIVATE_PEM,
        &svc_claims,
        Duration::from_secs(3600),
    )
    .expect("sign svc-jwt");

    let user_claims = json!({
        "iss": "token-service",
        "sub": sub,
        "tid": tenant_id,
        "ct": cross_tenant,
        "iat": now,
        "exp": now + 3600,
        "user_code": sub,
        "roles": ["user"],
    });
    let user_jwt = sign_token(
        TEST_TOKEN_PRIVATE_PEM,
        &user_claims,
        Duration::from_secs(3600),
    )
    .expect("sign user-jwt");

    let server = MockServer::start().await;
    let svc_clone = service_jwt.clone();
    Mock::given(method("POST"))
        .and(path("/tokens/service-jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": svc_clone,
            "expires_in": 3600,
        })))
        .mount(&server)
        .await;
    let user_clone = user_jwt.clone();
    Mock::given(method("POST"))
        .and(path("/tokens/user-jwt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": user_clone,
            "expires_in": 3600,
        })))
        .mount(&server)
        .await;
    (server.uri(), user_jwt)
}
