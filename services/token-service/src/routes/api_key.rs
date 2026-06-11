use crate::models::oauth_client::Client;
use crate::service::oauth_client_svc;
use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiKeyCreated {
    pub api_key_id: String,
    pub api_key_secret: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: Option<i64>,
    pub refresh_token_ttl_sec: Option<i64>,
}

pub async fn handle_create_api_key(
    State(state): State<TokenServerState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyCreated>, (StatusCode, &'static str)> {
    let client_id = uuid::Uuid::new_v4().to_string();
    let client_secret = uuid::Uuid::new_v4().to_string();
    let hash = crate::secret::hash_client_secret(&client_secret);

    let new_client = Client {
        client_id: client_id.clone(),
        client_secret_hash: hash,
        name: req.name,
        permissions: req.permissions,
        grant_types: req.grant_types,
        access_token_ttl_sec: req.access_token_ttl_sec.unwrap_or(3600),
        refresh_token_ttl_sec: req.refresh_token_ttl_sec.unwrap_or(2592000),
        status: "active".into(),
    };

    oauth_client_svc::insert(&state.db, &new_client)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    tracing::info!("created api key client_id={}", client_id);

    Ok(Json(ApiKeyCreated {
        api_key_id: client_id,
        api_key_secret: client_secret,
    }))
}
