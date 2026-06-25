use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct LoginLog {
    pub id: i64,
    pub user_id: Option<String>,
    pub username: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub ip_address: String,
    pub location: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateLoginLog {
    pub user_id: Option<String>,
    pub username: String,
    pub success: bool,
    pub fail_reason: Option<String>,
    pub ip_address: String,
    pub location: Option<String>,
    pub user_agent: Option<String>,
    pub request_id: String,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct QueryLoginLogs {
    pub username: Option<String>,
    pub success: Option<bool>,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct PagedLoginLogs {
    pub items: Vec<LoginLog>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
