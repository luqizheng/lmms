use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

/// Only supported presign operations. "put" is intended for an existing
/// file object key; "get" grants read access to the stored object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PresignOperation {
    Get,
    Put,
}

impl PresignOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Put => "put",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct PresignUrl {
    pub id: String,
    pub file_id: String,
    pub operation: String,
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePresignUrl {
    pub file_id: String,
    pub operation: PresignOperation,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PresignUrlResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub operation: PresignOperation,
}
