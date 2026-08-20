use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    ConfigItem, CreateConfigItem, CreateNamespace, ListItemsQuery, ListNamespacesQuery, Namespace,
    PagedConfigItems, PagedNamespaces,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct ConfigClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl ConfigClient {
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

    pub async fn list_namespaces(
        &self,
        q: &ListNamespacesQuery,
    ) -> Result<PagedNamespaces, SdkError> {
        let url = format!("{}/config/namespace?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "config list_namespaces")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedNamespaces: {}", e)))
    }

    pub async fn create_namespace(&self, req: &CreateNamespace) -> Result<Namespace, SdkError> {
        let url = format!("{}/config/namespace", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateNamespace: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "config create_namespace")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode Namespace: {}", e)))
    }

    pub async fn list_items(&self, q: &ListItemsQuery) -> Result<PagedConfigItems, SdkError> {
        let url = format!("{}/config/item?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "config list_items")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedConfigItems: {}", e)))
    }

    pub async fn create_item(&self, req: &CreateConfigItem) -> Result<ConfigItem, SdkError> {
        let url = format!("{}/config/item", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateConfigItem: {}", e)))?;
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
        ensure_success(&resp, "config create_item")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode ConfigItem: {}", e)))
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
        400 | 422 => SdkError::Validation {
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
