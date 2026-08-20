use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct EventBusConfig {
    pub url: String,
    pub exchange: String,
    #[serde(default = "default_publish_confirms")]
    pub publish_confirms: bool,
}

fn default_publish_confirms() -> bool {
    true
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@localhost:5672/%2f".to_string(),
            exchange: "events".to_string(),
            publish_confirms: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutboxPublisherConfig {
    pub poll_interval_ms: u64,
    pub batch_size: u32,
    pub backlog_warn_threshold: u64,
}

impl Default for OutboxPublisherConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1000,
            batch_size: 100,
            backlog_warn_threshold: 10_000,
        }
    }
}
