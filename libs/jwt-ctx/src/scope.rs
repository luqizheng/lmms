use crate::{Principal, PrincipalStateProvider};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantScope {
    Single(i64),
    CrossTenant,
}

impl TenantScope {
    pub fn tenant_id(&self) -> Option<i64> {
        match self {
            Self::Single(t) => Some(*t),
            Self::CrossTenant => None,
        }
    }
}

impl<S: Send + Sync + PrincipalStateProvider> FromRequestParts<S> for TenantScope {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let p = Principal::from_request_parts(parts, state).await?;
        Ok(if p.cross_tenant {
            TenantScope::CrossTenant
        } else {
            TenantScope::Single(p.tenant_id)
        })
    }
}
