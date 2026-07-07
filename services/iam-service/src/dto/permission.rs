use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreatePermissionRequest {
    #[validate(length(min = 1, max = 100))]
    pub code: String,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 2))]
    pub category: i32,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdatePermissionRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct PermissionQuery {
    pub keyword: Option<String>,
    pub category: Option<i32>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionResponse {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: i32,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PermissionListResponse {
    pub items: Vec<PermissionResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}
