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

/// `POST /tokens/resource` 路由构造器。
///
/// WP-LMMS-HARDEN-01-R1：本端点曾是签发类端点中唯一无认证的——任何能发包的
/// 调用方都能让平台签名密钥签出任意 resource_type/resource_id/permission 的
/// JWT（公开签名神谕）。现与 `/tokens/user-jwt`、`/admin/*` 一致，要求
/// service-jwt（aud=token-service）准入。
pub fn routes(state: crate::TokenServerState) -> axum::Router {
    axum::Router::new()
        .route("/tokens/resource", axum::routing::post(handle_resource))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            digua_authz::verify_aud_principal_middleware::<crate::TokenServerState>(
                "token-service",
            ),
        ))
        .with_state(state)
}
