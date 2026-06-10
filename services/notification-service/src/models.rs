use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Scheduled,
    Published,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR")]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ScopeType {
    User,
    Role,
    Dept,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Announcement {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: i8,
    pub popover: i8,
    pub status: Status,
    pub created_by: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScopeInput {
    pub scope_type: ScopeType,
    pub scope_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAnnouncement {
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub expire_at: Option<DateTime<Utc>>,
    pub pinned: bool,
    pub popover: bool,
    pub recipients: Vec<ScopeInput>,
}
