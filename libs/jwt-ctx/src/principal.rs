use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use common_jwt::{verify_token, JWKSCache, VerifyError};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: String,
    pub user_code: String,
    pub tenant_id: i64,
    pub cross_tenant: bool,
    pub dept_id: Option<i64>,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

#[derive(Error, Debug)]
pub enum PrincipalError {
    #[error("missing authorization header")]
    MissingAuthorization,
    #[error("invalid authorization header format")]
    InvalidAuthorizationFormat,
    #[error("invalid jwt format")]
    InvalidJwtFormat,
    #[error("invalid base64 payload")]
    InvalidBase64,
    #[error("invalid json payload")]
    InvalidJson,
    #[error("missing sub claim")]
    MissingSub,
    #[error("missing tid claim")]
    MissingTenantId,
}

#[derive(Error, Debug)]
pub enum PrincipalVerifyError {
    #[error("missing authorization header")]
    MissingAuthorization,
    #[error("invalid authorization header format")]
    InvalidAuthorizationFormat,
    #[error("verification failed: {0}")]
    VerifyFailed(#[from] VerifyError),
    #[error("missing sub claim")]
    MissingSub,
    #[error("missing tid claim")]
    MissingTenantId,
}

impl Principal {
    /// Phase 5-A-2 path: pure decode, no signature verification.
    /// Kept for unit tests / mock contexts where the test owns the
    /// private key. Production code should use `from_token`.
    #[deprecated(since = "0.1.0", note = "use `from_token` for production; this function performs no signature verification")]
    pub fn from_jwt_payload(token: &str) -> Result<Self, PrincipalError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(PrincipalError::InvalidJwtFormat);
        }
        let payload_b64 = parts[1];
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|_| PrincipalError::InvalidBase64)?;
        let payload: Value =
            serde_json::from_slice(&payload_bytes).map_err(|_| PrincipalError::InvalidJson)?;
        Self::from_claims(payload)
    }

    /// Phase 5-B: decode + verify against a JWKSCache.
    /// `expected_aud`: Some forces aud match; None skips (gateway path).
    pub async fn from_token(
        token: &str,
        cache: &JWKSCache,
        expected_aud: Option<&str>,
    ) -> Result<Self, PrincipalVerifyError> {
        let claims = verify_token(token, cache, expected_aud)
            .await
            .map_err(PrincipalVerifyError::VerifyFailed)?;
        Self::from_claims(claims).map_err(|e| match e {
            PrincipalError::MissingSub => PrincipalVerifyError::MissingSub,
            PrincipalError::MissingTenantId => PrincipalVerifyError::MissingTenantId,
            other => PrincipalVerifyError::VerifyFailed(VerifyError::Malformed(other.to_string())),
        })
    }

    pub fn from_headers(h: &axum::http::HeaderMap) -> Result<Self, PrincipalError> {
        let auth_header = h
            .get("Authorization")
            .ok_or(PrincipalError::MissingAuthorization)?
            .to_str()
            .map_err(|_| PrincipalError::InvalidAuthorizationFormat)?;
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(PrincipalError::InvalidAuthorizationFormat)?;
        Self::from_jwt_payload(token)
    }

    fn from_claims(payload: Value) -> Result<Self, PrincipalError> {
        let user_id = payload
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or(PrincipalError::MissingSub)?
            .to_string();

        let user_code = payload
            .get("user_code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tenant_id = payload
            .get("tid")
            .and_then(|v| v.as_i64())
            .ok_or(PrincipalError::MissingTenantId)?;

        let cross_tenant = payload
            .get("ct")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let dept_id = payload.get("dept_id").and_then(|v| v.as_i64());

        let roles = payload
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let permissions = payload
            .get("permissions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            user_id,
            user_code,
            tenant_id,
            cross_tenant,
            dept_id,
            roles,
            permissions,
        })
    }

    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    pub fn has_permission(&self, perm: &str) -> bool {
        self.permissions.contains(perm)
    }
}

/// Phase 5-B: any state that holds a JWKSCache can be the source of
/// `Principal` extraction. The state type (e.g. iam AppState, gateway
/// AppState) implements this trait to expose the cache.
pub trait PrincipalStateProvider: Send + Sync {
    fn jwks(&self) -> Arc<JWKSCache>;
}

impl<T: PrincipalStateProvider> PrincipalStateProvider for Arc<T> {
    fn jwks(&self) -> Arc<JWKSCache> {
        (**self).jwks()
    }
}

/// Phase 5-B: FromRequestParts becomes async and uses JWKSCache for verify.
/// `expected_aud` is `None` here — gateway path accepts any valid token
/// and the per-route middleware (in `authz`) enforces aud when needed.
impl<S> FromRequestParts<S> for Principal
where
    S: PrincipalStateProvider,
{
    type Rejection = axum::http::StatusCode;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
        let cache = state.jwks();
        Self::from_token(token, &cache, None)
            .await
            .map_err(|err| {
                tracing::warn!(error = %err, "jwt verification failed, returning 401");
                axum::http::StatusCode::UNAUTHORIZED
            })
    }
}
