use crate::circuit_breaker::CircuitBreaker;
use crate::retry::RetryPolicy;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ResilienceConfig {
    pub timeout_ms: u64,
    pub overall_timeout_ms: u64,
    pub retry_max_attempts: u32,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_jitter: f64,
    pub breaker_failure_threshold: u64,
    pub breaker_open_timeout_ms: u64,
    pub breaker_half_open_max_calls: u32,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 3000,
            overall_timeout_ms: 8000,
            retry_max_attempts: 3,
            retry_base_delay_ms: 200,
            retry_max_delay_ms: 10000,
            retry_jitter: 0.1,
            breaker_failure_threshold: 10,
            breaker_open_timeout_ms: 30000,
            breaker_half_open_max_calls: 3,
        }
    }
}

impl ResilienceConfig {
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
    pub fn overall_timeout(&self) -> Duration {
        Duration::from_millis(self.overall_timeout_ms)
    }
    pub fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::new(
            self.retry_max_attempts,
            Duration::from_millis(self.retry_base_delay_ms),
            Duration::from_millis(self.retry_max_delay_ms),
            self.retry_jitter,
        )
    }
    pub fn new_breaker(&self) -> CircuitBreaker {
        CircuitBreaker::new(
            self.breaker_failure_threshold,
            Duration::from_millis(self.breaker_open_timeout_ms),
            self.breaker_half_open_max_calls,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = ResilienceConfig::default();
        assert_eq!(c.timeout_ms, 3000);
        assert_eq!(c.overall_timeout_ms, 8000);
        assert_eq!(c.retry_max_attempts, 3);
        assert_eq!(c.breaker_failure_threshold, 10);
    }

    #[test]
    fn partial_json_fills_defaults() {
        let c: ResilienceConfig = serde_json::from_str(r#"{"timeout_ms":1500}"#).unwrap();
        assert_eq!(c.timeout_ms, 1500);
        assert_eq!(c.overall_timeout_ms, 8000);
    }

    #[test]
    fn builds_policy_and_breaker() {
        let c = ResilienceConfig::default();
        assert_eq!(c.retry_policy().max_attempts, 3);
        let _ = c.new_breaker();
        assert_eq!(c.timeout(), Duration::from_millis(3000));
    }
}
