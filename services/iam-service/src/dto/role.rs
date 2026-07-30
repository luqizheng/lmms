use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 3))]
    pub data_scope: i32,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 3))]
    pub data_scope: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RoleQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RoleUsersQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub size: u32,
}

fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 20 }

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    pub data_scope: String,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleListResponse {
    pub items: Vec<RoleResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignUserRolesRequest {
    pub role_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AssignUserRolesResponse {
    pub user_id: String,
    pub role_ids: Vec<String>,
}
