use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use crate::Principal;

#[derive(Debug, Clone, Copy)]
pub struct TenantScope(pub i64);

impl<S: Send + Sync> FromRequestParts<S> for TenantScope {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let p = Principal::from_request_parts(parts, state).await?;
        Ok(TenantScope(p.tenant_id))
    }
}
