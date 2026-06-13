use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub code: Option<String>,
    pub sort_order: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrgUnitResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: i32,
    pub sort_order: i32,
    pub status: String,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
    pub children: Vec<OrgUnitResponse>,
}

#[derive(Debug, Deserialize)]
pub struct AssignUsersRequest {
    pub user_ids: Vec<String>,
}
