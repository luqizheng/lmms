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

#[derive(Debug)]
struct RefreshTokenInfo {
    sub: String,
    jti: String,
    exp: i64,
    iat: i64,
    /// Mandatory tenant binding (WP-LMMS-REFRESH-TID-01). Tokens without a
    /// well-formed integer `tid` claim are rejected at decode time — no
    /// default-tenant fallback, no legacy compatibility path.
    tid: i64,
    /// Issuance channel carried from the access/refresh family. `None` for
    /// legacy tokens — the rotated access token then stays purpose-less
    /// (contract: claim absent = legacy, downstream keeps old inference).
    purpose: Option<String>,
}

/// Multi-key refresh decode: try every JWKS-published public key (active
/// first, then retiring). Refresh families issued before a rotation keep
/// verifying until the key is retired out of the JWKS.
fn decode_refresh_token_multi(
    token: &str,
    public_key_pems: &[String],
    issuer: &str,
) -> Result<RefreshTokenInfo, StatusCode> {
    let options = VerificationOptions {
        allowed_issuers: Some(std::collections::HashSet::from([issuer.to_string()])),
        accept_future: false,
        ..Default::default()
    };

    for pem in public_key_pems {
        let public_key =
            RS256PublicKey::from_pem(pem).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Ok(claims) = public_key
            .verify_token::<HashMap<String, serde_json::Value>>(token, Some(options.clone()))
        {
            return claims_into_info(claims);
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

/// WP-LMMS-REFRESH-TID-01: `tid` must be present and a well-formed integer.
/// Signature-verified tokens missing it (pre-fix issuance) are rejected with
/// 401 — the caller must re-authenticate; we never fall back to a default
/// tenant.
fn claims_into_info(claims: JWTClaims<HashMap<String, serde_json::Value>>) -> Result<RefreshTokenInfo, StatusCode> {
    let sub = claims.subject.clone().unwrap_or_default();
    let jti = claims.jwt_id.clone().unwrap_or_default();
    let iat = claims.issued_at.map(|d| d.as_secs() as i64).unwrap_or(0);
    let exp = claims.expires_at.map(|d| d.as_secs() as i64).unwrap_or(0);
    let tid = claims
        .custom
        .get("tid")
        .and_then(|v| v.as_i64())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let purpose = claims
        .custom
        .get("purpose")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(RefreshTokenInfo {
        sub,
        jti,
        exp,
        iat,
        tid,
        purpose,
    })
}

/// WP-LMMS-REFRESH-TID-01: the tenant resolved by iam must match the tenant
/// bound into the refresh token. A mismatch means the user's tenant moved
/// after issuance — refuse with 401 instead of silently switching tenant
/// context. Also fails closed if iam omits `tenant_id`.
fn ensure_tenant_consistency(rt_tid: i64, claims: &serde_json::Value) -> Result<(), StatusCode> {
    let iam_tid = claims
        .get("tenant_id")
        .and_then(|v| v.as_i64())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if iam_tid != rt_tid {
        tracing::warn!(
            "refresh: tenant drift detected: refresh_token tid={} but iam tenant_id={}",
            rt_tid,
            iam_tid
        );
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/tokens/refresh",
    summary = "Refresh access token (single-use refresh_token)",
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

    let info = {
        let pub_pems = {
            let ks = state.keys.read().expect("keystore lock poisoned");
            ks.verify_public_pems()
        };
        decode_refresh_token_multi(&req.refresh_token, &pub_pems, &state.issuer_str)
    }
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
        "{}/internal/users/{}/claims?tenant_id={}",
        state.user_service_url, info.sub, info.tid
    );
    // 2026-11-05 review (P0-3): the claims endpoint enforces
    // `aud == iam-service`; forward the self-signed service-jwt instead of the
    // caller's user-jwt (whose aud is `coder-micro-services`).
    let iam_bearer = crate::mint_service_bearer_for_state(&state, "iam-service", 300)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "internal error"))?;
    let extra_claims: serde_json::Value = match crate::fetch_json(
        &state.http_client,
        "iam-service",
        &claims_url,
        Some(&iam_bearer),
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("refresh: user-service unreachable: {}", e);
            return Err((StatusCode::SERVICE_UNAVAILABLE, "user_service_unreachable"));
        }
    };

    // WP-LMMS-REFRESH-TID-01: refuse silent tenant context switching.
    ensure_tenant_consistency(info.tid, &extra_claims)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "invalid or expired refresh_token"))?;

    let issuer = &state.issuer;
    // Inherit the issuance channel from the refresh token family
    // (None for legacy tokens → purpose-less access token, see contract).
    let purpose = info.purpose.as_deref();
    let at = issuer
        .issue_access_token(
            &info.sub,
            &req.client_id,
            purpose,
            extra_claims,
            client.access_token_ttl_sec,
        )
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    // Rotation keeps the verified tenant binding — the new refresh token
    // carries the same tid as the one it replaces.
    let (rt, _family) = issuer
        .issue_refresh_token(&info.sub, info.tid, purpose, client.refresh_token_ttl_sec)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "token issue failed"))?;

    Ok(Json(RefreshResponse {
        access_token: at,
        refresh_token: rt,
        token_type: "Bearer".into(),
        expires_in: client.access_token_ttl_sec,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issuer::purpose;

    fn keypair() -> (RS256KeyPair, String) {
        let kp = RS256KeyPair::generate(2048).unwrap();
        let public_pem = kp.public_key().to_pem().unwrap();
        (kp, public_pem)
    }

    fn decode(public_pem: &str, token: &str) -> Result<RefreshTokenInfo, StatusCode> {
        decode_refresh_token_multi(token, &[public_pem.to_string()], "test-issuer")
    }

    #[test]
    fn decode_reads_purpose_and_tid_from_refresh_token() {
        let (kp, public_pem) = keypair();
        let issuer = crate::issuer::TokenIssuer::new(&kp.to_pem().unwrap(), "test-issuer").unwrap();
        let (token, _fam) = issuer
            .issue_refresh_token("user-1", 42, Some(purpose::AUTH_CODE), 3600)
            .unwrap();
        let info = decode(&public_pem, &token).expect("decode");
        assert_eq!(info.purpose.as_deref(), Some(purpose::AUTH_CODE));
        assert_eq!(info.tid, 42, "refresh token 必须携带签发时的租户绑定");
    }

    #[test]
    fn decode_legacy_refresh_token_without_tid_is_rejected() {
        // WP-LMMS-REFRESH-TID-01: pre-fix tokens carry no tid. They must be
        // rejected (401) instead of falling back to the default tenant.
        let kp = RS256KeyPair::generate(2048).unwrap();
        let public_pem = kp.public_key().to_pem().unwrap();
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some("user-1".into());
        claims.jwt_id = Some("legacy-jti".into());
        claims.expires_at = Some(
            jwt_simple::prelude::Duration::from_secs(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
        );
        claims.custom = HashMap::from([(
            "scope".to_string(),
            serde_json::Value::String("refresh".into()),
        )]);
        let legacy = kp.sign(claims).unwrap();
        let err = decode(&public_pem, &legacy).expect_err("缺 tid 必须拒绝");
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn decode_rejects_malformed_tid_type() {
        let kp = RS256KeyPair::generate(2048).unwrap();
        let public_pem = kp.public_key().to_pem().unwrap();
        let mut claims: JWTClaims<HashMap<String, serde_json::Value>> = JWTClaims::new();
        claims.subject = Some("user-1".into());
        claims.jwt_id = Some("bad-tid-jti".into());
        claims.expires_at = Some(
            jwt_simple::prelude::Duration::from_secs(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
        );
        claims.custom = HashMap::from([
            ("scope".to_string(), serde_json::Value::String("refresh".into())),
            ("tid".to_string(), serde_json::Value::String("7".into())),
        ]);
        let token = kp.sign(claims).unwrap();
        let err = decode(&public_pem, &token).expect_err("tid 类型非法必须拒绝");
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn tenant_consistency_accepts_matching_tenant() {
        let claims = serde_json::json!({"tenant_id": 42, "roles": ["member"]});
        assert!(ensure_tenant_consistency(42, &claims).is_ok());
    }

    #[test]
    fn tenant_consistency_rejects_tenant_drift() {
        let claims = serde_json::json!({"tenant_id": 7});
        let err = ensure_tenant_consistency(42, &claims).expect_err("租户漂移必须拒绝");
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn tenant_consistency_fails_closed_without_iam_tenant() {
        let claims = serde_json::json!({"roles": ["member"]});
        let err = ensure_tenant_consistency(42, &claims).expect_err("iam 缺 tenant_id 必须 fail-closed");
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn rotation_keeps_tenant_binding() {
        // 刷新轮换继承：decode 出的 tid 原样传入下一次签发，全程一致。
        let (kp, public_pem) = keypair();
        let issuer = crate::issuer::TokenIssuer::new(&kp.to_pem().unwrap(), "test-issuer").unwrap();
        let (token, _fam) = issuer.issue_refresh_token("user-1", 9, None, 3600).unwrap();
        let info = decode(&public_pem, &token).expect("decode");
        let (rotated, _fam2) = issuer
            .issue_refresh_token(&info.sub, info.tid, info.purpose.as_deref(), 3600)
            .unwrap();
        let rotated_info = decode(&public_pem, &rotated).expect("decode rotated");
        assert_eq!(rotated_info.tid, info.tid, "轮换后租户绑定不得丢失");
        assert!(rotated_info.purpose.is_none(), "purpose=None 轮换后保持 legacy 语义");
    }
}
