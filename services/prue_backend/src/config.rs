#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub log_level: String,
}

fn default_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string())
}

fn default_server_addr() -> String {
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8080".to_string());
    std::env::var("SERVER_ADDR").unwrap_or_else(|_| format!("{host}:{port}"))
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
