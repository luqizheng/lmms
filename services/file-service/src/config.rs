use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_server_addr")]
    pub server_addr: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
}

fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

fn default_server_addr() -> String {
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8082".to_string())
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

fn default_storage_path() -> String {
    std::env::var("STORAGE_PATH").unwrap_or_else(|_| "./data/files".to_string())
}

fn default_max_file_size() -> u64 {
    100 * 1024 * 1024 // 100MB
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: default_database_url(),
            server_addr: default_server_addr(),
            log_level: default_log_level(),
            storage_path: default_storage_path(),
            max_file_size: default_max_file_size(),
        }
    }
}