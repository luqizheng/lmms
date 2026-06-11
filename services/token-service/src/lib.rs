use axum::{routing::{get, post}, Router};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::Request;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use redis::aio::MultiplexedConnection;

pub type TokenHttpClient = Client<HttpConnector, Full<Bytes>>;

pub async fn fetch_json(
    client: &TokenHttpClient,
    url: &str,
) -> Result<serde_json::Value, String> {
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
    pub client_store: std::sync::Arc<store::client::ClientStore>,
    pub issuer: std::sync::Arc<issuer::TokenIssuer>,
    pub issuer_str: String,
    pub redis_conn: MultiplexedConnection,
    pub redis_prefix: String,
    pub jwt_public_key: String,
    pub default_resource_ttl_sec: i64,
    pub http_client: TokenHttpClient,
    pub user_service_url: String,
}

pub mod config;
pub mod consul;
pub mod db;
pub mod issuer;
pub mod models;
pub mod routes;
pub mod secret;
pub mod store;

pub use config::{load_config, Config as TokenServerConfig, DatabaseCfg, DbType};
pub use consul::ConsulRegistrar;
pub use issuer::TokenIssuer;
pub use store::client::ClientStore;

pub async fn create_app(state: crate::TokenServerState) -> Router {
    Router::new()
        .route("/health", get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/tokens/access", post(routes::access::handle_access))
        .route("/api/tokens/refresh", post(routes::refresh::handle_refresh))
        .route("/api/tokens/revoke", post(routes::revoke::handle_revoke))
        .route("/api/tokens/resource", post(routes::resource::handle_resource))
        .route("/api/tokens/.well-known/jwks.json", get(routes::jwks::handle_jwks))
        .route("/.well-known/jwks.json", get(routes::jwks::handle_jwks))
        .route("/api/api-keys", post(routes::api_key::handle_create_api_key))
        .route(
            "/api/v1/tokens/user-jwt",
            post(routes::user_jwt::handle_user_jwt),
        )
        .with_state(state)
}

pub async fn create_test_app() -> Router {
    let health_route = Router::new().route(
        "/health",
        get(|| async { axum::Json(serde_json::json!({"status": "ok"})) }),
    );

    Router::new()
        .merge(health_route)
        .with_state(())
}
