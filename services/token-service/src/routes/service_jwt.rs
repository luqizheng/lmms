//! Phase 5-B: `POST /tokens/service-jwt` — bootstrap-token auth, mints
//! short-lived RS256 JWT for service-to-service calls.
//!
//! Each business service has a pre-shared `SERVICE_BOOTSTRAP_TOKEN` (per
//! service, 32-byte random). On startup it calls this endpoint with the
//! token in `Authorization: Bearer …` to exchange for a short-lived
//! service-jwt (default 1h TTL, `aud: <caller-specified target>`).
//!
//! See spec §3.3 / plan Task B for design rationale.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use common_jwt::sign_token;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::TokenServerState;

#[derive(Debug, Deserialize)]
pub struct ServiceJwtRequest {
    /// Target service name. Required. Mints JWT with `aud: <target>`.
    pub aud: String,
    /// Caller tenant. Required.
    pub tid: i64,
}

#[derive(Debug, Serialize)]
pub struct ServiceJwtResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
}

const SERVICE_JWT_TTL_SECS: i64 = 3600;

pub async fn handle_service_jwt(
    State(state): State<TokenServerState>,
    headers: HeaderMap,
    Json(req): Json<ServiceJwtRequest>,
) -> Result<Json<ServiceJwtResponse>, (StatusCode, String)> {
    // 1. Validate bootstrap token.
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer".into()))?;
    if !state.bootstrap_tokens.iter().any(|t| t == auth) {
        return Err((StatusCode::UNAUTHORIZED, "invalid bootstrap".into()));
    }

    // 2. Validate request.
    if req.aud.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "aud required".into()));
    }

    // 3. Sign.
    let now = chrono::Utc::now().timestamp();
    let caller = std::env::var("SERVICE_NAME").unwrap_or_else(|_| "unknown".into());
    let claims = serde_json::json!({
        "iss": "token-service",
        "sub": caller,
        "aud": req.aud,
        "tid": req.tid,
        "iat": now,
        "exp": now + SERVICE_JWT_TTL_SECS,
        "roles": ["service"],
    });
    let access_token = sign_token(
        &state.private_key_pem,
        &claims,
        Duration::from_secs(SERVICE_JWT_TTL_SECS as u64),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("sign: {}", e)))?;

    Ok(Json(ServiceJwtResponse {
        access_token,
        token_type: "Bearer",
        expires_in: SERVICE_JWT_TTL_SECS,
    }))
}

pub fn routes(state: TokenServerState) -> Router {
    Router::new()
        .route(
            "/tokens/service-jwt",
            axum::routing::post(handle_service_jwt),
        )
        .with_state(state)
}
