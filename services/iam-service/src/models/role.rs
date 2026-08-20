use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    pub data_scope: DataScope,
    pub status: RoleStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum RoleStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "disabled")]
    Disabled,
}

impl std::fmt::Display for RoleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            RoleStatus::Active => write!(f, "active"),
            RoleStatus::Disabled => write!(f, "disabled"),
        }
    }
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum DataScope {
    #[sqlx(rename = "all")]
    All,
    #[sqlx(rename = "org")]
    Org,
    #[sqlx(rename = "dept")]
    Dept,
    #[sqlx(rename = "self")]
    SelfOnly,
}

impl std::fmt::Display for DataScope {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DataScope::All => write!(f, "all"),
            DataScope::Org => write!(f, "org"),
            DataScope::Dept => write!(f, "dept"),
            DataScope::SelfOnly => write!(f, "self"),
        }
    }
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct RolePermission {
    pub role_id: String,
    pub permission_id: String,
    pub created_at: DateTime<Utc>,
}
