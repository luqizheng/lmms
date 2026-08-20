//! Client for the IAM internal member endpoints (Phase 4.3-fu2).
//!
//! ## Endpoints used
//!
//! - `by_role` / `by_dept`: `GET /iam/internal/users/by-{role,dept}/{id}` —
//!   **DEPRECATED** by Phase 5B-fu3 (notification → iam 事件化本地投影).
//!   Kept for `CrossTenant` (platform-admin) reads and as fallback path during
//!   bootstrap grace window. Use `fetch_membership_bootstrap` + local
//!   `ntf_user_projection` table for the hot path.
//! - `expand_scopes`: walks `ScopeType::{User,Role,Dept}` and resolves each
//!   into the unioned user-id set via `by_role` / `by_dept`. **DEPRECATED** —
//!   `db.resolve_users_by_scopes` replaces this on the hot path.
//! - `fetch_membership_bootstrap` (Phase 5B-fu3): one-shot GET
//!   `/iam/internal/membership-bootstrap` returning full user→{role_ids, dept_id}
//!   map for the caller's tenant (tenant_id from JWT tid claim, not query param).
//!
//! ## Service-jwt
//!
//! Phase 5-B: outbound calls carry a service-jwt (aud=iam-service) fetched from
//! `ServiceJwtCache` on each request.

use bytes::Bytes;
use digua_common_http::{ResilientClient, ResilientError};
use digua_common_jwt::ServiceJwtCache;
use digua_jwt_ctx::TenantScope;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Method,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::warn;

use crate::error::AppError;
use crate::models::ScopeType;

const IAM_SERVICE: &str = "iam";

#[derive(Clone)]
pub struct IamMemberClient {
    http: Arc<ResilientClient>,
    base_url: String,
    service_jwt: Arc<ServiceJwtCache>,
}

impl IamMemberClient {
    pub fn new(
        http: Arc<ResilientClient>,
        base_url: String,
        service_jwt: Arc<ServiceJwtCache>,
    ) -> Self {
        Self {
            http,
            base_url,
            service_jwt,
        }
    }

    /// DEPRECATED (Phase 5B-fu3): use `db.resolve_users_by_scopes` (local
    /// `ntf_user_projection`). Kept for `CrossTenant` (platform-admin) reads
    /// where local projection is single-tenant only. Removal planned for
    /// Roadmap 2.0 cleanup phase.
    #[allow(clippy::deprecated_semver)] // `5B-fu3` is an internal phase tag, not a crate version.
    #[deprecated(
        since = "5B-fu3",
        note = "use db.resolve_users_by_scopes (local projection); kept for CrossTenant (platform admin) only"
    )]
    pub async fn by_role(&self, role_id: &str, tenant_id: i64) -> Result<Vec<String>, AppError> {
        warn!(target: "deprecated", "IamMemberClient::by_role called; migrate to ntf_user_projection. Removal planned Roadmap 2.0.");
        let url = format!(
            "{}/iam/internal/users/by-role/{}?size=1000&tenant_id={}",
            self.base_url, role_id, tenant_id
        );
        self.fetch_items(&url).await
    }

    /// DEPRECATED (Phase 5B-fu3): see `by_role`.
    #[allow(clippy::deprecated_semver)] // `5B-fu3` is an internal phase tag, not a crate version.
    #[deprecated(
        since = "5B-fu3",
        note = "use db.resolve_users_by_scopes (local projection); kept for CrossTenant (platform admin) only"
    )]
    pub async fn by_dept(&self, dept_id: &str, tenant_id: i64) -> Result<Vec<String>, AppError> {
        warn!(target: "deprecated", "IamMemberClient::by_dept called; migrate to ntf_user_projection. Removal planned Roadmap 2.0.");
        let url = format!(
            "{}/iam/internal/users/by-dept/{}?size=1000&tenant_id={}",
            self.base_url, dept_id, tenant_id
        );
        self.fetch_items(&url).await
    }

    /// DEPRECATED (Phase 5B-fu3): wraps the deprecated `by_role`/`by_dept`.
    #[allow(clippy::deprecated_semver)] // `5B-fu3` is an internal phase tag, not a crate version.
    #[deprecated(
        since = "5B-fu3",
        note = "use db.resolve_users_by_scopes (local projection)"
    )]
    pub async fn expand_scopes(
        &self,
        scopes: &[(ScopeType, String)],
        scope: &TenantScope,
    ) -> Result<HashSet<String>, AppError> {
        let tenant_id = scope.tenant_id().unwrap_or(1);
        let mut out: HashSet<String> = HashSet::new();
        for (t, id) in scopes {
            match t {
                ScopeType::User => {
                    out.insert(id.clone());
                }
                ScopeType::Role => {
                    #[allow(deprecated)]
                    out.extend(self.by_role(id, tenant_id).await?);
                }
                ScopeType::Dept => {
                    #[allow(deprecated)]
                    out.extend(self.by_dept(id, tenant_id).await?);
                }
            }
        }
        Ok(out)
    }

    /// Phase 5B-fu3: one-shot bootstrap. Calls
    /// `GET /iam/internal/membership-bootstrap` (tenant_id from JWT tid claim
    /// on the service-jwt, **not** a query parameter — the bootstrap is the
    /// caller's own tenant projection, not a "query any tenant" endpoint).
    pub async fn fetch_membership_bootstrap(&self) -> Result<MembershipBootstrap, AppError> {
        let url = format!("{}/iam/internal/membership-bootstrap", self.base_url);
        let token = self
            .service_jwt
            .get("iam-service")
            .await
            .map_err(|e| AppError::external("IAM_FAIL", format!("service-jwt: {}", e)))?;
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(AUTHORIZATION, v);
        }
        let resp = self
            .http
            .call(IAM_SERVICE, Method::GET, &url, &headers, Bytes::new())
            .await
            .map_err(|e: ResilientError| {
                AppError::external("IAM_FAIL", format!("upstream call failed: {:?}", e))
            })?;
        if !resp.status.is_success() {
            return Err(AppError::external(
                "IAM_FAIL",
                format!("status {}", resp.status.as_u16()),
            ));
        }
        let body: MembershipBootstrap = serde_json::from_slice(&resp.body)
            .map_err(|e| AppError::external("IAM_FAIL", format!("decode response: {}", e)))?;
        Ok(body)
    }

    async fn fetch_items(&self, url: &str) -> Result<Vec<String>, AppError> {
        let token = self
            .service_jwt
            .get("iam-service")
            .await
            .map_err(|e| AppError::external("IAM_FAIL", format!("service-jwt: {}", e)))?;
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(AUTHORIZATION, v);
        }
        let resp = self
            .http
            .call(IAM_SERVICE, Method::GET, url, &headers, Bytes::new())
            .await
            .map_err(|e: ResilientError| {
                AppError::external("IAM_FAIL", format!("upstream call failed: {:?}", e))
            })?;

        if !resp.status.is_success() {
            return Err(AppError::external(
                "IAM_FAIL",
                format!("status {}", resp.status.as_u16()),
            ));
        }

        let body: UserList = serde_json::from_slice(&resp.body)
            .map_err(|e| AppError::external("IAM_FAIL", format!("decode response: {}", e)))?;
        Ok(body.items)
    }
}

#[derive(Deserialize)]
struct UserList {
    items: Vec<String>,
    #[serde(default)]
    _total: i64,
    #[serde(default)]
    _truncated: bool,
}

/// Phase 5B-fu3: bootstrap snapshot returned by
/// `GET /iam/internal/membership-bootstrap`. `tenant_id` mirrors the JWT tid
/// claim of the caller; `mappings` is user_id → membership record.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MembershipBootstrap {
    #[serde(default)]
    pub tenant_id: i64,
    #[serde(default)]
    pub mappings: HashMap<String, MembershipUserRow>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MembershipUserRow {
    #[serde(default)]
    pub role_ids: Vec<String>,
    #[serde(default)]
    pub dept_id: Option<String>,
}
