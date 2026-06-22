use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use crate::error::{AppError, AppResult};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: String,
    pub dept_id: Option<String>,
    pub roles: Vec<String>,
}

impl Identity {
    pub fn from_headers(parts: &Parts) -> AppResult<Self> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .ok_or(AppError::Forbidden)?
            .to_str()
            .map_err(|_| AppError::Forbidden)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AppError::Forbidden)?;

        Self::from_jwt_payload(token)
    }

    pub fn from_jwt_payload(token: &str) -> AppResult<Self> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(AppError::Forbidden);
        }

        let payload_b64 = parts[1];
        let payload_bytes = URL_SAFE_NO_PAD.decode(payload_b64)
            .map_err(|_| AppError::Forbidden)?;

        let payload: Value = serde_json::from_slice(&payload_bytes)
            .map_err(|_| AppError::Forbidden)?;

        let user_id = payload
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or(AppError::Forbidden)?
            .to_string();

        let dept_id = payload
            .get("dept_id")
            .and_then(|v| v.as_i64())
            .map(|v| v.to_string());

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

        Ok(Self {
            user_id,
            dept_id,
            roles,
        })
    }

    pub fn is_admin(&self) -> bool {
        self.roles.iter().any(|r| r == "notification.admin" || r == "admin")
    }
}

impl<S> FromRequestParts<S> for Identity
where
    S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Self::from_headers(parts)
    }
}