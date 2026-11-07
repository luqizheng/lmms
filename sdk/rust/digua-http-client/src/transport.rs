use crate::error::KudiHttpError;
use crate::options::KudiHttpOptions;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

#[async_trait]
pub trait HttpTransport: Send + Sync {
    async fn post_form(
        &self,
        url: &str,
        form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError>;
    async fn post_json(
        &self,
        url: &str,
        bearer: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError>;
    async fn get(&self, url: &str, bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError>;
}

pub struct ReqwestTransport {
    client: Client,
    options: KudiHttpOptions,
}

impl ReqwestTransport {
    pub fn new(options: KudiHttpOptions) -> Result<Arc<Self>, KudiHttpError> {
        crate::ensure_crypto_provider();
        // Mesh-internal transport: never route through a system proxy — a
        // leaked http_proxy env on dev machines would hijack loopback/intranet
        // calls (same recipe as the JWKS clients in iam/api-gateway).
        let client = Client::builder()
            .no_proxy()
            .connect_timeout(options.connect_timeout)
            .timeout(options.read_timeout)
            .user_agent(options.user_agent.clone())
            .build()
            .map_err(|e| KudiHttpError::Network(format!("build reqwest client: {}", e)))?;
        Ok(Arc::new(Self { client, options }))
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn post_form(
        &self,
        url: &str,
        form: &HashMap<String, String>,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.send_with_retry(
            "POST",
            url,
            None,
            Some(("application/x-www-form-urlencoded", encode_form(form))),
        )
        .await
    }

    async fn post_json(
        &self,
        url: &str,
        bearer: Option<&str>,
        json_body: &str,
    ) -> Result<HttpResponse, KudiHttpError> {
        self.send_with_retry(
            "POST",
            url,
            bearer,
            Some(("application/json", json_body.to_string())),
        )
        .await
    }

    async fn get(&self, url: &str, bearer: Option<&str>) -> Result<HttpResponse, KudiHttpError> {
        self.send_with_retry("GET", url, bearer, None).await
    }
}

impl ReqwestTransport {
    async fn send_with_retry(
        &self,
        method: &str,
        url: &str,
        bearer: Option<&str>,
        body: Option<(&'static str, String)>,
    ) -> Result<HttpResponse, KudiHttpError> {
        let mut attempt = 0u32;
        loop {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if let Some(b) = bearer {
                if let Ok(v) = HeaderValue::from_str(&format!("Bearer {}", b)) {
                    headers.insert(HeaderName::from_static("authorization"), v);
                }
            }

            let m = reqwest::Method::from_bytes(method.as_bytes())
                .map_err(|e| KudiHttpError::Network(format!("bad method: {}", e)))?;
            let mut req = self.client.request(m, url).headers(headers);
            if let Some((ct, body_str)) = body.clone() {
                req = req
                    .header(CONTENT_TYPE, HeaderValue::from_static(ct))
                    .body(body_str);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    return Ok(HttpResponse {
                        status: status.as_u16(),
                        body: text,
                    });
                }
                Err(e) if attempt < self.options.retry_count => {
                    attempt += 1;
                    tracing::warn!(
                        "request retry {}/{}: {}",
                        attempt,
                        self.options.retry_count,
                        e
                    );
                    tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
                    continue;
                }
                Err(e) => return Err(KudiHttpError::Network(e.to_string())),
            }
        }
    }
}

fn encode_form(form: &HashMap<String, String>) -> String {
    form.iter()
        .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

impl HttpResponse {
    #[allow(dead_code)]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    #[allow(dead_code)]
    pub fn status_kind(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
