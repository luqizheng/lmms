use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseCfg {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthCfg {
    pub service_token_issuer: String,
    pub service_token_audience: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerCfg,
    pub database: DatabaseCfg,
    pub redis: RedisCfg,
    pub auth: AuthCfg,
}

pub fn load_config(path: &str) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read config {path}"))?;
    let cfg: Config = serde_yaml::from_str(&raw)
        .with_context(|| format!("parse config {path}"))?;
    Ok(cfg)
}
