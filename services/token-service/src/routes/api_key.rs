use crate::store::client::Client;
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
    use argon2::password_hash::{rand_core::OsRng, SaltString};
    use argon2::PasswordHasher;
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2::Argon2::default()
        .hash_password(client_secret.as_bytes(), &salt)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "hash error"))?
        .to_string();

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

    // 将新 client 追加到内存中（磁盘持久化由管理员手动处理）
    {
        state.client_store.add_client(new_client).await;
    }

    tracing::info!("created api key client_id={}", client_id);

    Ok(Json(ApiKeyCreated {
        api_key_id: client_id,
        api_key_secret: client_secret,
    }))
}
