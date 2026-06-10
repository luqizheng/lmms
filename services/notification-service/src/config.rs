use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerCfg {
    pub http_addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MysqlCfg {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerCfg {
    pub tick_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogCfg {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerCfg,
    pub mysql: MysqlCfg,
    pub redis: RedisCfg,
    pub scheduler: SchedulerCfg,
    pub log: LogCfg,
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Ok(config::Config::builder()
            .add_source(config::File::from_str(&raw, config::FileFormat::Yaml))
            .build()?
            .try_deserialize()?)
    }
}
