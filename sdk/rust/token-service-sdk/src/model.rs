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

// ── Service JWT audience grants（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）──

/// 一条 owner → audience 签发授权关系（无任何凭据材料）。
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceJwtGrant {
    pub owner: String,
    pub audience: String,
    /// `active` | `disabled`。
    pub status: String,
    /// true = 内置系统授权（服务端保护：管理 API 禁改禁删）。
    #[serde(default)]
    pub system_managed: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListServiceJwtGrantsResponse {
    #[serde(default)]
    pub items: Vec<ServiceJwtGrant>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub size: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateServiceJwtGrantRequest {
    pub owner: String,
    pub audience: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SetServiceJwtGrantStatusRequest {
    /// `active` | `disabled`。
    pub status: String,
}

/// grant 列表查询参数（owner / audience 精确过滤 + status + 分页）。
#[derive(Debug, Clone, Default)]
pub struct ListServiceJwtGrantsQuery {
    pub owner: Option<String>,
    pub audience: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListServiceJwtGrantsQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn owner(mut self, v: impl Into<String>) -> Self {
        self.owner = Some(v.into());
        self
    }
    pub fn audience(mut self, v: impl Into<String>) -> Self {
        self.audience = Some(v.into());
        self
    }
    pub fn status(mut self, v: impl Into<String>) -> Self {
        self.status = Some(v.into());
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
        if let Some(v) = &self.owner {
            out.push_str(&format!("owner={}&", url_encode(v)));
        }
        if let Some(v) = &self.audience {
            out.push_str(&format!("audience={}&", url_encode(v)));
        }
        if let Some(v) = &self.status {
            out.push_str(&format!("status={}&", url_encode(v)));
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
