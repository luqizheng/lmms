use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

pub struct RateLimiter {
    tokens: Arc<AtomicU64>,
    capacity: u64,
    refill_rate: u64,
    last_refill: Arc<AtomicU64>,
}

impl RateLimiter {
    pub fn new(capacity: u64, requests_per_second: u64) -> Self {
        Self {
            tokens: Arc::new(AtomicU64::new(capacity)),
            capacity,
            refill_rate: requests_per_second,
            last_refill: Arc::new(AtomicU64::new(current_millis())),
        }
    }

    pub fn capacity(mut self, c: u64) -> Self {
        self.capacity = c;
        self
    }

    pub fn refill_rate(mut self, r: u64) -> Self {
        self.refill_rate = r;
        self
    }

    fn refill(&self) {
        let now = current_millis();
        let last = self.last_refill.load(Ordering::SeqCst);
        let elapsed = now.saturating_sub(last);
        
        if elapsed > 0 {
            let tokens_to_add = (elapsed / 1000) * self.refill_rate;
            let current = self.tokens.load(Ordering::SeqCst);
            let new_tokens = (current + tokens_to_add).min(self.capacity);
            self.tokens.store(new_tokens, Ordering::SeqCst);
            self.last_refill.store(now, Ordering::SeqCst);
        }
    }

    pub fn try_acquire(&self) -> bool {
        self.refill();
        
        let current = self.tokens.load(Ordering::SeqCst);
        if current > 0 {
            self.tokens.store(current - 1, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    pub fn available_tokens(&self) -> u64 {
        self.refill();
        self.tokens.load(Ordering::SeqCst)
    }
}

fn current_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_up_to_capacity() {
        let limiter = RateLimiter::new(5, 1);
        
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_refill() {
        let limiter = RateLimiter::new(2, 10);
        
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
        
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        assert!(limiter.try_acquire());
    }
}
