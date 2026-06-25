use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct ConfigItem {
    pub id: String,
    pub namespace_id: String,
    pub key: String,
    pub value: String,
    pub value_type: String,
    pub is_secret: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateConfigItem {
    pub namespace_id: String,
    pub key: String,
    pub value: String,
    #[serde(default = "default_value_type")]
    pub value_type: String,
    #[serde(default = "default_is_secret")]
    pub is_secret: bool,
    pub description: Option<String>,
}

fn default_value_type() -> String {
    "string".to_string()
}

fn default_is_secret() -> bool {
    false
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateConfigItem {
    pub value: Option<String>,
    pub value_type: Option<String>,
    pub is_secret: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct QueryConfigItems {
    pub namespace_id: Option<String>,
    pub key: Option<String>,
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

#[derive(Debug, Serialize, ToSchema)]
pub struct PagedConfigItems {
    pub items: Vec<ConfigItem>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
