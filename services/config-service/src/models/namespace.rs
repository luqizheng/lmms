use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Namespace {
    pub id: String,
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNamespace {
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNamespace {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryNamespaces {
    pub app: Option<String>,
    pub env: Option<String>,
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
pub struct PagedNamespaces {
    pub items: Vec<Namespace>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
