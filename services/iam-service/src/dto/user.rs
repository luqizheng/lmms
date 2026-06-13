use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::DateTime;

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 1, max = 50))]
    pub username: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
    #[validate(email)]
    pub email: Option<String>,
    #[validate(length(min = 11, max = 11))]
    pub phone: Option<String>,
    #[validate(length(min = 1, max = 100))]
    pub nickname: String,
    pub avatar: Option<String>,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Deserialize, Validate)]
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

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub keyword: Option<String>,
    pub status: Option<String>,
    pub org_unit_id: Option<String>,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub items: Vec<UserResponse>,
    pub total: u64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Deserialize)]
pub struct BatchImportUsersRequest {
    pub users: Vec<CreateUserRequest>,
}
