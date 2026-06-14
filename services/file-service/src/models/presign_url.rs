use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PresignUrl {
    pub id: String,
    pub file_id: String,
    pub operation: String, // "get" or "put"
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePresignUrl {
    pub file_id: String,
    pub operation: String,
    pub expires_in_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PresignUrlResponse {
    pub url: String,
    pub expires_at: DateTime<Utc>,
    pub operation: String,
}