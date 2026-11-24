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
    /// 租户 ID。
    /// 类型为 i64（而非 u64）：sqlx-postgres 无 unsigned 支持，u64 无法 FromRow 解码。
    ///  手工保住 OpenAPI 的 minimum: 0，使 spec 零变更。
    #[schema(minimum = 0)]
    pub tenant_id: i64,
    pub user_id: Option<String>,
    pub is_temporary: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 兼容字段：内部 file-service 相对路径。用户下载授权必须由业务服务创建 presign URL。
    /// 由 handlers 在返回前注入；DB 不存（`#[sqlx(skip)]`）。
    /// 2026-11-07 裁决后网关不再暴露 /v1/file/* 用户路由——该字段仅在
    /// file-service 被业务服务网关化暴露时才有意义；用户下载一律走
    /// presign 签名 URL（见 contracts/http-conventions.md）。
    #[serde(default)]
    #[sqlx(skip)]
    pub url: String,
}

/// 构造带可渲染 URL 的 FileInfo（用于 API 响应）
pub fn with_url(mut info: FileInfo) -> FileInfo {
    info.url = format!("/file/{}", info.id);
    info
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UploadFileMeta {
    pub filename: String,
    pub content_type: Option<String>,
    /// 见 FileInfo::tenant_id 的类型说明。
    #[schema(minimum = 0)]
    pub tenant_id: Option<i64>,
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
