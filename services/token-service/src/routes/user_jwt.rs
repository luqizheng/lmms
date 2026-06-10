use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct UserJwtRequest {
    pub user_id: String,
    pub user_code: String,
    #[serde(default)]
    pub claims: Value,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct UserJwtResponse {
    pub access_token: String,
    pub expires_in: u64,
}

pub async fn handle_user_jwt(
    State(state): State<TokenServerState>,
    Json(req): Json<UserJwtRequest>,
) -> Result<Json<UserJwtResponse>, (StatusCode, &'static str)> {
    let ttl = req.ttl_secs.unwrap_or(3600);
    let access_token = state
        .issuer
        .issue_user_jwt(&req.user_id, &req.user_code, req.claims, ttl)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "user jwt issue failed"))?;
    Ok(Json(UserJwtResponse {
        access_token,
        expires_in: ttl,
    }))
}
