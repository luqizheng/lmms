use crate::service::{oauth_client_svc, refresh_svc};
use crate::TokenServerState;
use argon2::{PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

struct RefreshTokenInfo {
    sub: String,
    jti: String,
    exp: i64,
    iat: i64,
}

fn decode_refresh_token(
    token: &str,
    public_key_pem: &str,
    issuer: &str,
) -> Result<RefreshTokenInfo, StatusCode> {
    let public_key =
        RS256PublicKey::from_pem(public_key_pem).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let options = VerificationOptions {
        allowed_issuers: Some(std::collections::HashSet::from([issuer.to_string()])),
        accept_future: false,
        ..Default::default()
    };

    let claims = public_key
        .verify_token::<HashMap<String, serde_json::Value>>(token, Some(options))
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let sub = claims.subject.clone().unwrap_or_default();
    let jti = claims.jwt_id.clone().unwrap_or_default();
    let iat = claims.issued_at.map(|d| d.as_secs() as i64).unwrap_or(0);
    let exp = claims.expires_at.map(|d| d.as_secs() as i64).unwrap_or(0);

    Ok(RefreshTokenInfo { sub, jti, exp, iat })
}

#[utoipa::path(
    post,
    path = "/api/tokens/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "refreshed tokens", body = RefreshResponse),
        (status = 401, description = "invalid or expired"),
    ),
    tag = "tokens"
)]
pub async fn handle_refresh(
    State(state): State<TokenServerState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, (StatusCode, &'static str)> {
    let client = oauth_client_svc::find_by_id(&state.db, &req.client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

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

    let info = decode_refresh_token(&req.refresh_token, &state.jwt_public_key, &state.issuer_str)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired refresh_token"))?;

    let replayed = refresh_svc::is_marked(&state.db, &info.jti)
        .await
        .unwrap_or(false);
    if replayed {
        return Err((StatusCode::UNAUTHORIZED, "refresh token already used"));
    }

    let ttl = info.exp - info.iat;
    let _ = refresh_svc::mark(&state.db, &info.jti, ttl.max(0)).await;

    let claims_url = format!(
        "{}/internal/users/{}/claims",
        state.user_service_url, info.sub
    );
    let extra_claims: serde_json::Value =
        match crate::fetch_json(&state.http_client, &claims_url).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("refresh: user-service unreachable: {}", e);
                return Err((StatusCode::SERVICE_UNAVAILABLE, "user_service_unreachable"));
            }
        };

    let issuer = &state.issuer;
    let at = issuer
        .issue_access_token(
            &info.sub,
            &req.client_id,
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    let (rt, _family) = issuer
        .issue_refresh_token(&info.sub, client.refresh_token_ttl_sec)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(RefreshResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}
