use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConfigVersion {
    pub id: String,
    pub namespace_id: String,
    pub item_id: String,
    pub key: String,
    pub old_value: String,
    pub new_value: String,
    pub operator: Option<String>,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct QueryVersions {
    pub namespace_id: Option<String>,
    pub item_id: Option<String>,
    #[serde(default = "default_page")]
    pub page: Option<u32>,
    #[serde(default = "default_size")]
    pub size: Option<u32>,
}

fn default_page() -> Option<u32> {
    Some(1)
}

fn default_size() -> Option<u32> {
    Some(20)
}

#[derive(Debug, Serialize)]
pub struct PagedVersions {
    pub items: Vec<ConfigVersion>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
