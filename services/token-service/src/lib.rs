use crate::db::DbPool;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;

pub type TokenHttpClient = Client<HttpConnector, Full<Bytes>>;

pub async fn fetch_json(client: &TokenHttpClient, url: &str) -> Result<serde_json::Value, String> {
    let req = Request::builder()
        .method("GET")
        .uri(url)
        .body(Full::new(Bytes::new()))
        .map_err(|e| format!("build request: {}", e))?;
    let resp = client.request(req).await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp
        .into_body()
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .to_bytes();
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

#[derive(Clone)]
pub struct TokenServerState {
    pub db: DbPool,
    pub issuer: std::sync::Arc<crate::issuer::TokenIssuer>,
    pub issuer_str: String,
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: TokenHttpClient,
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
pub mod tracing_mw;

pub use config::{load_config, Config as TokenServerConfig, DatabaseCfg, DbType};
pub use consul_registrar::ConsulRegistrar;
pub use issuer::TokenIssuer;
