use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_server_addr")]
    pub server_addr: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_consul_addr")]
    pub consul_addr: String,
    #[serde(default = "default_consul_service_name")]
    pub consul_service_name: String,
}

fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let host = std::env::var("DATABASE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "3306".to_string());
        let user = std::env::var("DATABASE_USER").unwrap_or_else(|_| "root".to_string());
        let password = std::env::var("DATABASE_PASSWORD").unwrap_or_else(|_| "rootpass".to_string());
        let name = std::env::var("DATABASE_NAME").unwrap_or_else(|_| "superpowers".to_string());
        format!("mysql://{user}:{password}@{host}:{port}/{name}")
    })
}

fn default_server_addr() -> String {
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| {
        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
        format!("{host}:{port}")
    })
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

fn default_consul_addr() -> String {
    std::env::var("CONSUL_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8500".to_string())
}

fn default_consul_service_name() -> String {
    std::env::var("CONSUL_SERVICE_NAME").unwrap_or_else(|_| "audit-service".to_string())
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: default_database_url(),
            server_addr: default_server_addr(),
            log_level: default_log_level(),
            consul_addr: default_consul_addr(),
            consul_service_name: default_consul_service_name(),
        }
    }
}
