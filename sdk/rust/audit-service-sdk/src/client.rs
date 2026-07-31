use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::SdkError;

use crate::model::{
    LoginLogQuery, OperationLogQuery, PagedLoginLogs, PagedOperationLogs, RetentionPolicy,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct AuditClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl AuditClient {
    pub fn new<F>(base_url: impl Into<String>, bearer_supplier: F) -> Self
    where
        F: Fn() -> Option<String> + Send + Sync + 'static,
    {
        let url = base_url.into();
        let base_url = url.trim_end_matches('/').to_string();
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

    pub async fn query_login_logs(
        &self,
        q: &LoginLogQuery,
    ) -> Result<PagedLoginLogs, SdkError> {
        let url = format!("{}/audit/login?{}", self.base_url, q.to_query_string());
        let resp = self
            .http
            .get(&url, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "audit query_login_logs")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedLoginLogs: {}", e)))
    }

    pub async fn query_operation_logs(
        &self,
        q: &OperationLogQuery,
    ) -> Result<PagedOperationLogs, SdkError> {
        let url = format!("{}/audit/operation?{}", self.base_url, q.to_query_string());
        let resp = self
            .http
            .get(&url, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "audit query_operation_logs")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedOperationLogs: {}", e)))
    }

    pub async fn trigger_cleanup(&self) -> Result<serde_json::Value, SdkError> {
        let url = format!("{}/audit/retention/cleanup", self.base_url);
        let resp = self
            .http
            .post(&url, "{}", (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "audit trigger_cleanup")?;
        if resp.body.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode cleanup response: {}", e)))
    }

    pub async fn get_retention_policy(&self) -> Result<RetentionPolicy, SdkError> {
        let url = format!("{}/audit/retention/policy", self.base_url);
        let resp = self
            .http
            .get(&url, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "audit get_retention_policy")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RetentionPolicy: {}", e)))
    }

    pub async fn update_retention_policy(
        &self,
        policy: &RetentionPolicy,
    ) -> Result<RetentionPolicy, SdkError> {
        let url = format!("{}/audit/retention/policy", self.base_url);
        let body = serde_json::to_string(policy)
            .map_err(|e| SdkError::Network(format!("serialize RetentionPolicy: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "audit update_retention_policy")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode RetentionPolicy: {}", e)))
    }
}

fn ensure_success(resp: &digua_http_client::transport::HttpResponse, op: &str) -> Result<(), SdkError> {
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