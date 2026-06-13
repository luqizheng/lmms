use sqlx::{FromRow, Type};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Menu {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    pub path: String,
    pub component: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub visible: bool,
    pub status: MenuStatus,
    pub permission_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Type, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "varchar")]
pub enum MenuStatus {
    #[sqlx(rename = "enabled")]
    Enabled,
    #[sqlx(rename = "disabled")]
    Disabled,
}

impl fmt::Display for MenuStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MenuStatus::Enabled => write!(f, "enabled"),
            MenuStatus::Disabled => write!(f, "disabled"),
        }
    }
}
