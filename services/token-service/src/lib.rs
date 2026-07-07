use crate::db::DbPool;
use common_http::ResilientClient;
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

pub use config::{load_config, Config as TokenServerConfig, DatabaseCfg, DbType};
pub use issuer::TokenIssuer;
pub use routes::service_jwt::{
    handle_service_jwt, routes as service_jwt_routes, ServiceJwtRequest, ServiceJwtResponse,
};
