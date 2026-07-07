//! Client for the IAM internal member endpoints (Phase 4.3-fu2).
//!
//! `by_role` / `by_dept` query `GET /iam/internal/users/by-{role,dept}/{id}`.
//! `expand_scopes` walks `ScopeType::{User,Role,Dept}` and resolves each into
//! the unioned user-id set.
//!
//! tenant_id is passed as a URL query parameter; the IAM service enforces
//! tenant scoping server-side.

use bytes::Bytes;
use common_http::{ResilientClient, ResilientError};
use jwt_ctx::TenantScope;
use reqwest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Method,
};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

use crate::error::AppError;
use crate::models::ScopeType;

const IAM_SERVICE: &str = "iam";

#[derive(Clone)]
pub struct IamMemberClient {
    http: Arc<ResilientClient>,
    base_url: String,
}

impl IamMemberClient {
    pub fn new(http: Arc<ResilientClient>, base_url: String) -> Self {
        Self { http, base_url }
    }

    pub async fn by_role(
        &self,
        role_id: &str,
        tenant_id: i64,
        user_jwt: Option<&str>,
    ) -> Result<Vec<String>, AppError> {
        let url = format!(
            "{}/iam/internal/users/by-role/{}?size=1000&tenant_id={}",
            self.base_url, role_id, tenant_id
        );
        self.fetch_items(&url, user_jwt).await
    }

    pub async fn by_dept(
        &self,
        dept_id: &str,
        tenant_id: i64,
        user_jwt: Option<&str>,
    ) -> Result<Vec<String>, AppError> {
        let url = format!(
            "{}/iam/internal/users/by-dept/{}?size=1000&tenant_id={}",
            self.base_url, dept_id, tenant_id
        );
        self.fetch_items(&url, user_jwt).await
    }

    pub async fn expand_scopes(
        &self,
        scopes: &[(ScopeType, String)],
        scope: &TenantScope,
        user_jwt: Option<&str>,
    ) -> Result<HashSet<String>, AppError> {
        let tenant_id = scope.tenant_id().unwrap_or(1);
        let mut out: HashSet<String> = HashSet::new();
        for (t, id) in scopes {
            match t {
                ScopeType::User => {
                    out.insert(id.clone());
                }
                ScopeType::Role => {
                    out.extend(self.by_role(id, tenant_id, user_jwt).await?);
                }
                ScopeType::Dept => {
                    out.extend(self.by_dept(id, tenant_id, user_jwt).await?);
                }
            }
        }
        Ok(out)
    }

    async fn fetch_items(&self, url: &str, user_jwt: Option<&str>) -> Result<Vec<String>, AppError> {
        let mut headers = HeaderMap::new();
        if let Some(bearer) = user_jwt {
            if let Ok(v) = HeaderValue::from_str(&format!("Bearer {bearer}")) {
                headers.insert(AUTHORIZATION, v);
            }
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

        let body: UserList = serde_json::from_slice(&resp.body).map_err(|e| {
            AppError::external("IAM_FAIL", format!("decode response: {}", e))
        })?;
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
