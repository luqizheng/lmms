use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_server_addr")]
    pub server_addr: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

fn default_server_addr() -> String {
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8081".to_string())
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: default_database_url(),
            server_addr: default_server_addr(),
            log_level: default_log_level(),
        }
    }
}
