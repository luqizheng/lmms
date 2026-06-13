use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Permission {
    pub id: String,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub category: PermissionCategory,
    pub resource: Option<String>,
    pub action: Option<String>,
    pub sort_order: i32,
    pub status: PermissionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum PermissionCategory {
    #[sqlx(rename = "menu")]
    Menu,
    #[sqlx(rename = "button")]
    Button,
    #[sqlx(rename = "api")]
    Api,
}

impl fmt::Display for PermissionCategory {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PermissionCategory::Menu => write!(f, "menu"),
            PermissionCategory::Button => write!(f, "button"),
            PermissionCategory::Api => write!(f, "api"),
        }
    }
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum PermissionStatus {
    #[sqlx(rename = "enabled")]
    Enabled,
    #[sqlx(rename = "disabled")]
    Disabled,
}

impl fmt::Display for PermissionStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PermissionStatus::Enabled => write!(f, "enabled"),
            PermissionStatus::Disabled => write!(f, "disabled"),
        }
    }
}
