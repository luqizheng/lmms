use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct LoginLog {
    pub id: String,
    pub user_id: Option<String>,
    pub username: String,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub location: Option<String>,
    pub login_type: LoginType,
    pub success: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum LoginType {
    #[sqlx(rename = "password")]
    Password,
    #[sqlx(rename = "sms")]
    Sms,
    #[sqlx(rename = "sso")]
    Sso,
}
