use chrono::DateTime;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 50))]
    pub code: String,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateOrgUnitRequest {
    pub parent_id: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(min = 1, max = 50))]
    pub code: Option<String>,
    pub sort_order: Option<i32>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Clone, ToSchema)]
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
    // Self-referential schema; without `no_recursion` utoipa inlines the
    // subtree indefinitely and overflows the stack while building `openapi()`.
    // The runtime JSON payload is unaffected — this only shapes the OpenAPI
    // schema output so it emits a `$ref` back to `OrgUnitResponse` instead of
    // expanding forever.
    #[schema(no_recursion)]
    pub children: Vec<OrgUnitResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignUsersRequest {
    pub user_ids: Vec<String>,
}
