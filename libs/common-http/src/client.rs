use digua_common_error::{AppError, AppResult};
use reqwest::Client;
use std::time::Duration;

pub struct HttpClient {
    inner: Client,
    request_id_header: String,
}

#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub pool_max_idle: usize,
    pub request_id_header: String,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
            pool_max_idle: 10,
            request_id_header: "X-Request-Id".to_string(),
        }
    }
}

impl HttpClient {
    pub fn builder() -> HttpClientBuilder {
        HttpClientBuilder {
            config: HttpClientConfig::default(),
            default_request_id: None,
        }
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }

    pub fn request_id_header(&self) -> &str {
        &self.request_id_header
    }
}

pub struct HttpClientBuilder {
    config: HttpClientConfig,
    default_request_id: Option<String>,
}

impl HttpClientBuilder {
    pub fn timeout(mut self, t: Duration) -> Self {
        self.config.timeout = t;
        self
    }

    pub fn connect_timeout(mut self, t: Duration) -> Self {
        self.config.connect_timeout = t;
        self
    }

    pub fn pool_max_idle(mut self, n: usize) -> Self {
        self.config.pool_max_idle = n;
        self
    }

    pub fn request_id_header(mut self, header: impl Into<String>) -> Self {
        self.config.request_id_header = header.into();
        self
    }

    pub fn default_request_id(mut self, rid: impl Into<String>) -> Self {
        self.default_request_id = Some(rid.into());
        self
    }

    pub fn build(self) -> AppResult<HttpClient> {
        let inner = Client::builder()
            .timeout(self.config.timeout)
            .connect_timeout(self.config.connect_timeout)
            .pool_max_idle_per_host(self.config.pool_max_idle)
            .build()
            .map_err(|e| AppError::internal("HTTP_BUILD", e.to_string()))?;

        Ok(HttpClient {
            inner,
            request_id_header: self.config.request_id_header,
        })
    }
}
