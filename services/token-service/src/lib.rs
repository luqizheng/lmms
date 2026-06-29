use crate::db::DbPool;
use common_http::ResilientClient;
use reqwest::{header::HeaderMap, Method};
use std::sync::Arc;

/// Fetch JSON from iam-service via ResilientClient (timeout/retry/breaker).
pub async fn fetch_json(
    client: &ResilientClient,
    service: &str,
    url: &str,
) -> Result<serde_json::Value, String> {
    let resp = client
        .call(service, Method::GET, url, &HeaderMap::new(), bytes::Bytes::new())
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
pub use consul_registrar::ConsulRegistrar;
pub use issuer::TokenIssuer;
