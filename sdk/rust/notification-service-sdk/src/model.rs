use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct CreateAnnouncement {
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub pinned: bool,
    pub popover: bool,
    pub recipients: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Announcement {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub publish_at: DateTime<Utc>,
    pub pinned: bool,
    pub popover: bool,
    pub status: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ListAnnouncementsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
}

impl ListAnnouncementsQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn active(mut self, v: bool) -> Self {
        self.active = Some(v);
        self
    }
    pub fn pinned(mut self, v: bool) -> Self {
        self.pinned = Some(v);
        self
    }
    pub fn page(mut self, v: u32) -> Self {
        self.page = Some(v);
        self
    }
    pub fn size(mut self, v: u32) -> Self {
        self.size = Some(v);
        self
    }
    pub(crate) fn to_query_string(&self) -> String {
        let mut out = String::new();
        if let Some(v) = self.active {
            out.push_str(&format!("active={}&", v));
        }
        if let Some(v) = self.pinned {
            out.push_str(&format!("pinned={}&", v));
        }
        if let Some(v) = self.page {
            out.push_str(&format!("page={}&", v));
        }
        if let Some(v) = self.size {
            out.push_str(&format!("size={}&", v));
        }
        out
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PagedAnnouncements {
    pub items: Vec<Announcement>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}
