use axum::http::HeaderMap;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: String,
    pub user_code: String,
    pub dept_id: Option<i64>,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PrincipalError {
    #[error("missing x-user-id")]
    MissingUserId,
}

impl Principal {
    pub fn from_headers(h: &HeaderMap) -> Result<Self, PrincipalError> {
        let user_id = h
            .get("x-user-id")
            .ok_or(PrincipalError::MissingUserId)?
            .to_str()
            .map_err(|_| PrincipalError::MissingUserId)?
            .to_string();
        let user_code = h
            .get("x-user-code")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let dept_id = h
            .get("x-user-dept")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok());
        let roles = h
            .get("x-user-roles")
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.split(',')
                    .filter(|x| !x.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let permissions = h
            .get("x-user-perms")
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.split(',')
                    .filter(|x| !x.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            user_id,
            user_code,
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
    async fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        Self::from_headers(&parts.headers).map_err(|_| axum::http::StatusCode::UNAUTHORIZED)
    }
}
