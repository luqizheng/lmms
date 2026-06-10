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
                        client_secret_hash: "$argon2id$v=19$m=65536,t=3,p=4$c29tZS1zb21lLWtleQ$c29tZS1zZWNyZXQtaGFzaA".to_string(),
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
        let clients: ClientsConfig = serde_json::from_str(&content)?;
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
        let clients: ClientsConfig = serde_json::from_str(&content)?;
        *self.clients.write().await = clients;
        tracing::info!("clients reloaded from {}", self.path);
        Ok(())
    }
}
