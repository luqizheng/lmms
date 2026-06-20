use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OperationLog {
    pub id: i64,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub request_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOperationLog {
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub resource_id: Option<String>,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub request_id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryOperationLogs {
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
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
pub struct PagedOperationLogs {
    pub items: Vec<OperationLog>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
