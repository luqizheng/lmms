//! Phase 4.4-fu2 + 4.3-fu2: tests for /iam/internal/users/by-{role,dept} endpoints.
//! Service-jwt required (aud contains "service"); user-jwt rejected;
//! cross-tenant reads forbidden.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::db;
use iam_service::routes::build_router;
use iam_service::state::AppState;
use std::sync::Arc;
use testcontainers::ImageExt;
use testcontainers_modules::{mysql::Mysql, testcontainers::runners::AsyncRunner};
use tower::ServiceExt;

struct Ctx {
    app: axum::Router,
    state: AppState,
    _mysql: testcontainers::ContainerAsync<Mysql>,
    service_token_t1: String,
    service_token_t2: String,
    user_token: String,
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
        app: AppConfig { name: "iam-service".into(), host: "0.0.0.0".into(), port: 8080 },
        database: DatabaseConfig { url: url.clone(), max_connections: 5, min_connections: 1 },
        redis: RedisConfig { url: "redis://x".into(), ttl_sec: 86400, prefix: "test:".into() },
        jwt: JwtConfig {
            private_key_path: "/dev/null".into(),
            public_key_path: "/dev/null".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig { host: "consul".into(), port: 8500, service_name: "iam-service".into() },
    };
    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let state = AppState::new(pool, cache, config, None, outbox_repo);

    let service_token_t1 = mint_service_token(1);
    let service_token_t2 = mint_service_token(2);
    let user_token = mint_user_token("u1", 1);

    let app = build_router(state.clone());
    Ctx { app, state, _mysql: mysql_c, service_token_t1, service_token_t2, user_token }
}

fn mint_service_token(tid: i64) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\",\"typ\":\"JWT\"}");
    let payload = format!(r#"{{"aud":"service","tid":{tid},"sub":"svc"}}"#);
    let body = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    format!("{header}.{body}.signature")
}

fn mint_user_token(sub: &str, tid: i64) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let header = URL_SAFE_NO_PAD.encode(b"{\"alg\":\"none\",\"typ\":\"JWT\"}");
    let payload = format!(r#"{{"sub":"{sub}","tid":{tid},"roles":["admin"]}}"#);
    let body = URL_SAFE_NO_PAD.encode(payload.as_bytes());
    format!("{header}.{body}.signature")
}

async fn seed_tenant(state: &AppState, id: i64, slug: &str) {
    sqlx::query(
        "INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'active', NOW(), NOW()) \
         ON DUPLICATE KEY UPDATE id = id",
    )
    .bind(id).bind(slug).bind(slug)
    .execute(&state.db).await.unwrap();
}

async fn seed_tenant_user(state: &AppState, tenant: i64, uid: &str) {
    sqlx::query(
        "INSERT INTO iam_users (id, tenant_id, username, password_hash, salt, nickname, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'h', 's', ?, 'active', NOW(), NOW())",
    )
    .bind(uid).bind(tenant).bind(uid).bind(uid)
    .execute(&state.db).await.unwrap();
}

async fn seed_role(state: &AppState, rid: &str) {
    sqlx::query(
        "INSERT INTO iam_roles (id, name, code, data_scope, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'self', 'active', NOW(), NOW())",
    )
    .bind(rid).bind(rid).bind(rid)
    .execute(&state.db).await.unwrap();
}

async fn seed_org(state: &AppState, oid: &str) {
    sqlx::query(
        "INSERT INTO iam_org_units (id, name, code, parent_id, created_at, updated_at) \
         VALUES (?, ?, ?, NULL, NOW(), NOW())",
    )
    .bind(oid).bind(oid).bind(oid)
    .execute(&state.db).await.unwrap();
}

async fn bind_user_to_role(state: &AppState, uid: &str, rid: &str) {
    sqlx::query("INSERT INTO iam_user_roles (user_id, role_id, created_at) VALUES (?, ?, NOW())")
        .bind(uid).bind(rid)
        .execute(&state.db).await.unwrap();
}

async fn bind_user_to_org(state: &AppState, uid: &str, oid: &str) {
    sqlx::query(
        "INSERT INTO iam_user_org_units (user_id, org_unit_id, is_primary, created_at) \
         VALUES (?, ?, TRUE, NOW())",
    )
    .bind(uid).bind(oid)
    .execute(&state.db).await.unwrap();
}

async fn call(app: axum::Router, uri: &str, auth: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .header("Authorization", format!("Bearer {auth}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = if body_bytes.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or(serde_json::json!({}))
    };
    (status, json)
}

#[tokio::test]
async fn internal_users_by_role_returns_member_user_ids() {
    let ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 1, "u2").await;
    bind_user_to_role(&ctx.state, "u1", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-role/r1", &ctx.service_token_t1).await;
    assert_eq!(status, StatusCode::OK);
    let mut items: Vec<String> = body["items"]
        .as_array().unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    items.sort();
    assert_eq!(items, vec!["u1", "u2"]);
}

#[tokio::test]
async fn internal_users_by_dept_returns_member_user_ids() {
    let ctx = setup().await;
    seed_org(&ctx.state, "o1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 1, "u2").await;
    bind_user_to_org(&ctx.state, "u1", "o1").await;
    bind_user_to_org(&ctx.state, "u2", "o1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-dept/o1", &ctx.service_token_t1).await;
    assert_eq!(status, StatusCode::OK);
    let count = body["items"].as_array().unwrap().len();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn internal_users_tenant_isolation() {
    let ctx = setup().await;
    seed_tenant(&ctx.state, 2, "t2").await;
    seed_role(&ctx.state, "r1").await;
    seed_tenant_user(&ctx.state, 1, "u1").await;
    seed_tenant_user(&ctx.state, 2, "u2").await;
    bind_user_to_role(&ctx.state, "u1", "r1").await;
    bind_user_to_role(&ctx.state, "u2", "r1").await;

    let (status, body) = call(ctx.app, "/iam/internal/users/by-role/r1", &ctx.service_token_t2).await;
    assert_eq!(status, StatusCode::OK);
    let items: Vec<String> = body["items"]
        .as_array().unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(items, vec!["u2"]);
}

#[tokio::test]
async fn internal_users_rejects_user_jwt() {
    let ctx = setup().await;
    seed_role(&ctx.state, "r1").await;
    let (status, _) = call(ctx.app, "/iam/internal/users/by-role/r1", &ctx.user_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}