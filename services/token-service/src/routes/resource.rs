use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ResourceRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub permission: String,
    pub ttl_sec: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ResourceResponse {
    pub resource_token: String,
    pub expires_in: i64,
}

pub async fn handle_resource(
    State(state): State<TokenServerState>,
    Json(req): Json<ResourceRequest>,
) -> Result<Json<ResourceResponse>, (StatusCode, &'static str)> {
    let ttl = req.ttl_sec.unwrap_or(state.default_resource_ttl_sec);

    let token = state
        .issuer
        .issue_resource_token(
            "resource-service",
            &req.resource_type,
            &req.resource_id,
            &req.permission,
            ttl,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(ResourceResponse {
        resource_token: token,
        expires_in: ttl,
    }))
}
