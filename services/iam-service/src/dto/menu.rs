use chrono::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 100))]
    pub code: String,
    #[validate(length(min = 1, max = 255))]
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateMenuRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub code: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub path: Option<String>,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
    pub visible: Option<bool>,
    pub permission_id: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
pub struct MenuResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub visible: bool,
    pub status: String,
    pub permission_id: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
    pub children: Vec<MenuResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MenuListResponse {
    pub items: Vec<MenuResponse>,
    pub total: u64,
}
