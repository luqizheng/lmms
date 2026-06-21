use crate::error::{AppError, AppResult};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: String,
    pub dept_id: Option<String>,
    pub roles: Vec<String>,
}

impl Identity {
    pub fn from_headers(parts: &Parts) -> AppResult<Self> {
        let user_id = parts
            .headers
            .get("x-user-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or(AppError::Forbidden)?;
        let dept_id = parts
            .headers
            .get("x-user-dept")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let roles = parts
            .headers
            .get("x-user-roles")
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
