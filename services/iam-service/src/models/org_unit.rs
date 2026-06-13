use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct OrgUnit {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub level: i32,
    pub sort_order: i32,
    pub status: OrgStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct UserOrgUnit {
    pub user_id: String,
    pub org_unit_id: String,
    pub is_primary: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum OrgStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "disabled")]
    Disabled,
}

impl fmt::Display for OrgStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OrgStatus::Active => write!(f, "active"),
            OrgStatus::Disabled => write!(f, "disabled"),
        }
    }
}
