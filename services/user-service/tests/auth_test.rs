//! Integration tests for /api/v1/auth/{login,register,setup-status}.
//! Run with: `cargo test -p user-service --test auth_test -- --test-threads=1`

use redis::AsyncCommands;
use serde_json::Value;
use sqlx::mysql::MySqlPool;
use std::net::SocketAddr;
use std::time::Duration as StdDuration;
use testcontainers_modules::{mysql::Mysql, redis::Redis};
use tokio::net::TcpListener;
use user_service::config::{
    Argon2Cfg, AuthCfg, Config, DatabaseCfg, DbType, Duration, LoginSecurityCfg, RedisCfg,
    ServerCfg,
};
use user_service::routes::auth::{router as auth_router, AuthState};

struct Harness {
    _pool: MySqlPool,
    addr: SocketAddr,
    redis_client: redis::Client,
}

async fn setup() -> Harness {
    let mysql = Mysql::default().start().await.expect("start mysql");
    let redis = Redis::default().start().await.expect("start redis");

    let mysql_host = mysql.get_host().await.expect("host");
    let mysql_port = mysql.get_host_port_ipv4(3306).await;
    let redis_host = redis.get_host().await.expect("host");
    let redis_port = redis.get_host_port_ipv4(6379).await;

    let mysql_url = format!("mysql://root:secret@{mysql_host}:{mysql_port}/mysql");
    let redis_url = format!("redis://{redis_host}:{redis_port}");

    let pool = MySqlPool::connect(&mysql_url).await.expect("connect mysql");

    sqlx::query(include_str!(
        "../../migrations/2026_06_10_add_password_and_login_log.sql"
    ))
    .execute(&pool)
    .await
    .expect("run migration");

    // Seed admin/user roles (idempotent)
    let _ = sqlx::query("INSERT IGNORE INTO roles (code, name, status) VALUES ('admin', '管理员', 1), ('user', '用户', 1)")
        .execute(&pool).await;

    let redis_client = redis::Client::open(redis_url.clone()).expect("redis client");
    let cfg = Config {
        server: ServerCfg {
            host: "127.0.0.1".into(),
            port: 0,
        },
        database: DatabaseCfg {
            db_type: DbType::Mysql,
            url: mysql_url.clone(),
            max_connections: 5,
        },
        redis: RedisCfg {
            url: redis_url.clone(),
        },
        auth: AuthCfg {
            service_token_issuer: "x".into(),
            service_token_audience: "y".into(),
        },
        login_security: LoginSecurityCfg {
            max_failures: 3,
            lockout_duration: Duration(StdDuration::from_secs(300)),
            counter_window: Duration(StdDuration::from_secs(900)),
            code_ttl: Duration(StdDuration::from_secs(600)),
            argon2: Argon2Cfg {
                memory_kib: 12288,
                iterations: 1,
                parallelism: 1,
            },
        },
    };
    let state = AuthState {
        pool: pool.clone(),
        redis: redis_client.clone(),
        cfg,
    };
    let app = auth_router(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    Harness {
        _pool: pool,
        addr,
        redis_client,
    }
}

#[tokio::test]
async fn setup_status_true_when_no_users() {
    let h = setup().await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/api/v1/auth/setup-status", h.addr))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["setup_required"], true);
}

#[tokio::test]
async fn register_then_setup_status_false() {
    let h = setup().await;
    let client = reqwest::Client::new();

    let username = format!("admin_{}", uuid::Uuid::new_v4().simple());

    let resp = client
        .post(format!("http://{}/api/v1/auth/register", h.addr))
        .json(&serde_json::json!({
            "username": username,
            "password": "admin123",
            "email": "admin@test.local"
        }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 201);

    let resp = client
        .get(format!("http://{}/api/v1/auth/setup-status", h.addr))
        .send()
        .await
        .expect("request");
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["setup_required"], false);
}

#[tokio::test]
async fn register_second_user_rejected_403() {
    let h = setup().await;
    let client = reqwest::Client::new();

    let first = format!("first_{}", uuid::Uuid::new_v4().simple());
    let second = format!("second_{}", uuid::Uuid::new_v4().simple());

    let resp = client
        .post(format!("http://{}/api/v1/auth/register", h.addr))
        .json(&serde_json::json!({ "username": first, "password": "pass1234" }))
        .send()
        .await
        .expect("request 1");
    assert_eq!(resp.status().as_u16(), 201);

    let resp = client
        .post(format!("http://{}/api/v1/auth/register", h.addr))
        .json(&serde_json::json!({ "username": second, "password": "pass1234" }))
        .send()
        .await
        .expect("request 2");
    assert_eq!(resp.status().as_u16(), 403);
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["error"], "registration closed");
}

#[tokio::test]
async fn register_short_password_returns_400() {
    let h = setup().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/v1/auth/register", h.addr))
        .json(&serde_json::json!({ "username": "shorty", "password": "123" }))
        .send()
        .await
        .expect("request");
    assert_eq!(resp.status().as_u16(), 400);
}

#[tokio::test]
async fn login_invalid_credentials_returns_401() {
    let h = setup().await;
    let username = format!("ghost_{}", uuid::Uuid::new_v4().simple());

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/v1/auth/login", h.addr))
        .json(&serde_json::json!({ "username": username, "password": "wrongpass" }))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status().as_u16(), 401);
    let body: Value = resp.json().await.expect("parse");
    assert_eq!(body["error"], "invalid credentials");

    let mut conn = h
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .expect("redis conn");
    let _: () = conn.del(format!("auth:login:fail:{username}")).await.ok();
    let _: () = conn.del(format!("auth:login:lock:{username}")).await.ok();
}

#[tokio::test]
async fn login_empty_fields_returns_400() {
    let h = setup().await;
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}/api/v1/auth/login", h.addr))
        .json(&serde_json::json!({ "username": "", "password": "" }))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status().as_u16(), 400);
}

#[tokio::test]
async fn login_attempts_increment_failure_counter() {
    let h = setup().await;
    let username = format!("counter_{}", uuid::Uuid::new_v4().simple());

    let client = reqwest::Client::new();
    for _ in 0..3 {
        let resp = client
            .post(format!("http://{}/api/v1/auth/login", h.addr))
            .json(&serde_json::json!({ "username": username, "password": "wrong" }))
            .send()
            .await
            .expect("request");
        assert_eq!(resp.status().as_u16(), 401);
    }

    let mut conn = h
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .expect("conn");
    let fail_key = format!("auth:login:fail:{username}");
    let count: i64 = conn.get(&fail_key).await.unwrap_or(0);
    assert!(count >= 3, "expected fail counter >=3, got {count}");

    let _: () = conn.del(&fail_key).await.ok();
    let _: () = conn.del(format!("auth:login:lock:{username}")).await.ok();
}
