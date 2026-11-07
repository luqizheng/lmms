use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Both derives: downstream REST proxies re-parse the request body before
/// forwarding (e.g. pm-api's /v1/file/presign handler).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePresignUrl {
    pub file_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresignUrlResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub operation: String,
}

/// Multipart upload metadata for `POST /file/upload`. `tenant_id` is
/// deliberately absent: since the 2026-11-07 mesh-only ruling the file's
/// tenant comes from the service-jwt `tid` claim (server-derived), never
/// from client-sent metadata.
#[derive(Debug, Clone, Default, Serialize)]
pub struct UploadFileMeta {
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Real end-user attribution, declared by the trusted caller
    /// (see contracts/http-conventions.md).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_temporary: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub id: String,
    pub filename: String,
    pub size: i64,
    pub storage_path: String,
    pub tenant_id: String,
    pub is_temporary: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedFiles {
    pub items: Vec<FileInfo>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ListFilesQuery {
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub filename: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListFilesQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn tenant_id(mut self, v: impl Into<String>) -> Self {
        self.tenant_id = Some(v.into());
        self
    }
    pub fn user_id(mut self, v: impl Into<String>) -> Self {
        self.user_id = Some(v.into());
        self
    }
    pub fn filename(mut self, v: impl Into<String>) -> Self {
        self.filename = Some(v.into());
        self
    }
    pub fn page(mut self, v: u32) -> Self {
        self.page = Some(v);
        self
    }
    pub fn size(mut self, v: u32) -> Self {
        self.size = Some(v);
        self
    }
    pub(crate) fn to_query_string(&self) -> String {
        let mut out = String::new();
        if let Some(v) = &self.tenant_id {
            out.push_str(&format!("tenant_id={}&", url_encode(v)));
        }
        if let Some(v) = &self.user_id {
            out.push_str(&format!("user_id={}&", url_encode(v)));
        }
        if let Some(v) = &self.filename {
            out.push_str(&format!("filename={}&", url_encode(v)));
        }
        if let Some(v) = self.page {
            out.push_str(&format!("page={}&", v));
        }
        if let Some(v) = self.size {
            out.push_str(&format!("size={}&", v));
        }
        out
    }
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

/// Binary download result for `GET /file/{id}`.
#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    /// Filename parsed from `Content-Disposition` when present.
    pub filename: Option<String>,
}
