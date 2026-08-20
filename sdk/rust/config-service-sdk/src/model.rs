use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct CreateNamespace {
    pub app: String,
    pub env: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Namespace {
    pub id: String,
    pub tenant_id: String,
    pub app: String,
    pub env: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListNamespacesQuery {
    pub app: Option<String>,
    pub env: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListNamespacesQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn app(mut self, v: impl Into<String>) -> Self {
        self.app = Some(v.into());
        self
    }
    pub fn env(mut self, v: impl Into<String>) -> Self {
        self.env = Some(v.into());
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
        if let Some(v) = &self.app {
            out.push_str(&format!("app={}&", url_encode(v)));
        }
        if let Some(v) = &self.env {
            out.push_str(&format!("env={}&", url_encode(v)));
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
pub struct PagedNamespaces {
    pub items: Vec<Namespace>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateConfigItem {
    pub namespace_id: String,
    pub key: String,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_secret: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfigItem {
    pub id: String,
    pub namespace_id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub value_type: String,
    pub is_secret: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListItemsQuery {
    pub namespace_id: Option<String>,
    pub key: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListItemsQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn namespace_id(mut self, v: impl Into<String>) -> Self {
        self.namespace_id = Some(v.into());
        self
    }
    pub fn key(mut self, v: impl Into<String>) -> Self {
        self.key = Some(v.into());
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
        if let Some(v) = &self.namespace_id {
            out.push_str(&format!("namespace_id={}&", url_encode(v)));
        }
        if let Some(v) = &self.key {
            out.push_str(&format!("key={}&", url_encode(v)));
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
pub struct PagedConfigItems {
    pub items: Vec<ConfigItem>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
