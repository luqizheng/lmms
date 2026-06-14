use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileInfo {
    pub id: String,
    pub filename: String,
    pub content_type: Option<String>,
    pub size: i64,
    pub hash: Option<String>,
    pub storage_path: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub is_temporary: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UploadFileMeta {
    pub filename: String,
    pub content_type: Option<String>,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub is_temporary: Option<bool>,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Serialize)]
pub struct PagedFiles {
    pub items: Vec<FileInfo>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}