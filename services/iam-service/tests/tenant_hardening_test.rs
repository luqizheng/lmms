//! Phase 8.3: tenant hardening — JWT tid enforcement, cross_tenant flag,
//! TenantScope resolution, and get_claims tenant scoping.
mod common;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use common_cache::MemoryCache;
use common_event::MySqlOutboxRepo;
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::dto::{CreateUserRequest, LoginRequest};
use iam_service::error::IamError;
use iam_service::service::{AuthService, TenantService, UserService};
use iam_service::state::AppState;
use jwt_ctx::{Principal, TenantScope};
use jwt_simple::prelude::*;
use std::io::Write;
use std::sync::Arc;

const PREFIX: &str = "superpowers:";

struct Ctx {
    state: AppState,
    _mysql: Option<common::MysqlContainer>,
    _key_file: tempfile::NamedTempFile,
}

async fn setup() -> Ctx {
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let url = mysql_h.url;

    let key_pair = RS256KeyPair::generate(2048).unwrap();
    let pem = key_pair.to_pem().unwrap();
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
            url: "redis://127.0.0.1:0".into(),
            ttl_sec: 86400,
            prefix: PREFIX.into(),
        },
        jwt: JwtConfig {
            private_key_path: key_file.path().to_str().unwrap().to_string().into(),
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
    let jwks = std::sync::Arc::new(common_jwt::JWKSCache::new(
        common_jwt::JwksConfig {
            url: "http://localhost:0/.well-known/jwks.json".into(),
            refresh_interval: std::time::Duration::from_secs(3600),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
    ));
    let service_jwt = std::sync::Arc::new(common_jwt::ServiceJwtCache::new(
        common_jwt::ServiceJwtConfig {
            token_service_url: "http://localhost:0".into(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));    let state = AppState::new(pool, cache, config, None, outbox_repo, jwks, service_jwt);
    Ctx {
        state,
        _mysql: mysql_h.container,
        _key_file: key_file,
    }
}

fn sample_req(slug: &str, admin_username: &str) -> iam_service::dto::CreateTenantRequest {
    iam_service::dto::CreateTenantRequest {
        tenant_slug: slug.to_string(),
        display_name: format!("{slug} display"),
        admin_username: admin_username.to_string(),
        admin_password: "secret-strong-pw-123".to_string(),
        admin_email: None,
    }
}

#[tokio::test]
async fn tid_missing_returns_error() {
    let header = r#"{"alg":"RS256","typ":"JWT"}"#;
    let payload = r#"{"sub":"user-1","username":"alice"}"#;
    let h_b64 = URL_SAFE_NO_PAD.encode(header);
    let p_b64 = URL_SAFE_NO_PAD.encode(payload);
    let token = format!("{h_b64}.{p_b64}.fake_sig");

    let err = Principal::from_jwt_payload(&token).unwrap_err();
    assert!(
        matches!(err, jwt_ctx::PrincipalError::MissingTenantId),
        "expected MissingTenantId, got: {err:?}"
    );
}

#[tokio::test]
async fn cross_tenant_and_get_claims() {
    let ctx = setup().await;
    let tenant_svc = TenantService::new(ctx.state.clone());

    tenant_svc
        .register(sample_req("acme", "admin"))
        .await
        .unwrap();
    tenant_svc
        .register(sample_req("globex", "admin"))
        .await
        .unwrap();

    // cross_tenant_scope: login as user with platform_admin role
    let user_svc = UserService::new(ctx.state.clone());
    let admin = user_svc
        .create(
            CreateUserRequest {
                username: "platform_admin_user".into(),
                password: "TestAdmin@123".into(),
                email: None,
                phone: None,
                nickname: "PA".into(),
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
    .bind(&admin.id)
    .execute(&ctx.state.db)
    .await
    .unwrap();

    let auth_svc = AuthService::new(ctx.state.clone());
    let login = auth_svc
        .login(
            LoginRequest {
                username: "platform_admin_user".into(),
                password: "TestAdmin@123".into(),
                tenant_slug: None,
            },
            "127.0.0.1",
            "test",
        )
        .await
        .unwrap();

    let principal = Principal::from_jwt_payload(&login.access_token).unwrap();
    assert!(principal.cross_tenant, "platform_admin must have cross_tenant=true");
    assert_eq!(principal.user_id, admin.id);

    let scope = if principal.cross_tenant {
        TenantScope::CrossTenant
    } else {
        TenantScope::Single(principal.tenant_id)
    };
    assert_eq!(scope, TenantScope::CrossTenant);

    // get_claims_wrong_tenant
    let acme = tenant_svc
        .register(sample_req("acme", "admin"))
        .await
        .unwrap_err();
    assert!(matches!(acme, IamError::DuplicateTenant));

    let globex = tenant_svc
        .register(sample_req("globex", "admin"))
        .await
        .unwrap_err();
    assert!(matches!(globex, IamError::DuplicateTenant));

    // fetch tenant ids
    let (acme_tid,): (i64,) = sqlx::query_as(
        "SELECT CAST(id AS SIGNED) FROM iam_tenants WHERE slug = 'acme'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();
    let (globex_tid,): (i64,) = sqlx::query_as(
        "SELECT CAST(id AS SIGNED) FROM iam_tenants WHERE slug = 'globex'",
    )
    .fetch_one(&ctx.state.db)
    .await
    .unwrap();

    let acme_user = user_svc
        .create(
            CreateUserRequest {
                username: "acme-user-1".into(),
                password: "P@ssw0rd!".into(),
                email: None,
                phone: None,
                nickname: "AU1".into(),
                avatar: None,
                role_ids: None,
            },
            &TenantScope::Single(acme_tid),
        )
        .await
        .unwrap();

    let err = user_svc
        .get_claims(&acme_user.id, &TenantScope::Single(globex_tid))
        .await
        .unwrap_err();
    assert!(
        matches!(err, IamError::UserNotFound),
        "get_claims with wrong tenant must return UserNotFound, got: {err:?}"
    );

    let claims = user_svc
        .get_claims(&acme_user.id, &TenantScope::Single(acme_tid))
        .await
        .unwrap();
    assert_eq!(claims.user_id, acme_user.id);
    assert!(!claims.cross_tenant, "regular user must not have cross_tenant=true");
}
