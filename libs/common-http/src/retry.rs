use crate::resilient::ResilientError;
use rand::Rng;
use reqwest::{Method, StatusCode};
use std::time::Duration;

/// 幂等方法才允许重试。
pub fn is_idempotent(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS
    )
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter: f64,
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration, jitter: f64) -> Self {
        Self { max_attempts, base_delay, max_delay, jitter }
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        let factor = 2u32.saturating_pow(attempt.saturating_sub(1));
        let capped = self.base_delay.saturating_mul(factor).min(self.max_delay);
        let extra_ms = (capped.as_millis() as f64 * self.jitter) as u64;
        let jitter_ms = if extra_ms > 0 {
            rand::thread_rng().gen_range(0..=extra_ms)
        } else {
            0
        };
        capped + Duration::from_millis(jitter_ms)
    }

    pub fn should_retry_err(&self, attempt: u32, err: &ResilientError, method: &Method) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        match err {
            ResilientError::CircuitOpen => false,
            ResilientError::Timeout | ResilientError::Connect(_) => is_idempotent(method),
        }
    }

    pub fn should_retry_status(&self, attempt: u32, status: StatusCode, method: &Method) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        let retriable = matches!(status.as_u16(), 502 | 503 | 429);
        retriable && is_idempotent(method)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> RetryPolicy {
        RetryPolicy::new(3, Duration::from_millis(200), Duration::from_secs(10), 0.1)
    }

    #[test]
    fn exponential_backoff_within_jitter_bound() {
        let p = p();
        let d1 = p.delay_for(1).as_millis() as u64;
        assert!((200..=220).contains(&d1), "d1={d1}");
        let d2 = p.delay_for(2).as_millis() as u64;
        assert!((400..=440).contains(&d2), "d2={d2}");
    }

    #[test]
    fn backoff_capped_at_max() {
        let p = RetryPolicy::new(5, Duration::from_secs(8), Duration::from_secs(10), 0.0);
        assert_eq!(p.delay_for(3), Duration::from_secs(10)); // 8*4=32 capped to 10
    }

    #[test]
    fn circuit_open_never_retries() {
        let p = p();
        assert!(!p.should_retry_err(1, &ResilientError::CircuitOpen, &Method::GET));
    }

    #[test]
    fn timeout_retry_only_idempotent() {
        let p = p();
        assert!(p.should_retry_err(1, &ResilientError::Timeout, &Method::GET));
        assert!(!p.should_retry_err(1, &ResilientError::Timeout, &Method::POST));
    }

    #[test]
    fn status_retry_matrix() {
        let p = p();
        assert!(p.should_retry_status(1, StatusCode::BAD_GATEWAY, &Method::GET)); // 502
        assert!(p.should_retry_status(1, StatusCode::SERVICE_UNAVAILABLE, &Method::PUT)); // 503
        assert!(p.should_retry_status(1, StatusCode::TOO_MANY_REQUESTS, &Method::GET)); // 429
        assert!(!p.should_retry_status(1, StatusCode::BAD_GATEWAY, &Method::POST)); // 非幂等
        assert!(!p.should_retry_status(1, StatusCode::INTERNAL_SERVER_ERROR, &Method::GET)); // 500
        assert!(!p.should_retry_status(1, StatusCode::OK, &Method::GET)); // 2xx
        assert!(!p.should_retry_status(1, StatusCode::BAD_REQUEST, &Method::GET)); // 4xx
    }

    #[test]
    fn exhausted_attempts_no_retry() {
        let p = p();
        assert!(!p.should_retry_err(3, &ResilientError::Timeout, &Method::GET)); // attempt==max
    }
}