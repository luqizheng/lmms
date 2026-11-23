//! Phase 5-B: `POST /tokens/service-jwt` — bootstrap-token auth, mints
//! short-lived RS256 JWT for service-to-service calls.
//!
//! Phase 2 演进 (2026-11-09): token 验证从内存 Vec 改为查 DB（`token_bootstrap_tokens`
//! 表），消除 SERVICE_BOOTSTRAP_TOKENS/OWNERS 双 CSV 无限增长 + 长度错位问题。
//!
//! 每个业务服务在 `token_bootstrap_tokens` 表有一行（owner 唯一）；handler 按明文
//! token 精确匹配 + status='active' + expires_at > NOW() 三条件筛选。

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::models::bootstrap_token::find_active_owner_by_token;
use crate::TokenServerState;

#[derive(Debug, Deserialize)]
pub struct ServiceJwtRequest {
    /// Target service name. Required. Mints JWT with `aud: <target>`.
    pub aud: String,
    /// Caller tenant. Required.
    pub tid: i64,
    /// Cross-tenant flag (2026-11-07): minted as the `ct` claim so a caller
    /// serving platform-admin class principals keeps its downstream
    /// CrossTenant scope. Absent/false = tenant-scoped (default).
    #[serde(default)]
    pub ct: bool,
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
    // 1. Extract bearer token.
    let auth = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "missing bearer".into()))?;

    // 2. Validate against DB (token = 明文精确匹配 + status=active + 未过期).
    let caller = match find_active_owner_by_token(&state.db, auth).await {
        Ok(Some(owner)) => owner,
        Ok(None) => return Err((StatusCode::UNAUTHORIZED, "invalid bootstrap".into())),
        Err(e) => {
            tracing::error!(error = %e, "bootstrap token db lookup failed");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "db error".into()));
        }
    };

    // 3. Validate request.
    if req.aud.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "aud required".into()));
    }

    // 4. Sign. `sub` = bootstrap owner from DB。
    let now = chrono::Utc::now().timestamp();
    let mut claims = serde_json::json!({
        "iss": "token-service",
        "sub": caller,
        "aud": req.aud,
        "tid": req.tid,
        "iat": now,
        "exp": now + SERVICE_JWT_TTL_SECS,
        "roles": ["service"],
        "purpose": crate::issuer::purpose::SERVICE,
    });
    if req.ct {
        claims["ct"] = serde_json::json!(true);
    }
    // kid-strict verifiers need the header kid to match the JWKS entry —
    // sign with the active keystore kid, not the legacy hard-coded "default".
    let access_token = digua_common_jwt::sign_token_with_kid(
        &state.active_private_pem(),
        &state
            .keys
            .read()
            .expect("keystore lock poisoned")
            .active()
            .kid,
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
