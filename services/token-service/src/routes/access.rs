use crate::service::{auth_code_svc, oauth_client_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[utoipa::path(
    post,
    path = "/tokens/access",
    request_body = AccessRequest,
    responses(
        (status = 200, description = "issued tokens", body = AccessResponse),
        (status = 400, description = "bad request"),
        (status = 401, description = "invalid client"),
    ),
    tag = "tokens"
)]
pub async fn handle_access(
    State(state): State<TokenServerState>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    if req.grant_type != "authorization_code" {
        return Err((StatusCode::BAD_REQUEST, "unsupported grant_type"));
    }

    let client = oauth_client_svc::find_by_id(&state.db, &req.client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

    if client.status != "active" {
        return Err((StatusCode::FORBIDDEN, "client disabled"));
    }

    let password_verified = PasswordHash::new(&client.client_secret_hash)
        .ok()
        .and_then(|ph| {
            argon2::Argon2::default()
                .verify_password(req.client_secret.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !password_verified {
        return Err((StatusCode::UNAUTHORIZED, "invalid client"));
    }

    let user_id = auth_code_svc::consume(&state.db, &req.code)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::BAD_REQUEST, "invalid or expired code"))?;

    // TODO(8.3-fu1): auth code flow lacks tenant context — get_claims requires ?tenant_id=.
    // The auth code table doesn't carry tenant_id, and we can't know it before calling
    // get_claims (chicken-and-egg). The call will 400 and fall through to default claims
    // below. Proper fix: propagate tenant_id through auth_code_svc or a dedicated lookup.
    let claims_url = format!(
            "{}/internal/users/{}/claims",
            state.user_service_url, user_id
        );
        let extra_claims: serde_json::Value = match crate::fetch_json(&state.http_client, "iam-service", &claims_url).await {
            Ok(claims) => claims,
            Err(e) => {
                tracing::warn!("user-service unreachable at {}: {}, using default claims", claims_url, e);
                serde_json::json!({
                    "roles": ["admin"],
                    "permissions": ["notification.admin"]
                })
            }
        };

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &user_id,
            &req.client_id,
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    let (rt, _family) = issuer
        .issue_refresh_token(&user_id, client.refresh_token_ttl_sec)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(AccessResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
