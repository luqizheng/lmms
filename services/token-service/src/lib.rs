use crate::db::DbPool;
use digua_common_http::ResilientClient;

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Method,
};
use std::sync::Arc;

// 显式引用（匿名 use）：本 crate 的方言 feature 需要转发 `digua_common` 的
// 同名 feature（见 Cargo.toml `[features]`），而 cargo 的 unused_dependencies
// lint 不把 feature 转发计为使用，需在代码里留一个真实引用点。
use digua_common as _;

/// Fetch JSON from iam-service via ResilientClient (timeout/retry/breaker).
/// `user_jwt` is the original `Authorization: Bearer <jwt>` value from the
/// inbound request (if any). Phase 5-A-2 dephase: forwarded as-is to the
/// upstream, replacing the previous sidecar mTLS service-identity.
pub async fn fetch_json(
    client: &ResilientClient,
    service: &str,
    url: &str,
    user_jwt: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut headers = HeaderMap::new();
    if let Some(bearer) = user_jwt {
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {bearer}")) {
            headers.insert(AUTHORIZATION, v);
        }
    }
    let resp = client
        .call(service, Method::GET, url, &headers, bytes::Bytes::new())
        .await
        .map_err(|e| format!("{:?}", e))?;
    if !resp.status.is_success() {
        return Err(format!("HTTP {}", resp.status));
    }
    serde_json::from_slice(&resp.body).map_err(|e| e.to_string())
}

/// 2026-11-05 review (P0-3): mint a short-lived self-signed service-jwt for
/// token-service's own outbound `/internal` calls (e.g. iam user claims).
/// Downstream internal endpoints enforce `aud == <target>` via
/// `verify_aud_principal_middleware`, so calls must carry a service-jwt
/// instead of a user-jwt (or nothing at all).
/// Legacy helper: signs with the legacy "default" kid. Kept for tests and
/// tooling; request-path callers must use `mint_service_bearer_for_state`
/// so the header kid matches the active keystore key (kid-strict verifiers).
pub fn mint_service_bearer(
    private_key_pem: &str,
    aud: &str,
    ttl_secs: i64,
) -> Result<String, String> {
    mint_service_bearer_with_kid(private_key_pem, crate::issuer::JWT_KEY_ID, aud, ttl_secs)
}

/// Mint an outbound service-jwt signed by the ACTIVE keystore key with its
/// real kid — downstream verifiers are kid-strict, so after a rotation the
/// header kid must equal the active key's JWKS kid.
pub fn mint_service_bearer_for_state(
    state: &TokenServerState,
    aud: &str,
    ttl_secs: i64,
) -> Result<String, String> {
    let (pem, kid) = {
        let ks = state.keys.read().expect("keystore lock poisoned");
        (ks.active().private_pem.clone(), ks.active().kid.clone())
    };
    mint_service_bearer_with_kid(&pem, &kid, aud, ttl_secs)
}

pub fn mint_service_bearer_with_kid(
    private_key_pem: &str,
    kid: &str,
    aud: &str,
    ttl_secs: i64,
) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": "token-service",
        "aud": aud,
        "tid": 0,
        "iat": now,
        "exp": now + ttl_secs,
        "roles": ["service"],
        "purpose": crate::issuer::purpose::SERVICE,
    });
    digua_common_jwt::sign_token_with_kid(
        private_key_pem,
        kid,
        &claims,
        std::time::Duration::from_secs(ttl_secs.max(1) as u64),
    )
    .map_err(|e| format!("mint service token: {}", e))
}

#[derive(Clone)]
pub struct TokenServerState {
    pub db: DbPool,
    pub issuer: Arc<crate::issuer::TokenIssuer>,
    pub issuer_str: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: Arc<ResilientClient>,
    pub user_service_url: String,
    /// 2026-11-23 keystore: single source of truth for all signing keys
    /// (active + retiring + retired). Replaces the old `jwt_public_key` /
    /// `private_key_pem` string fields, which went stale after rotation.
    pub keys: Arc<std::sync::RwLock<crate::keystore::KeyStore>>,
    /// Phase 5-B: JWKS cache for verifying inbound tokens.
    pub jwks: Arc<digua_common_jwt::JWKSCache>,
}

impl TokenServerState {
    /// PEM of the active signing key (service-jwt minting, outbound iam
    /// bearer). Kept as a helper so call sites stay short.
    pub fn active_private_pem(&self) -> String {
        self.keys
            .read()
            .expect("keystore lock poisoned")
            .active()
            .private_pem
            .clone()
    }
}

impl digua_jwt_ctx::PrincipalStateProvider for TokenServerState {
    fn jwks(&self) -> Arc<digua_common_jwt::JWKSCache> {
        self.jwks.clone()
    }
}

pub mod audit;
pub mod config;
pub mod db;
pub mod issuer;
pub mod keystore;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod secret;
pub mod service;
pub mod store;

pub use config::{
    apply_env_overrides, load_config, load_config_with_env, validate_all_fields_present,
    Config as TokenServerConfig, DatabaseCfg, DbType, DEFAULT_CONFIG_PATH,
};
pub use digua_common_jwt;
pub use issuer::TokenIssuer;
pub use routes::service_jwt::{
    handle_service_jwt, routes as service_jwt_routes, ServiceJwtRequest, ServiceJwtResponse,
};
