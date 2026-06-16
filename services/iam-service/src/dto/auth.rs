use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 100))]
    pub username: String,
    #[validate(length(min = 6, max = 128))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RefreshRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 6, max = 128))]
    pub old_password: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ResetPasswordRequest {
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub code: String,
    #[validate(length(min = 8, max = 128))]
    pub new_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SessionInfo {
    pub id: String,
    pub user_id: String,
    pub client_ip: String,
    pub user_agent: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub is_current: bool,
}
