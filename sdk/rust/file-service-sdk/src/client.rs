use std::sync::Arc;

use digua_http_client::client::KudiWebClient;
use digua_http_client::{transport::HttpResponse, SdkError};

use crate::model::{
    CreatePresignUrl, DownloadedFile, FileInfo, ListFilesQuery, PagedFiles, PresignUrlResponse,
    UploadFileMeta,
};

type BearerSupplier = Arc<dyn Fn() -> Option<String> + Send + Sync>;

pub struct FileClient {
    base_url: String,
    bearer: BearerSupplier,
    http: KudiWebClient,
    /// Dedicated reqwest client for multipart/binary paths — the shared
    /// KudiWebClient transport is String-body only.
    bytes_http: reqwest::Client,
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
        // Mesh-internal client: never route through a system proxy (a
        // leaked http_proxy env on dev machines would hijack loopback/intranet
        // calls — same rationale as the JWKS clients in iam/gateway).
        let bytes_http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .expect("file-sdk: reqwest build");
        Self {
            base_url,
            bearer: Arc::new(bearer_supplier),
            http,
            bytes_http,
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
        let resp = self
            .http
            .post(&url, &body, (self.bearer)().as_deref())
            .await?;
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

    /// Multipart upload: `POST /file/upload` with fields `metadata` (JSON)
    /// and `file` (raw bytes). The file's tenant comes from the bearer
    /// service-jwt's `tid` claim — never from client-sent metadata.
    pub async fn upload_file(
        &self,
        meta: &UploadFileMeta,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<FileInfo, SdkError> {
        let url = format!("{}/file/upload", self.base_url);
        let meta_json = serde_json::to_string(meta)
            .map_err(|e| SdkError::Network(format!("serialize UploadFileMeta: {}", e)))?;
        let part = reqwest::multipart::Part::bytes(data).file_name(filename.to_string());
        let form = reqwest::multipart::Form::new()
            .text("metadata", meta_json)
            .part("file", part);
        let resp = self
            .bytes_http
            .post(&url)
            .bearer_auth(self.require_bearer()?)
            .multipart(form)
            .send()
            .await
            .map_err(|e| SdkError::Network(format!("file upload: {}", e)))?;
        let (status, body) = split_text_response(resp, "file upload").await?;
        ensure_status(status, &body, "file upload")?;
        serde_json::from_str(&body)
            .map_err(|e| SdkError::Network(format!("decode FileInfo: {}", e)))
    }

    /// Binary download: `GET /file/{id}` (raw bytes, Content-Disposition
    /// filename parsed when present).
    pub async fn download_file(&self, id: &str) -> Result<DownloadedFile, SdkError> {
        let url = format!("{}/file/{}", self.base_url, url_encode_path(id));
        let resp = self
            .bytes_http
            .get(&url)
            .bearer_auth(self.require_bearer()?)
            .send()
            .await
            .map_err(|e| SdkError::Network(format!("file download: {}", e)))?;
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let body = resp
            .bytes()
            .await
            .map_err(|e| SdkError::Network(format!("file download: read body: {}", e)))?
            .to_vec();
        let body_text = std::str::from_utf8(&body).unwrap_or("");
        ensure_status(status, body_text, "file download")?;
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let filename = headers
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_disposition_filename);
        Ok(DownloadedFile {
            bytes: body,
            content_type,
            filename,
        })
    }

    fn require_bearer(&self) -> Result<String, SdkError> {
        (self.bearer)().ok_or_else(|| SdkError::Auth {
            status: 401,
            message: "no bearer token supplied".into(),
            details: None,
        })
    }
}

async fn split_text_response(
    resp: reqwest::Response,
    op: &str,
) -> Result<(u16, String), SdkError> {
    let status = resp.status().as_u16();
    let body = resp
        .text()
        .await
        .map_err(|e| SdkError::Network(format!("{}: read body: {}", op, e)))?;
    Ok((status, body))
}

fn ensure_status(status: u16, body: &str, op: &str) -> Result<(), SdkError> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let msg = format!("{}: HTTP {}", op, status);
    Err(match status {
        401 | 403 => SdkError::Auth {
            status,
            message: msg,
            details: Some(body.to_string()),
        },
        429 => SdkError::RateLimit {
            message: msg,
            details: Some(body.to_string()),
        },
        400 | 422 => SdkError::Validation {
            status,
            message: msg,
            details: Some(body.to_string()),
        },
        500..=599 => SdkError::Server {
            status,
            message: msg,
            details: Some(body.to_string()),
        },
        _ => SdkError::Network(format!("{}: unexpected status {}", op, status)),
    })
}

/// Parse `filename="..."` / `filename*=UTF-8''...` from a Content-Disposition
/// header value (minimal subset, same semantics as the TS SDK helper).
fn parse_disposition_filename(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            // RFC 5987: charset'lang'percent-encoded
            let mut it = rest.splitn(3, '\'');
            let _charset = it.next()?;
            let _lang = it.next()?;
            return percent_decode(it.next()?);
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            return percent_decode(rest.trim_matches('"'));
        }
    }
    None
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
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
