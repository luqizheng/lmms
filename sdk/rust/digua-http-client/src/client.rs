use crate::cache::ServiceTokenCache;
use crate::error::KudiHttpError;
use crate::fetcher::ClientCredentialsFetcher;
use crate::options::KudiHttpOptions;
use crate::transport::{HttpResponse, HttpTransport, ReqwestTransport};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct KudiWebClient {
    transport: Arc<dyn HttpTransport>,
    options: KudiHttpOptions,
    cache: ServiceTokenCache,
    fetcher: Arc<ClientCredentialsFetcher>,
    credential: Option<(String, String)>,
    base_url: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Builder {
    pub options: KudiHttpOptions,
    pub client_id: Option<String>,
    pub secret_key: Option<String>,
    pub base_url: Option<String>,
}

impl Builder {
    pub fn credential(
        mut self,
        client_id: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.client_id = Some(client_id.into());
        self.secret_key = Some(secret_key.into());
        self
    }
    pub fn token_endpoint(mut self, url: impl Into<String>) -> Self {
        self.options.token_endpoint = url.into();
        self
    }
    pub fn read_timeout_ms(mut self, ms: u64) -> Self {
        self.options.read_timeout = Duration::from_millis(ms);
        self
    }
    pub fn connect_timeout_ms(mut self, ms: u64) -> Self {
        self.options.connect_timeout = Duration::from_millis(ms);
        self
    }
    pub fn retry_count(mut self, n: u32) -> Self {
        self.options.retry_count = n;
        self
    }
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }
    pub fn build(self) -> Result<KudiWebClient, KudiHttpError> {
        let transport = ReqwestTransport::new(self.options.clone())?;
        let cache = ServiceTokenCache::new(self.options.refresh_skew);
        let fetcher = Arc::new(ClientCredentialsFetcher::new(
            transport.clone(),
            self.options.token_endpoint.clone(),
        ));
        Ok(KudiWebClient {
            transport,
            options: self.options,
            cache,
            fetcher,
            credential: self.client_id.zip(self.secret_key),
            base_url: self.base_url,
        })
    }
}

impl KudiWebClient {
    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn with_transport(transport: Arc<dyn HttpTransport>, options: KudiHttpOptions) -> Self {
        let cache = ServiceTokenCache::new(options.refresh_skew);
        let fetcher = Arc::new(ClientCredentialsFetcher::new(
            transport.clone(),
            options.token_endpoint.clone(),
        ));
        Self {
            transport,
            options,
            cache,
            fetcher,
            credential: None,
            base_url: None,
        }
    }

    pub fn with_credential(
        mut self,
        client_id: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.credential = Some((client_id.into(), secret_key.into()));
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Returns the service base URL previously set on the builder, or `None`
    /// if the caller did not provide one. Service SDKs (iam/token/audit/...)
    /// use this to compose full request paths such as
    /// `format!("{}/v1/iam/auth/login", self.base_url().unwrap())`.
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    pub fn options(&self) -> &KudiHttpOptions {
        &self.options
    }

    pub async fn cache_snapshot(&self) -> crate::cache::TokenCacheSnapshot {
        self.cache.snapshot().await
    }

    /// Invalidate the cached client_credentials access token so the next call
    /// refetches it from the token endpoint. Service SDKs invoke this after
    /// observing a 401 to recover from a rotated/expired token.
    ///
    /// No-op when the client was constructed without `credential(...)` or
    /// `with_credential(...)` (Bearer passthrough has no cache entry).
    pub async fn force_refresh(&self) -> Result<(), KudiHttpError> {
        if let Some((cid, sk)) = &self.credential {
            self.cache.invalidate(cid, sk).await;
        }
        Ok(())
    }

    async fn resolve_bearer(
        &self,
        explicit: Option<&str>,
    ) -> Result<Option<String>, KudiHttpError> {
        if let Some(b) = explicit {
            return Ok(Some(b.to_string()));
        }
        if let Some((cid, sk)) = &self.credential {
            let cid = cid.clone();
            let sk = sk.clone();
            let f = self.fetcher.clone();
            let token = self
                .cache
                .get_or_fetch(&cid, &sk.clone(), || {
                    let cid2 = cid.clone();
                    let sk2 = sk.clone();
                    async move { f.fetch(&cid2, &sk2).await }
                })
                .await?;
            return Ok(Some(token));
        }
        Ok(None)
    }

    pub async fn post(
        &self,
        url: &str,
        json_body: &str,
        bearer: Option<&str>,
    ) -> Result<HttpResponse, KudiHttpError> {
        let bearer = self.resolve_bearer(bearer).await?;
        self.transport
            .post_json(url, bearer.as_deref(), json_body)
            .await
    }

    pub async fn get(
        &self,
        url: &str,
        bearer: Option<&str>,
    ) -> Result<HttpResponse, KudiHttpError> {
        let bearer = self.resolve_bearer(bearer).await?;
        self.transport.get(url, bearer.as_deref()).await
    }

    pub async fn get_with_bearer(
        &self,
        url: &str,
        bearer: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.transport.get(url, Some(bearer)).await
    }
}
