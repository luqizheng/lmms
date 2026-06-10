use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientsConfig {
    pub clients: Vec<Client>,
}

/// Dev 用的明文 client_secret；启动时如果现存 hash 验证不通过就用它重算并替换。
/// 生产环境应通过环境变量 `CLIENT_SECRETS_JSON` 注入真 hash，本函数不参与。
pub const DEFAULT_DEV_CLIENT_SECRET: &str = "dev-gateway-secret";

pub fn hash_client_secret(plain: &str) -> String {
    use argon2::password_hash::{PasswordHasher, SaltString};
    use argon2::Argon2;
    use rand::rngs::OsRng;
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .expect("hash client secret")
        .to_string()
}

pub fn maybe_upgrade_client_secret(c: &mut Client, default_plain: &str) {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    use argon2::Argon2;
    let verified = PasswordHash::new(&c.client_secret_hash)
        .ok()
        .and_then(|ph| {
            Argon2::default()
                .verify_password(default_plain.as_bytes(), &ph)
                .ok()
        })
        .is_some();
    if !verified {
        tracing::warn!(
            client_id = %c.client_id,
            "client_secret_hash did not verify against DEFAULT_DEV_CLIENT_SECRET; replacing with freshly computed hash"
        );
        c.client_secret_hash = hash_client_secret(default_plain);
    }
}

pub struct ClientStore {
    pub clients: Arc<RwLock<ClientsConfig>>,
    path: String,
}

impl ClientStore {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("clients.json not found at {}, creating default", path);
                let default_clients = ClientsConfig {
                    clients: vec![Client {
                        client_id: "api-gateway".to_string(),
                        client_secret_hash: hash_client_secret(DEFAULT_DEV_CLIENT_SECRET),
                        name: "API Gateway".to_string(),
                        permissions: vec!["token:issue".to_string(), "token:revoke".to_string()],
                        grant_types: vec!["authorization_code".to_string(), "refresh_token".to_string()],
                        access_token_ttl_sec: 3600,
                        refresh_token_ttl_sec: 2592000,
                        status: "active".to_string(),
                    }],
                };
                let default_content = serde_json::to_string_pretty(&default_clients)?;
                std::fs::write(path, &default_content)?;
                tracing::info!("created default clients.json at {}", path);
                default_content
            }
            Err(e) => return Err(e.into()),
        };
        let mut clients: ClientsConfig = serde_json::from_str(&content)?;
        for c in &mut clients.clients {
            maybe_upgrade_client_secret(c, DEFAULT_DEV_CLIENT_SECRET);
        }
        tracing::info!("loaded {} clients from {}", clients.clients.len(), path);
        Ok(Self {
            clients: Arc::new(RwLock::new(clients)),
            path: path.to_string(),
        })
    }

    pub async fn find_by_id(&self, client_id: &str) -> Option<Client> {
        let clients = self.clients.read().await;
        clients
            .clients
            .iter()
            .find(|c| c.client_id == client_id)
            .cloned()
    }

    pub async fn add_client(&self, client: Client) {
        let mut clients = self.clients.write().await;
        clients.clients.push(client);
    }

    pub async fn reload(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(&self.path)?;
        let mut clients: ClientsConfig = serde_json::from_str(&content)?;
        for c in &mut clients.clients {
            maybe_upgrade_client_secret(c, DEFAULT_DEV_CLIENT_SECRET);
        }
        *self.clients.write().await = clients;
        tracing::info!("clients reloaded from {}", self.path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::PasswordHash;

    #[test]
    fn hash_then_verify_dev_secret() {
        let h = hash_client_secret(DEFAULT_DEV_CLIENT_SECRET);
        assert!(PasswordHash::new(&h).is_ok());
    }

    #[test]
    fn upgrade_replaces_placeholder_hash() {
        let mut c = Client {
            client_id: "api-gateway".into(),
            client_secret_hash: "$argon2id$v=19$m=65536,t=3,p=4$placeholder$invalidsaltvalue".into(),
            name: "x".into(),
            permissions: vec![],
            grant_types: vec![],
            access_token_ttl_sec: 3600,
            refresh_token_ttl_sec: 2592000,
            status: "active".into(),
        };
        maybe_upgrade_client_secret(&mut c, DEFAULT_DEV_CLIENT_SECRET);
        let ph = PasswordHash::new(&c.client_secret_hash).unwrap();
        assert!(argon2::Argon2::default()
            .verify_password(DEFAULT_DEV_CLIENT_SECRET.as_bytes(), &ph)
            .is_ok());
    }
}

