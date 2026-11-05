use crate::db::DbPool;
use digua_common_http::ResilientClient;

use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Method,
};
use std::sync::Arc;

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
pub fn mint_service_bearer(
    private_key_pem: &str,
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
    digua_common_jwt::sign_token(
        private_key_pem,
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
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: Arc<ResilientClient>,
    pub user_service_url: String,
    /// Phase 5-B: PEM-encoded private key for signing service-jwts
    /// (separate from `issuer` to keep user-jwt signing path stable).
    pub private_key_pem: String,
    /// Phase 5-B: pre-shared bootstrap tokens accepted by
    /// `POST /tokens/service-jwt`. Each business service has its own
    /// 32-byte random token; rotate by editing this list and restarting.
    pub bootstrap_tokens: Vec<String>,
    /// Phase 5-B: JWKS cache for verifying inbound tokens.
    pub jwks: Arc<digua_common_jwt::JWKSCache>,
}

impl digua_jwt_ctx::PrincipalStateProvider for TokenServerState {
    fn jwks(&self) -> Arc<digua_common_jwt::JWKSCache> {
        self.jwks.clone()
    }
}

pub mod config;
pub mod db;
pub mod issuer;
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
