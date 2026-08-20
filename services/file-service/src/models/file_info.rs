use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct FileInfo {
    pub id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub size: i64,
    pub hash: Option<String>,
    pub storage_path: String,
    pub tenant_id: u64,
    pub user_id: Option<String>,
    pub is_temporary: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 客户端可渲染的下载 URL（相对路径，前端 axios baseURL /api + 此值）。
    /// 由 handlers 在返回前注入；DB 不存（`#[sqlx(skip)]`）。
    #[serde(default)]
    #[sqlx(skip)]
    pub url: String,
}

/// 构造带可渲染 URL 的 FileInfo（用于 API 响应）
pub fn with_url(mut info: FileInfo) -> FileInfo {
    info.url = format!("/v1/file/{}", info.id);
    info
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadFileMeta {
    pub filename: String,
    pub content_type: Option<String>,
    pub tenant_id: Option<u64>,
    pub user_id: Option<String>,
    pub is_temporary: Option<bool>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct QueryFiles {
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub filename: Option<String>,
    #[serde(default = "default_page")]
    pub page: Option<u32>,
    #[serde(default = "default_size")]
    pub size: Option<u32>,
}

fn default_page() -> Option<u32> {
    Some(1)
}

fn default_size() -> Option<u32> {
    Some(20)
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PagedFiles {
    pub items: Vec<FileInfo>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
