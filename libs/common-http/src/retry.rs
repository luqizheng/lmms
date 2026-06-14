use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            jitter: 0.1,
        }
    }
}

impl RetryPolicy {
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            ..Default::default()
        }
    }

    pub fn base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    pub fn jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter;
        self
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        let exp_delay = self.base_delay * 2u32.pow(attempt - 1);
        let capped_delay = exp_delay.min(self.max_delay);
        
        let jitter_range = (capped_delay.as_millis() as f64 * self.jitter) as i64;
        let jitter_ns = if jitter_range > 0 {
            let jitter = rand_u64() % jitter_range as u64;
            Duration::from_nanos(jitter)
        } else {
            Duration::ZERO
        };

        capped_delay + jitter_ns
    }

    pub fn should_retry(&self, attempt: u32, error: &str) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }

        let retriable = error.contains("timeout")
            || error.contains("connection")
            || error.contains("502")
            || error.contains("503")
            || error.contains("429");

        retriable
    }

    pub async fn execute<F, Fut, T>(&self, mut f: F) -> Result<T, String>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if !self.should_retry(attempt, &e) {
                        return Err(e);
                    }

                    if attempt >= self.max_attempts {
                        return Err(e);
                    }

                    let delay = self.delay_for(attempt);
                    tracing::warn!(
                        "Retry attempt {}/{} after {:?} due to: {}",
                        attempt, self.max_attempts, delay, e
                    );
                    sleep(delay).await;
                }
            }
        }
    }
}

fn rand_u64() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_delay_exponential() {
        let policy = RetryPolicy::default();
        
        assert_eq!(policy.delay_for(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for(2), Duration::from_millis(400));
        assert_eq!(policy.delay_for(3), Duration::from_millis(800));
    }

    #[test]
    fn test_retry_delay_capped() {
        let policy = RetryPolicy::default().max_delay(Duration::from_millis(100));
        
        assert_eq!(policy.delay_for(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for(2), Duration::from_millis(100));
    }

    #[test]
    fn test_should_retry() {
        let policy = RetryPolicy::new(3);
        
        assert!(policy.should_retry(1, "connection timeout"));
        assert!(policy.should_retry(1, "502 Bad Gateway"));
        assert!(!policy.should_retry(1, "400 Bad Request"));
        assert!(!policy.should_retry(3, "error"));
    }
}
