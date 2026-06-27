use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerCfg {
    pub http_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MysqlCfg {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RedisCfg {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SchedulerCfg {
    pub tick_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogCfg {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ConsulCfg {
    pub addr: String,
    pub service_name: String,
    pub service_id_file: String,
}

impl Default for ConsulCfg {
    fn default() -> Self {
        Self {
            addr: "http://localhost:8500".into(),
            service_name: "notification-service".into(),
            service_id_file: "/tmp/notification-service-id".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct EventBusCfg {
    pub url: String,
}

impl Default for EventBusCfg {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@localhost:5672/%2f".into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub server: ServerCfg,
    pub mysql: MysqlCfg,
    pub redis: RedisCfg,
    pub scheduler: SchedulerCfg,
    pub log: LogCfg,
    pub consul: ConsulCfg,
    #[serde(default)]
    pub event_bus: EventBusCfg,
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
