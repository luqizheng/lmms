//! Phase 8.2: cross-tenant isolation for iam business tables (roles / orgs / users).
//! Phase 8.3-fu1: platform_admin cross-tenant point query (HTTP path).
#![allow(deprecated)]
mod common;

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use digua_common_cache::MemoryCache;
use digua_common_event::MySqlOutboxRepo;
use digua_jwt_ctx::{Principal, TenantScope};
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::dto::{
    CreateOrgUnitRequest, CreateRoleRequest, CreateTenantRequest, CreateUserRequest, LoginRequest,
    RoleQuery, UserQuery,
};
use iam_service::routes::build_router;
use iam_service::service::token_forward::TokenForwardClient;
use iam_service::service::{AuthService, OrgService, RoleService, TenantService, UserService};
use iam_service::state::AppState;
use jwt_simple::prelude::*;
use std::io::Write;
use std::sync::Arc;
use tower::ServiceExt;

struct Ctx {
    state: AppState,
    app: axum::Router,
    _mysql: Option<common::MysqlContainer>,
    _key_file: tempfile::NamedTempFile,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let url = mysql_h.url;

    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let pem = key_pair.to_pem().unwrap();
    let public_pem = key_pair.public_key().to_pem().unwrap();
    let mut key_file = tempfile::NamedTempFile::new().unwrap();
    key_file.write_all(pem.as_bytes()).unwrap();
    key_file.flush().unwrap();

    let config = Config {
        app: AppConfig {
            name: "iam-service".into(),
            host: "0.0.0.0".into(),
            port: 8080,
        },
        database: DatabaseConfig {
            url: url.clone(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: RedisConfig {
            url: "redis://x".into(),
            ttl_sec: 86400,
            prefix: "test:".into(),
        },
        jwt: JwtConfig {
            private_key_path: key_file.path().to_path_buf(),
            public_key_path: "/dev/null".into(),
            access_token_ttl_minutes: 15,
            refresh_token_ttl_days: 7,
        },
        consul: ConsulConfig {
            host: "consul".into(),
            port: 8500,
            service_name: "iam-service".into(),
        },
    };
    let cache = MemoryCache::new(10000);
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    let jwks = std::sync::Arc::new(
        digua_common_jwt::JWKSCache::from_pem(&public_pem).expect("test public key must parse"),
    );
    let service_jwt = std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let token_forward = Arc::new(TokenForwardClient::new(service_jwt.clone(), String::new()));
    let state = AppState::new(
        pool,
        cache,
        config,
        None,
        outbox_repo,
        jwks,
        service_jwt,
        token_forward,
    );
    let app = build_router(state.clone());

    Ctx {
        state,
        app,
        _mysql: mysql_h.container,
        _key_file: key_file,
    }
}

async fn seed_tenant(state: &AppState, id: i64, slug: &str) {
    sqlx::query(
        "INSERT INTO iam_tenants (id, slug, display_name, status, created_at, updated_at) \
         VALUES (?, ?, ?, 'active', NOW(), NOW()) \
         ON DUPLICATE KEY UPDATE id = id",
    )
    .bind(id)
    .bind(slug)
    .bind(slug)
    .execute(&state.db)
    .await
    .unwrap();
}

#[tokio::test]
async fn roles_are_tenant_isolated() {
    let ctx = setup().await;
    let svc = RoleService::new(ctx.state.clone());

    // seed tenant 2
    seed_tenant(&ctx.state, 2, "tenant-2").await;

    // create role in tenant 2
    let created = svc
        .create(
            CreateRoleRequest {
                name: "Acme Admin".into(),
                code: "acme-admin".into(),
                description: None,
                data_scope: 3,
            },
            &TenantScope::Single(2),
        )
        .await
        .unwrap();

    // tenant 2 list sees it
    let list_t2 = svc
        .list(
            RoleQuery {
                page: 1,
                size: 50,
                keyword: None,
                status: None,
            },
            &TenantScope::Single(2),
        )
        .await
        .unwrap();
    assert!(list_t2.items.iter().any(|r| r.id == created.id));

    // tenant 3 list does NOT see it
    let list_t3 = svc
        .list(
            RoleQuery {
                page: 1,
                size: 50,
                keyword: None,
                status: None,
            },
            &TenantScope::Single(3),
        )
        .await
        .unwrap();
    assert!(
        !list_t3.items.iter().any(|r| r.id == created.id),
        "cross-tenant role leak"
    );

    // tenant 3 get by id -> RoleNotFound
    let err = svc
        .get(&created.id, &TenantScope::Single(3))
        .await
        .unwrap_err();
    assert!(matches!(err, iam_service::error::IamError::RoleNotFound));
}

#[tokio::test]
async fn org_units_are_tenant_isolated() {
    let ctx = setup().await;
    let svc = OrgService::new(ctx.state.clone());

    seed_tenant(&ctx.state, 2, "tenant-2").await;

    let created = svc
        .create(
            CreateOrgUnitRequest {
                parent_id: None,
                name: "Acme HQ".into(),
                code: "acme-hq".into(),
                sort_order: Some(0),
            },
            &TenantScope::Single(2),
        )
        .await
        .unwrap();

    let tree_t2 = svc.list(&TenantScope::Single(2)).await.unwrap();
    assert!(tree_t2.iter().any(|o| o.id == created.id));
    let tree_t3 = svc.list(&TenantScope::Single(3)).await.unwrap();
    assert!(
        !tree_t3.iter().any(|o| o.id == created.id),
        "cross-tenant org leak"
    );

    let err = svc
        .get(&created.id, &TenantScope::Single(3))
        .await
        .unwrap_err();
    assert!(matches!(err, iam_service::error::IamError::OrgUnitNotFound));
}

#[tokio::test]
async fn users_are_tenant_isolated() {
    let ctx = setup().await;
    let svc = UserService::new(ctx.state.clone());

    seed_tenant(&ctx.state, 2, "tenant-2").await;
    seed_tenant(&ctx.state, 3, "tenant-3").await;

    let created = svc
        .create(
            CreateUserRequest {
                username: "u-t2".into(),
                password: "P@ssw0rd!".into(),
                nickname: "U2".into(),
                email: None,
                phone: None,
                avatar: None,
                role_ids: None,
            },
            &TenantScope::Single(2),
        )
        .await
        .unwrap();

    let l2 = svc
        .list(
            UserQuery {
                page: 1,
                size: 50,
                keyword: None,
                status: None,
                org_unit_id: None,
            },
            &TenantScope::Single(2),
        )
        .await
        .unwrap();
    assert!(l2.items.iter().any(|u| u.id == created.id));
    let l3 = svc
        .list(
            UserQuery {
                page: 1,
                size: 50,
                keyword: None,
                status: None,
                org_unit_id: None,
            },
            &TenantScope::Single(3),
        )
        .await
        .unwrap();
    assert!(
        !l3.items.iter().any(|u| u.id == created.id),
        "cross-tenant user leak in /iam/users"
    );

    let err = svc
        .get(&created.id, &TenantScope::Single(3))
        .await
        .unwrap_err();
    assert!(matches!(err, iam_service::error::IamError::UserNotFound));

    let dup = svc
        .create(
            CreateUserRequest {
                username: "u-t2".into(),
                password: "P@ssw0rd!".into(),
                nickname: "U3-dup".into(),
                email: None,
                phone: None,
                avatar: None,
                role_ids: None,
            },
            &TenantScope::Single(3),
        )
        .await
        .unwrap();
    assert_eq!(dup.username, "u-t2");
    assert_ne!(
        dup.id, created.id,
        "same username in different tenants must succeed"
    );
}

fn tenant_req(slug: &str, admin: &str) -> CreateTenantRequest {
    CreateTenantRequest {
        tenant_slug: slug.to_string(),
        display_name: format!("{slug} display"),
        admin_username: admin.to_string(),
        admin_password: "P@ssw0rd!".to_string(),
        admin_email: None,
    }
}

#[tokio::test]
async fn platform_admin_point_query_cross_tenant() {
    let ctx = setup().await;

    let tenant_svc = TenantService::new(ctx.state.clone());
    let acme = tenant_svc
        .register(tenant_req("acme", "acme_admin"))
        .await
        .unwrap();

    let user_svc = UserService::new(ctx.state.clone());
    let acme_user = user_svc
        .create(
            CreateUserRequest {
                username: "acme_user".into(),
                password: "P@ssw0rd!".into(),
                nickname: "AU".into(),
                email: None,
                phone: None,
                avatar: None,
                role_ids: None,
            },
            &TenantScope::Single(acme.tenant_id),
        )
        .await
        .unwrap();

    let default_admin = user_svc
        .create(
            CreateUserRequest {
                username: "default_admin".into(),
                password: "P@ssw0rd!".into(),
                nickname: "DA".into(),
                email: None,
                phone: None,
                avatar: None,
                role_ids: None,
            },
            &TenantScope::Single(1),
        )
        .await
        .unwrap();

    sqlx::query(
        "INSERT IGNORE INTO iam_user_roles (user_id, role_id, created_at) \
         VALUES (?, 'role-platform-admin-seed', NOW())",
    )
    .bind(&default_admin.id)
    .execute(&ctx.state.db)
    .await
    .unwrap();

    let auth = AuthService::new(ctx.state.clone());
    let login = auth
        .login(
            LoginRequest {
                username: "default_admin".into(),
                password: "P@ssw0rd!".into(),
                tenant_slug: None,
            },
            "127.0.0.1",
            "test",
        )
        .await
        .unwrap();

    let principal = Principal::from_jwt_payload(&login.access_token).unwrap();
    assert!(
        principal.cross_tenant,
        "platform_admin must have cross_tenant=true"
    );
    assert_eq!(
        principal.tenant_id, 1,
        "default_admin must have tenant_id=1"
    );

    let resp = ctx
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/iam/users/{}", acme_user.id))
                .header("Authorization", format!("Bearer {}", login.access_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(
        body["id"], acme_user.id,
        "platform_admin must read cross-tenant user via id"
    );
    assert_eq!(body["username"], "acme_user");
    let _ = acme.tenant_id;
}
