use std::sync::Arc;

use kudi_http_client::client::KudiWebClient;
use kudi_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    AccessRequest, AccessResponse, ApiKeyCreated, CreateApiKeyRequest, ListApiKeysQuery,
    ListApiKeysResponse, RefreshRequest, RefreshResponse, RevokeRequest, RevokeResponse,
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
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token access")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode AccessResponse: {}", e)))
    }

    pub async fn refresh(&self, req: &RefreshRequest) -> Result<RefreshResponse, SdkError> {
        let url = format!("{}/tokens/refresh", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize RefreshRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token refresh")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RefreshResponse: {}", e)))
    }

    pub async fn revoke(&self, req: &RevokeRequest) -> Result<RevokeResponse, SdkError> {
        let url = format!("{}/tokens/revoke", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize RevokeRequest: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
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
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "token create_api_key")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ApiKeyCreated: {}", e)))
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
        401 | 403 => SdkError::Auth { status, message: msg, details: Some(body) },
        429 => SdkError::RateLimit { message: msg, details: Some(body) },
        400 | 422 => SdkError::Validation { status, message: msg, details: Some(body) },
        500..=599 => SdkError::Server { status, message: msg, details: Some(body) },
        _ => SdkError::Network(format!("{}: unexpected status {}", op, status)),
    })
}