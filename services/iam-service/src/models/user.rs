use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};
use std::fmt;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    /// tenant_id 统一有符号 i64（PG 列即 BIGINT；MySQL UNSIGNED 列存正数无损，
    /// 与全仓 u64→i64 清单一致——S5d）。
    pub tenant_id: i64,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub password_hash: String,
    pub salt: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub status: UserStatus,
    pub email_verified: bool,
    pub phone_verified: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub last_login_ip: Option<String>,
    pub password_expired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(transparent)]
pub struct UserStatus(pub String);

impl UserStatus {
    pub fn active() -> Self {
        Self("active".to_string())
    }
    pub fn disabled() -> Self {
        Self("disabled".to_string())
    }
    pub fn locked() -> Self {
        Self("locked".to_string())
    }
    pub fn is_active(&self) -> bool {
        self.0 == "active"
    }
    pub fn is_disabled(&self) -> bool {
        self.0 == "disabled"
    }
    pub fn is_locked(&self) -> bool {
        self.0 == "locked"
    }
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct UserRole {
    pub user_id: String,
    pub role_id: String,
    pub created_at: DateTime<Utc>,
}
