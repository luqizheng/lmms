use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    AccessRequest, AccessResponse, ApiKeyCreated, CreateApiKeyRequest,
    CreateServiceJwtGrantRequest, ListApiKeysQuery, ListApiKeysResponse,
    ListServiceJwtGrantsQuery, ListServiceJwtGrantsResponse, RefreshRequest, RefreshResponse,
    RevokeRequest, RevokeResponse, ServiceJwtGrant, SetServiceJwtGrantStatusRequest,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct TokenClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl TokenClient {
    pub fn new<F>(base_url: impl Into<String>, bearer_supplier: F) -> Self
    where
        F: Fn() -> Option<String> + Send + Sync + 'static,
    {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let http = KudiWebClient::builder()
            .build()
            .expect("kudi-http-client: reqwest build");
        Self {
            base_url,
            bearer: Arc::new(bearer_supplier),
            http,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn access(&self, req: &AccessRequest) -> Result<AccessResponse, SdkError> {
        let url = format!("{}/tokens/access", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize AccessRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token access")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode AccessResponse: {}", e)))
    }

    pub async fn refresh(&self, req: &RefreshRequest) -> Result<RefreshResponse, SdkError> {
        let url = format!("{}/tokens/refresh", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize RefreshRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token refresh")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RefreshResponse: {}", e)))
    }

    pub async fn revoke(&self, req: &RevokeRequest) -> Result<RevokeResponse, SdkError> {
        let url = format!("{}/tokens/revoke", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize RevokeRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token revoke")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RevokeResponse: {}", e)))
    }

    pub async fn list_api_keys(
        &self,
        q: &ListApiKeysQuery,
    ) -> Result<ListApiKeysResponse, SdkError> {
        let url = format!("{}/api-keys?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token list_api_keys")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ListApiKeysResponse: {}", e)))
    }

    pub async fn create_api_key(
        &self,
        req: &CreateApiKeyRequest,
    ) -> Result<ApiKeyCreated, SdkError> {
        let url = format!("{}/api-keys", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateApiKeyRequest: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token create_api_key")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ApiKeyCreated: {}", e)))
    }

    // ── Service JWT audience grants（管理面，service-jwt aud=token-service）──

    /// 分页查询 owner → audience 授权关系。
    pub async fn list_service_jwt_grants(
        &self,
        q: &ListServiceJwtGrantsQuery,
    ) -> Result<ListServiceJwtGrantsResponse, SdkError> {
        let url = format!(
            "{}/admin/service-jwt-grants?{}",
            self.base_url,
            q.to_query_string()
        );
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token list_service_jwt_grants")?;
        serde_json::from_str(&resp.body).map_err(|e| {
            SdkError::Network(format!("decode ListServiceJwtGrantsResponse: {}", e))
        })
    }

    /// 查单条授权（不存在 → `SdkError::Validation`，见 [`ensure_success`] 映射）。
    pub async fn get_service_jwt_grant(
        &self,
        owner: &str,
        audience: &str,
    ) -> Result<ServiceJwtGrant, SdkError> {
        let url = format!("{}/admin/service-jwt-grants/{}/{}", self.base_url, owner, audience);
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token get_service_jwt_grant")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ServiceJwtGrant: {}", e)))
    }

    /// 创建授权。重复 → HTTP 409（见 [`ensure_success`] 的状态码映射）。
    pub async fn create_service_jwt_grant(
        &self,
        req: &CreateServiceJwtGrantRequest,
    ) -> Result<ServiceJwtGrant, SdkError> {
        let url = format!("{}/admin/service-jwt-grants", self.base_url);
        let body = serde_json::to_string(req).map_err(|e| {
            SdkError::Network(format!("serialize CreateServiceJwtGrantRequest: {}", e))
        })?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token create_service_jwt_grant")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ServiceJwtGrant: {}", e)))
    }

    /// 启用 / 禁用授权（`status`: `active` | `disabled`；system_managed 行 → 409）。
    pub async fn set_service_jwt_grant_status(
        &self,
        owner: &str,
        audience: &str,
        req: &SetServiceJwtGrantStatusRequest,
    ) -> Result<ServiceJwtGrant, SdkError> {
        let url = format!(
            "{}/admin/service-jwt-grants/{}/{}/status",
            self.base_url, owner, audience
        );
        let body = serde_json::to_string(req).map_err(|e| {
            SdkError::Network(format!("serialize SetServiceJwtGrantStatusRequest: {}", e))
        })?;
        let resp = self
            .http
            .put(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "token set_service_jwt_grant_status")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ServiceJwtGrant: {}", e)))
    }

    /// 删除授权（不存在/已删 → 404；system_managed 行 → 409）。
    pub async fn delete_service_jwt_grant(
        &self,
        owner: &str,
        audience: &str,
    ) -> Result<(), SdkError> {
        let url = format!("{}/admin/service-jwt-grants/{}/{}", self.base_url, owner, audience);
        let resp = self.http.delete(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token delete_service_jwt_grant")
    }
}

fn ensure_success(resp: &HttpResponse, op: &str) -> Result<(), SdkError> {
    let status = resp.status;
    if (200..300).contains(&status) {
        return Ok(());
    }
    let body = resp.body.clone();
    let msg = format!("{}: HTTP {}", op, status);
    Err(match status {
        401 | 403 => SdkError::Auth {
            status,
            message: msg,
            details: Some(body),
        },
        429 => SdkError::RateLimit {
            message: msg,
            details: Some(body),
        },
        // 404（不存在）/ 409（重复创建、system_managed 保护）也是确定性客户端类
        // 错误——归 Validation，status 字段可供调用方区分具体情形。
        400 | 404 | 409 | 422 => SdkError::Validation {
            status,
            message: msg,
            details: Some(body),
        },
        500..=599 => SdkError::Server {
            status,
            message: msg,
            details: Some(body),
        },
        _ => SdkError::Network(format!("{}: unexpected status {}", op, status)),
    })
}
