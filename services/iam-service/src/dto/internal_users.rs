use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct UserListQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_page() -> u32 { 1 }
fn default_size() -> u32 { 100 }

#[derive(Debug, Serialize, ToSchema)]
pub struct UserIdListResponse {
    pub items: Vec<String>,
    pub page: u32,
    pub size: u32,
    pub total: i64,
    pub truncated: bool,
}