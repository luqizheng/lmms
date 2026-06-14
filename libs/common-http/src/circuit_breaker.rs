use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub struct CircuitBreaker {
    state: Arc<std::sync::atomic::AtomicU8>,
    failure_count: Arc<AtomicU64>,
    success_count: Arc<AtomicU64>,
    last_failure: Arc<std::sync::atomic::AtomicU64>,
    threshold: u64,
    timeout: Duration,
    half_open_max_calls: u32,
    half_open_calls: Arc<AtomicU64>,
}

impl CircuitBreaker {
    pub fn new(threshold: u64, timeout: Duration) -> Self {
        Self {
            state: Arc::new(AtomicU8::new(CircuitState::Closed as u8)),
            failure_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            last_failure: Arc::new(AtomicU64::new(0)),
            threshold,
            timeout,
            half_open_max_calls: 3,
            half_open_calls: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn threshold(mut self, t: u64) -> Self {
        self.threshold = t;
        self
    }

    pub fn half_open_max_calls(mut self, n: u32) -> Self {
        self.half_open_max_calls = n;
        self
    }

    pub fn state(&self) -> CircuitState {
        let state_val = self.state.load(Ordering::SeqCst);
        match state_val {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    pub fn is_available(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                self.half_open_calls.load(Ordering::SeqCst) < self.half_open_max_calls as u64
            }
            CircuitState::Open => {
                let last = self.last_failure.load(Ordering::SeqCst);
                let elapsed = Instant::now()
                    .checked_duration_since(Instant::now() - Duration::from_secs(last))
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                
                if elapsed >= self.timeout.as_secs() {
                    self.state.store(CircuitState::HalfOpen as u8, Ordering::SeqCst);
                    self.half_open_calls.store(0, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn record_success(&self) {
        let current_state = self.state();
        
        match current_state {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let success = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if success >= self.half_open_max_calls as u64 {
                    self.state.store(CircuitState::Closed as u8, Ordering::SeqCst);
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                    tracing::info!("Circuit breaker closed");
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        let current_state = self.state();
        
        match current_state {
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                self.last_failure.store(
                    Instant::now().elapsed().as_secs(),
                    Ordering::SeqCst,
                );
                
                if failures >= self.threshold {
                    self.state.store(CircuitState::Open as u8, Ordering::SeqCst);
                    tracing::warn!(
                        "Circuit breaker opened after {} failures",
                        failures
                    );
                }
            }
            CircuitState::HalfOpen => {
                self.state.store(CircuitState::Open as u8, Ordering::SeqCst);
                self.last_failure.store(
                    Instant::now().elapsed().as_secs(),
                    Ordering::SeqCst,
                );
                tracing::warn!("Circuit breaker reopened from half-open state");
            }
            CircuitState::Open => {}
        }
    }

    pub fn reset(&self) {
        self.state.store(CircuitState::Closed as u8, Ordering::SeqCst);
        self.failure_count.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_circuit_closed_by_default() {
        let cb = CircuitBreaker::new(5, Duration::from_secs(60));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_available());
    }

    #[tokio::test]
    async fn test_circuit_opens_after_threshold() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(60));
        
        for _ in 0..3 {
            cb.record_failure();
        }
        
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.is_available());
    }

    #[tokio::test]
    async fn test_success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(60));
        
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
