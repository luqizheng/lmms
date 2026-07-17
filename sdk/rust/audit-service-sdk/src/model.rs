use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataStage {
    Hot,
    Warm,
    Cold,
    Expired,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginLog {
    pub id: i64,
    pub username: String,
    pub success: bool,
    pub ip_address: String,
    pub request_id: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub fail_reason: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LoginLogQuery {
    pub username: Option<String>,
    pub success: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl LoginLogQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn username(mut self, v: impl Into<String>) -> Self {
        self.username = Some(v.into());
        self
    }
    pub fn success(mut self, v: bool) -> Self {
        self.success = Some(v);
        self
    }
    pub fn start_time(mut self, v: impl Into<String>) -> Self {
        self.start_time = Some(v.into());
        self
    }
    pub fn end_time(mut self, v: impl Into<String>) -> Self {
        self.end_time = Some(v.into());
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
        if let Some(v) = &self.username {
            out.push_str(&format!("username={}&", url_encode(v)));
        }
        if let Some(v) = &self.success {
            out.push_str(&format!("success={}&", v));
        }
        if let Some(v) = &self.start_time {
            out.push_str(&format!("start_time={}&", url_encode(v)));
        }
        if let Some(v) = &self.end_time {
            out.push_str(&format!("end_time={}&", url_encode(v)));
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
pub struct OperationLog {
    pub id: i64,
    pub user_id: String,
    pub action: String,
    pub resource: String,
    pub request_id: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub after_value: Option<String>,
    #[serde(default)]
    pub before_value: Option<String>,
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OperationLogQuery {
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl OperationLogQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn user_id(mut self, v: impl Into<String>) -> Self {
        self.user_id = Some(v.into());
        self
    }
    pub fn action(mut self, v: impl Into<String>) -> Self {
        self.action = Some(v.into());
        self
    }
    pub fn resource(mut self, v: impl Into<String>) -> Self {
        self.resource = Some(v.into());
        self
    }
    pub fn start_time(mut self, v: impl Into<String>) -> Self {
        self.start_time = Some(v.into());
        self
    }
    pub fn end_time(mut self, v: impl Into<String>) -> Self {
        self.end_time = Some(v.into());
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
        if let Some(v) = &self.user_id {
            out.push_str(&format!("user_id={}&", url_encode(v)));
        }
        if let Some(v) = &self.action {
            out.push_str(&format!("action={}&", url_encode(v)));
        }
        if let Some(v) = &self.resource {
            out.push_str(&format!("resource={}&", url_encode(v)));
        }
        if let Some(v) = &self.start_time {
            out.push_str(&format!("start_time={}&", url_encode(v)));
        }
        if let Some(v) = &self.end_time {
            out.push_str(&format!("end_time={}&", url_encode(v)));
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
pub struct PagedLoginLogs {
    pub items: Vec<LoginLog>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PagedOperationLogs {
    pub items: Vec<OperationLog>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub hot_days: u32,
    pub warm_days: u32,
    pub cold_days: u32,
    pub archive_enabled: bool,
    pub purge_enabled: bool,
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