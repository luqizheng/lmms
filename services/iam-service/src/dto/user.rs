use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 50))]
    pub username: String,
    /// Optional: when omitted the account is created without a usable password
    /// and an invitation mail is sent (unless `send_invitation` is false).
    #[validate(length(min = 8, max = 128))]
    pub password: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub nickname: String,
    pub avatar: Option<String>,
    pub role_ids: Option<Vec<String>>,
    /// Defaults to true when `password` is omitted.
    pub send_invitation: Option<bool>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, max = 100))]
    pub nickname: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateSelfRequest {
    #[validate(length(min = 1, max = 100))]
    pub nickname: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UserQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub org_unit_id: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: String,
    pub avatar: Option<String>,
    pub status: String,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub last_login_at: Option<DateTime<chrono::Utc>>,
    pub last_login_ip: Option<String>,
    pub created_at: DateTime<chrono::Utc>,
    pub updated_at: DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserListResponse {
    pub items: Vec<UserResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchImportUsersRequest {
    pub users: Vec<CreateUserRequest>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct UserSimpleQuery {
    pub keyword: Option<String>,
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_size")]
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct UserSimpleResponse {
    pub id: String,
    pub nickname: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserSimpleListResponse {
    pub items: Vec<UserSimpleResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserClaimsResponse {
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(rename = "tid")]
    pub tenant_id: i64,
    #[serde(rename = "ct")]
    pub cross_tenant: bool,
}

fn default_page() -> u32 {
    1
}
fn default_size() -> u32 {
    20
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BatchGetUsersRequest {
    pub user_ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BatchGetUsersResponse {
    pub users: Vec<UserResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryUsersByOrgAndRoleRequest {
    pub org_path: String,
    pub role_name: String,
}
