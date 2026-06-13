use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub jwt: JwtConfig,
    pub consul: ConsulConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
    pub ttl_sec: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtConfig {
    pub private_key_path: PathBuf,
    pub public_key_path: PathBuf,
    pub access_token_ttl_minutes: u64,
    pub refresh_token_ttl_days: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConsulConfig {
    pub host: String,
    pub port: u16,
    pub service_name: String,
}

pub fn load_config() -> anyhow::Result<Config> {
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config/config.yaml".to_string());
    
    let content = std::fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&content)?;
    
    Ok(config)
}
