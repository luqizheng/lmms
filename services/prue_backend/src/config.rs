use anyhow::{bail, Context};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_addr: String,
    pub log_level: String,
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
    /// DATABASE_URL is required — no sqlite::memory: fallback. Callers
    /// (tests, dev scripts) must set it explicitly.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL required — prue_backend has no fallback")?;
        if database_url.trim().is_empty() {
            bail!("DATABASE_URL is set but empty");
        }
        Ok(Self {
            database_url,
            server_addr: default_server_addr(),
            log_level: default_log_level(),
        })
    }
}
