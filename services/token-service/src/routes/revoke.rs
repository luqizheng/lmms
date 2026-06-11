use crate::service::revoke_svc;
use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token_jti: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize)]
pub struct RevokeResponse {
    pub success: bool,
}

pub async fn handle_revoke(
    State(state): State<TokenServerState>,
    Json(req): Json<RevokeRequest>,
) -> Result<Json<RevokeResponse>, (StatusCode, &'static str)> {
    revoke_svc::revoke(&state.db, &req.token_jti, req.expires_in.max(0))
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;

    tracing::info!("revoked token jti={}", req.token_jti);
    Ok(Json(RevokeResponse { success: true }))
}
