use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct LoginLogRow {
    pub id: u64,
    pub user_id: Option<u64>,
    pub username: String,
    pub ip: String,
    pub user_agent: String,
    pub success: i8,
    pub fail_reason: Option<String>,
    pub login_at: String,
}

pub const FAIL_USER_NOT_FOUND: &str = "user_not_found";
pub const FAIL_BAD_PASSWORD: &str = "bad_password";
pub const FAIL_LOCKED: &str = "locked";
