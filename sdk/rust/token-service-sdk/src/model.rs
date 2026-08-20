use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct AccessRequest {
    pub grant_type: String,
    pub code: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccessResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RevokeRequest {
    pub token_jti: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevokeResponse {
    pub success: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeySummary {
    #[serde(default)]
    pub api_key_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub grant_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListApiKeysResponse {
    #[serde(default)]
    pub items: Vec<ApiKeySummary>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grant_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeyCreated {
    pub api_key_id: String,
    pub api_key_secret: String,
}

#[derive(Debug, Clone, Default)]
pub struct ListApiKeysQuery {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListApiKeysQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn keyword(mut self, v: impl Into<String>) -> Self {
        self.keyword = Some(v.into());
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
        if let Some(v) = &self.keyword {
            out.push_str(&format!("keyword={}&", url_encode(v)));
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
