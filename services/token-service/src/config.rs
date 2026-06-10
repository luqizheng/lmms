use common::config::{RedisConfig, ServerConfig};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ConsulConfig {
    #[serde(default)]
    pub addr: String,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub service_id_file: String,
}

impl Default for ConsulConfig {
    fn default() -> Self {
        Self {
            addr: "http://localhost:8500".into(),
            service_name: "token-server".into(),
            service_id_file: "/tmp/token-server-service-id".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenServerConfig {
    #[serde(default)]
    pub server: ServerConfig,

    #[serde(default)]
    pub redis: RedisConfig,

    #[serde(default)]
    pub consul: ConsulConfig,

    #[serde(default)]
    pub jwt_private_key_path: String,
    #[serde(default)]
    pub jwt_public_key_path: String,
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub clients_file_path: String,

    #[serde(default)]
    pub default_access_ttl_sec: i64,
    #[serde(default)]
    pub default_refresh_ttl_sec: i64,
    #[serde(default)]
    pub default_resource_ttl_sec: i64,
    #[serde(default)]
    pub user_service_url: String,
}

impl Default for TokenServerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".into(),
                port: 4000,
            },
            redis: RedisConfig::default(),
            consul: ConsulConfig::default(),
            jwt_private_key_path: "keys/private.pem".into(),
            jwt_public_key_path: "keys/public.pem".into(),
            issuer: "superpowers-token-server".into(),
            clients_file_path: "/config/client.json".into(),
            default_access_ttl_sec: 3600,
            default_refresh_ttl_sec: 2592000,
            default_resource_ttl_sec: 300,
            user_service_url: "http://user-service:8080".into(),
        }
    }
}
