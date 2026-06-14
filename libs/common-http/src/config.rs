use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct HttpDefaultsConfig {
    #[serde(default = "default_circuit_breaker_threshold")]
    pub circuit_breaker_threshold: u64,
    #[serde(default = "default_circuit_breaker_timeout_secs")]
    pub circuit_breaker_timeout_secs: u64,
    #[serde(default = "default_rate_limit_capacity")]
    pub rate_limit_capacity: u64,
    #[serde(default = "default_rate_limit_rps")]
    pub rate_limit_rps: u64,
    #[serde(default = "default_retry_max_attempts")]
    pub retry_max_attempts: u32,
    #[serde(default = "default_retry_base_delay_ms")]
    pub retry_base_delay_ms: u64,
}

fn default_circuit_breaker_threshold() -> u64 { 5 }
fn default_circuit_breaker_timeout_secs() -> u64 { 60 }
fn default_rate_limit_capacity() -> u64 { 100 }
fn default_rate_limit_rps() -> u64 { 50 }
fn default_retry_max_attempts() -> u32 { 3 }
fn default_retry_base_delay_ms() -> u64 { 200 }

impl Default for HttpDefaultsConfig {
    fn default() -> Self {
        Self {
            circuit_breaker_threshold: default_circuit_breaker_threshold(),
            circuit_breaker_timeout_secs: default_circuit_breaker_timeout_secs(),
            rate_limit_capacity: default_rate_limit_capacity(),
            rate_limit_rps: default_rate_limit_rps(),
            retry_max_attempts: default_retry_max_attempts(),
            retry_base_delay_ms: default_retry_base_delay_ms(),
        }
    }
}

impl HttpDefaultsConfig {
    pub fn production() -> Self {
        Self {
            circuit_breaker_threshold: 10,
            circuit_breaker_timeout_secs: 30,
            rate_limit_capacity: 1000,
            rate_limit_rps: 500,
            retry_max_attempts: 3,
            retry_base_delay_ms: 200,
        }
    }

    pub fn development() -> Self {
        Self {
            circuit_breaker_threshold: 3,
            circuit_breaker_timeout_secs: 30,
            rate_limit_capacity: 50,
            rate_limit_rps: 20,
            retry_max_attempts: 2,
            retry_base_delay_ms: 100,
        }
    }
}
