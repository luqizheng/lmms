use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    CreatePresignUrl, FileInfo, ListFilesQuery, PagedFiles, PresignUrlResponse,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct FileClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
}

impl FileClient {
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

    pub async fn create_presign(
        &self,
        req: &CreatePresignUrl,
    ) -> Result<PresignUrlResponse, SdkError> {
        let url = format!("{}/file/presign", self.base_url);
        let body = serde_json::to_string(req)
            .map_err(|e| SdkError::Network(format!("serialize CreatePresignUrl: {}", e)))?;
        let resp = self.http.post(&url, &body, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "file create_presign")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PresignUrlResponse: {}", e)))
    }

    pub async fn list_files(&self, q: &ListFilesQuery) -> Result<PagedFiles, SdkError> {
        let url = format!("{}/file/list?{}", self.base_url, q.to_query_string());
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "file list_files")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode PagedFiles: {}", e)))
    }

    pub async fn get_file_info(&self, id: &str) -> Result<FileInfo, SdkError> {
        let url = format!("{}/file/{}/info", self.base_url, url_encode_path(id));
        let resp = self.http.get(&url, (self.bearer)().as_deref()).await?;
        ensure_success(&resp, "file get_file_info")?;
        serde_json::from_str(&resp.body)
            .map_err(|e| SdkError::Network(format!("decode FileInfo: {}", e)))
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

fn url_encode_path(s: &str) -> String {
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