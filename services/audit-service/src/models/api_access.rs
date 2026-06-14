use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiAccessLog {
    pub id: i64,
    pub service_name: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub user_id: Option<String>,
    pub request_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiAccessLog {
    pub service_name: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub duration_ms: i32,
    pub user_id: Option<String>,
    pub request_id: String,
}
