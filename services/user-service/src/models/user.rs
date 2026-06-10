use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: u64,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    pub status: i8,
    pub user_code: String,
    pub profile_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UserProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl UserProfile {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        if s.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(s)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserDTO {
    pub id: u64,
    pub username: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub nickname: Option<String>,
    pub status: i8,
    pub user_code: String,
    pub roles: Vec<String>,
    pub profile: UserProfile,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extends: BTreeMap<String, serde_json::Value>,
}

impl UserRow {
    pub fn into_dto(self, roles: Vec<String>) -> UserDTO {
        let profile = UserProfile::from_json(&self.profile_json).unwrap_or_default();
        UserDTO {
            id: self.id,
            username: self.username,
            email: self.email,
            phone: self.phone,
            nickname: self.nickname,
            status: self.status,
            user_code: self.user_code,
            roles,
            profile,
            created_at: self.created_at,
            updated_at: self.updated_at,
            extends: BTreeMap::new(),
        }
    }
}
