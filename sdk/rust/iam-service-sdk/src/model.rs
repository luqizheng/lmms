use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_slug: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub nickname: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserResponse {
    pub id: String,
    pub username: String,
    pub nickname: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub dept_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub nickname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_slug: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateUserRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListUsersQuery {
    pub keyword: Option<String>,
    pub dept_id: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListUsersQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn keyword(mut self, v: impl Into<String>) -> Self {
        self.keyword = Some(v.into());
        self
    }
    pub fn dept_id(mut self, v: impl Into<String>) -> Self {
        self.dept_id = Some(v.into());
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
        if let Some(v) = &self.keyword {
            out.push_str(&format!("keyword={}&", url_encode(v)));
        }
        if let Some(v) = &self.dept_id {
            out.push_str(&format!("dept_id={}&", url_encode(v)));
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

#[derive(Debug, Clone, Deserialize)]
pub struct UserListResponse {
    pub items: Vec<UserResponse>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleResponse {
    pub id: String,
    pub name: String,
    pub code: String,
    pub data_scope: i32,
    pub status: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateRoleRequest {
    pub name: String,
    pub code: String,
    pub data_scope: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct UpdateRoleRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_scope: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct ListRolesQuery {
    pub keyword: Option<String>,
    pub page: Option<u32>,
    pub size: Option<u32>,
}

impl ListRolesQuery {
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

#[derive(Debug, Clone, Deserialize)]
pub struct RoleListResponse {
    pub items: Vec<RoleResponse>,
    pub total: i64,
    pub page: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PermissionResponse {
    pub id: String,
    pub code: String,
    pub name: String,
    pub category: i32,
    pub sort_order: i32,
    pub status: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PermissionListResponse {
    pub items: Vec<PermissionResponse>,
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
