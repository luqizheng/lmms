use axum::http::HeaderMap;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::Value;
use std::collections::HashSet;

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

#[derive(Debug, thiserror::Error)]
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

impl Principal {
    pub fn from_headers(h: &HeaderMap) -> Result<Self, PrincipalError> {
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

    pub fn from_jwt_payload(token: &str) -> Result<Self, PrincipalError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(PrincipalError::InvalidJwtFormat);
        }
        
        let payload_b64 = parts[1];
        let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64)
            .map_err(|_| PrincipalError::InvalidBase64)?;
        
        let payload: Value = serde_json::from_slice(&payload_bytes)
            .map_err(|_| PrincipalError::InvalidJson)?;
        
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

        let dept_id = payload
            .get("dept_id")
            .and_then(|v| v.as_i64());
        
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

use axum::extract::FromRequestParts;
use axum::http::request::Parts;

impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = axum::http::StatusCode;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Self::from_headers(&parts.headers).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
    }
}