use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClientRow {
    pub client_id: String,
    pub client_secret_hash: String,
    pub name: String,
    pub permissions: serde_json::Value, // JSON 列 -> Value
    pub grant_types: serde_json::Value,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub client_id: String,
    pub client_secret_hash: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub grant_types: Vec<String>,
    pub access_token_ttl_sec: i64,
    pub refresh_token_ttl_sec: i64,
    pub status: String,
}

impl ClientRow {
    pub fn into_dto(self) -> Client {
        Client {
            client_id: self.client_id,
            client_secret_hash: self.client_secret_hash,
            name: self.name,
            permissions: serde_json::from_value(self.permissions).unwrap_or_default(),
            grant_types: serde_json::from_value(self.grant_types).unwrap_or_default(),
            access_token_ttl_sec: self.access_token_ttl_sec,
            refresh_token_ttl_sec: self.refresh_token_ttl_sec,
            status: self.status,
        }
    }
}
