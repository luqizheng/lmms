use common_error::{AppError, AppResult};
use reqwest::Client;
use std::time::Duration;

pub struct HttpClient {
    inner: Client,
    request_id: Option<String>,
}

pub struct HttpClientBuilder {
    timeout: Duration,
    request_id: Option<String>,
}

impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder {
            timeout: Duration::from_secs(10),
            request_id: None,
        }
    }
    pub fn inner(&self) -> &Client {
        &self.inner
    }
    pub fn default_request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

impl HttpClientBuilder {
    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }
    pub fn request_id(mut self, rid: impl Into<String>) -> Self {
        self.request_id = Some(rid.into());
        self
    }
    pub fn build(self) -> AppResult<HttpClient> {
        let inner = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| AppError::internal("HTTP_BUILD", e.to_string()))?;
        Ok(HttpClient {
            inner,
            request_id: self.request_id,
        })
    }
}
