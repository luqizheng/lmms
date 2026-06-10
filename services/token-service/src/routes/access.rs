use crate::store::redis;
use crate::TokenServerState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Serialize)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

pub async fn handle_access(
    State(state): State<TokenServerState>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<AccessResponse>, (StatusCode, &'static str)> {
    if req.grant_type != "authorization_code" {
        return Err((StatusCode::BAD_REQUEST, "unsupported grant_type"));
    }

    // 1. 验证 client 身份
    let client = state
        .client_store
        .find_by_id(&req.client_id)
        .await
        .ok_or((StatusCode::UNAUTHORIZED, "invalid client"))?;

    if client.status != "active" {
        return Err((StatusCode::FORBIDDEN, "client disabled"));
    }

    // 验证 client_secret
    use argon2::{PasswordHash, PasswordVerifier};
    let password_verified = PasswordHash::new(&client.client_secret_hash)
        .ok()
        .and_then(|ph| {
            argon2::Argon2::default()
                .verify_password(req.client_secret.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !password_verified {
        return Err((StatusCode::UNAUTHORIZED, "invalid client_secret"));
    }

    // 2. 验证 authorization code
    let mut conn = state.redis_conn.clone();
    let user_id = redis::get_code(&mut conn, &state.redis_prefix, &req.code)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "redis error"))?
        .ok_or((StatusCode::BAD_REQUEST, "invalid or expired code"))?;

    // 3. 标记 code 为已使用
    redis::del_code(&mut conn, &state.redis_prefix, &req.code)
        .await
        .ok();

    // 4. 拉 user-service 业务 claims
    let claims_url = format!(
        "{}/internal/users/{}/claims",
        state.user_service_url, user_id
    );
    let extra_claims: serde_json::Value = crate::fetch_json(&state.http_client, &claims_url)
        .await
        .map_err(|e| {
            tracing::warn!("user-service unreachable at {}: {}", claims_url, e);
            (StatusCode::SERVICE_UNAVAILABLE, "user_service_unreachable")
        })?;

    // 5. 签发 token 对
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
