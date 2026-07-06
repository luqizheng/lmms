use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
pub struct Namespace {
    pub id: String,
    pub tenant_id: u64,
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateNamespace {
    pub app: String,
    pub env: String,
    pub profile: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNamespace {
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct PagedNamespaces {
    pub items: Vec<Namespace>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
