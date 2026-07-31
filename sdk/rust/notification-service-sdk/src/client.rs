use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    Announcement, CreateAnnouncement, ListAnnouncementsQuery, PagedAnnouncements,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct NotificationClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl NotificationClient {
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

    pub async fn list_announcements(
        &self,
        q: &ListAnnouncementsQuery,
    ) -> Result<PagedAnnouncements, SdkError> {
        let url = format!(
            "{}/notification/announcements?{}",
            self.base_url,
            q.to_query_string()
        );
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification list_announcements")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedAnnouncements: {}", e)))
    }

    pub async fn get_announcement(&self, id: i64) -> Result<Announcement, SdkError> {
        let url = format!("{}/notification/announcements/{}", self.base_url, id);
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification get_announcement")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode Announcement: {}", e)))
    }

    pub async fn create_announcement(
        &self,
        req: &CreateAnnouncement,
    ) -> Result<Announcement, SdkError> {
        let url = format!("{}/notification/announcements", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreateAnnouncement: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification create_announcement")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode Announcement: {}", e)))
    }

    pub async fn publish(&self, id: i64) -> Result<(), SdkError> {
        let url = format!("{}/notification/announcements/{}/publish", self.base_url, id);
        let resp = self.http.post(&url, "{}", (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification publish")?;
        Ok(())
    }

    pub async fn revoke(&self, id: i64) -> Result<(), SdkError> {
        let url = format!("{}/notification/announcements/{}/revoke", self.base_url, id);
        let resp = self.http.post(&url, "{}", (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification revoke")?;
        Ok(())
    }

    pub async fn unread_count(&self) -> Result<serde_json::Value, SdkError> {
        let url = format!("{}/notification/unread-count", self.base_url);
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "notification unread_count")?;
        if resp.body.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode unread_count: {}", e)))
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