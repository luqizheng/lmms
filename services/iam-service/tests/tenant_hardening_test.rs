//! Phase 8.3: tenant hardening — JWT tid enforcement, cross_tenant flag,
//! TenantScope resolution, and get_claims tenant scoping.
#![allow(deprecated)]
mod common;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use digua_common_cache::MemoryCache;
use digua_common_event::MySqlOutboxRepo;
use digua_jwt_ctx::{Principal, TenantScope};
use iam_service::config::{
    AppConfig, Config, ConsulConfig, DatabaseConfig, JwtConfig, RedisConfig,
};
use iam_service::dto::{CreateUserRequest, LoginRequest};
use iam_service::error::IamError;
use iam_service::service::token_forward::TokenForwardClient;
use iam_service::service::{AuthService, TenantService, UserService};
use iam_service::state::AppState;
use std::sync::Arc;

const PREFIX: &str = "superpowers:";

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
        matches!(err, digua_jwt_ctx::PrincipalError::MissingTenantId),
        "expected MissingTenantId, got: {err:?}"
    );
}

#[tokio::test]
async fn cross_tenant_and_get_claims() {
    // inline setup skeleton：MySQL + jwks + cache + outbox + config
    let mysql_h = common::spawn_mysql().await;
    let pool = mysql_h.pool;
    let config = Config {
        smtp: Default::default(),
        public: Default::default(),
        app: AppConfig {
            name: "iam-service".into(),
            host: "0.0.0.0".into(),
            port: 8080,
        },
        database: DatabaseConfig {
            url: mysql_h.url.clone(),
            max_connections: 5,
            min_connections: 1,
        },
        redis: RedisConfig {
            url: "redis://127.0.0.1:0".into(),
            ttl_sec: 86400,
            prefix: PREFIX.into(),
        },
        jwt: JwtConfig {
            private_key_path: "/dev/null".into(),
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
        digua_common_jwt::JWKSCache::from_pem(include_str!(
            "../../token-service/tests/fixtures/test_token_public.pem"
        ))
        .expect("test public key must parse"),
    );
    let mut state = AppState::new(
        digua_common_db::DbPool::from(pool.clone()),
        MemoryCache::new(10000),
        config.clone(),
        None,
        outbox_repo.clone(),
        jwks.clone(),
        std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
            digua_common_jwt::ServiceJwtConfig {
                token_service_url: "http://placeholder".into(),
                bootstrap_token: "test-bt".into(),
                caller_service_name: "iam-service".into(),
                caller_tenant: 1,
                refresh_skew: std::time::Duration::from_secs(60),
                request_timeout: std::time::Duration::from_secs(5),
            },
            reqwest::Client::new(),
            String::new(),
        )),
        Arc::new(TokenForwardClient::new(
            std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
                digua_common_jwt::ServiceJwtConfig {
                    token_service_url: "http://placeholder".into(),
                    bootstrap_token: "test-bt".into(),
                    caller_service_name: "iam-service".into(),
                    caller_tenant: 1,
                    refresh_skew: std::time::Duration::from_secs(60),
                    request_timeout: std::time::Duration::from_secs(5),
                },
                reqwest::Client::new(),
                String::new(),
            )),
            "http://placeholder".into(),
        )),
    );

    let tenant_svc = TenantService::new(state.clone());

    tenant_svc
        .register(sample_req("acme", "admin"))
        .await
        .unwrap();
    tenant_svc
        .register(sample_req("globex", "admin"))
        .await
        .unwrap();

    // 创建 admin：必须在 stub 之前，因为 stub 返回的 user-jwt `sub`
    // claim 必须 = admin.id（断言 `principal.user_id == admin.id`）。
    let user_svc = UserService::new(state.clone());
    let admin = user_svc
        .create(
            CreateUserRequest {
                send_invitation: None,
                username: "platform_admin_user".into(),
                password: Some("TestAdmin@123".into()),
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
    .execute(&common::mysql_of(&state))
    .await
    .unwrap();

    let (server_uri, _) = common::stub_token_service(&admin.id, 1, true).await;
    let service_jwt = std::sync::Arc::new(digua_common_jwt::ServiceJwtCache::new(
        digua_common_jwt::ServiceJwtConfig {
            token_service_url: server_uri.clone(),
            bootstrap_token: "test-bt".into(),
            caller_service_name: "iam-service".into(),
            caller_tenant: 1,
            refresh_skew: std::time::Duration::from_secs(60),
            request_timeout: std::time::Duration::from_secs(5),
        },
        reqwest::Client::new(),
        String::new(),
    ));
    let token_forward = Arc::new(TokenForwardClient::new(
        service_jwt.clone(),
        server_uri.clone(),
    ));
    // 替换 placeholder service_jwt + token_forward 为 stub-server 指向的真实例。
    let outbox_repo = Arc::new(MySqlOutboxRepo::new(pool.clone()));
    state = AppState::new(
        digua_common_db::DbPool::from(pool.clone()),
        cache,
        config.clone(),
        None,
        outbox_repo,
        jwks.clone(),
        service_jwt,
        token_forward,
    );

    let auth_svc = AuthService::new(state.clone());
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
    assert!(
        principal.cross_tenant,
        "platform_admin must have cross_tenant=true"
    );
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
    let (acme_tid,): (i64,) =
        sqlx::query_as("SELECT CAST(id AS SIGNED) FROM iam_tenants WHERE slug = 'acme'")
            .fetch_one(&common::mysql_of(&state))
            .await
            .unwrap();
    let (globex_tid,): (i64,) =
        sqlx::query_as("SELECT CAST(id AS SIGNED) FROM iam_tenants WHERE slug = 'globex'")
            .fetch_one(&common::mysql_of(&state))
            .await
            .unwrap();

    let acme_user = user_svc
        .create(
            CreateUserRequest {
                send_invitation: None,
                username: "acme-user-1".into(),
                password: Some("P@ssw0rd!".into()),
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
    assert!(
        !claims.cross_tenant,
        "regular user must not have cross_tenant=true"
    );
}
