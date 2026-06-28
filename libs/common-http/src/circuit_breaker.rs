use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

// 进程级单调时钟锚点（仅供 breaker 跨请求共享的 opened_at 使用；
// overall deadline 用 call 栈上的 Instant，不走这里 —— 决策 8C）
fn anchor() -> Instant {
    static ANCHOR: OnceLock<Instant> = OnceLock::new();
    *ANCHOR.get_or_init(Instant::now)
}
fn now_ms() -> u64 {
    anchor().elapsed().as_millis() as u64
}

/// 三态熔断器。
///
/// Closed --failure>=threshold--> Open(记 opened_at_ms)
/// Open --now-opened>=open_timeout--> HalfOpen(清计数)
/// HalfOpen --success>=half_open_max--> Closed
/// HalfOpen --任一 failure--> Open
/// HalfOpen 放行用 fetch_add 原子占额，至多 half_open_max_calls 次（决策 2A）
#[derive(Debug)]
pub struct CircuitBreaker {
    state: AtomicU8, // 0=Closed,1=Open,2=HalfOpen
    failure_count: AtomicU64,
    success_count: AtomicU64,
    half_open_calls: AtomicU64,
    opened_at_ms: AtomicU64,
    failure_threshold: u64,
    open_timeout_ms: u64,
    half_open_max_calls: u64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, open_timeout: Duration, half_open_max_calls: u32) -> Self {
        Self {
            state: AtomicU8::new(0),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            half_open_calls: AtomicU64::new(0),
            opened_at_ms: AtomicU64::new(0),
            failure_threshold,
            open_timeout_ms: open_timeout.as_millis() as u64,
            half_open_max_calls: half_open_max_calls as u64,
        }
    }

    pub fn state(&self) -> CircuitState {
        match self.state.load(Ordering::SeqCst) {
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed,
        }
    }

    fn to_half_open(&self) {
        self.state.store(2, Ordering::SeqCst);
        self.half_open_calls.store(0, Ordering::SeqCst);
        self.success_count.store(0, Ordering::SeqCst);
    }

    pub fn is_available(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let opened = self.opened_at_ms.load(Ordering::SeqCst);
                if now_ms().saturating_sub(opened) >= self.open_timeout_ms {
                    self.to_half_open();
                    // 转入 HalfOpen 后，本次调用也走占额逻辑
                    self.half_open_calls.fetch_add(1, Ordering::SeqCst) < self.half_open_max_calls
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // 决策 2A：原子占额，超额拒绝
                self.half_open_calls.fetch_add(1, Ordering::SeqCst) < self.half_open_max_calls
            }
        }
    }

    pub fn record_success(&self) {
        match self.state() {
            CircuitState::Closed => {
                self.failure_count.store(0, Ordering::SeqCst);
            }
            CircuitState::HalfOpen => {
                let s = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if s >= self.half_open_max_calls {
                    self.state.store(0, Ordering::SeqCst);
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Open => {}
        }
    }

    pub fn record_failure(&self) {
        match self.state() {
            CircuitState::Closed => {
                let f = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if f >= self.failure_threshold {
                    self.opened_at_ms.store(now_ms(), Ordering::SeqCst);
                    self.state.store(1, Ordering::SeqCst);
                }
            }
            CircuitState::HalfOpen => {
                self.opened_at_ms.store(now_ms(), Ordering::SeqCst);
                self.state.store(1, Ordering::SeqCst);
            }
            CircuitState::Open => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn cb() -> CircuitBreaker {
        CircuitBreaker::new(3, Duration::from_millis(100), 2)
    }

    #[test]
    fn closed_by_default() {
        let b = cb();
        assert_eq!(b.state(), CircuitState::Closed);
        assert!(b.is_available());
    }

    #[test]
    fn opens_after_threshold() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), CircuitState::Open);
        assert!(!b.is_available());
    }

    #[test]
    fn success_resets_failures_in_closed() {
        let b = cb();
        b.record_failure();
        b.record_failure();
        b.record_success();
        b.record_failure();
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn open_transitions_to_halfopen_after_timeout() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), CircuitState::Open);
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available()); // 计时到 → 转 HalfOpen 放行
        assert_eq!(b.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn halfopen_success_quota_closes() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available());
        b.record_success();
        b.record_success(); // 达 half_open_max_calls=2
        assert_eq!(b.state(), CircuitState::Closed);
    }

    #[test]
    fn halfopen_failure_reopens() {
        let b = cb();
        for _ in 0..3 {
            b.record_failure();
        }
        std::thread::sleep(Duration::from_millis(120));
        assert!(b.is_available());
        b.record_failure();
        assert_eq!(b.state(), CircuitState::Open);
    }

    // 决策 10A：并发占额，放行数必须 == half_open_max_calls
    #[tokio::test(flavor = "multi_thread", worker_threads = 8)]
    async fn halfopen_concurrent_admission_capped() {
        let b = Arc::new(CircuitBreaker::new(3, Duration::from_millis(50), 5));
        for _ in 0..3 {
            b.record_failure();
        }
        tokio::time::sleep(Duration::from_millis(70)).await;
        let mut handles = Vec::new();
        for _ in 0..100 {
            let bb = b.clone();
            handles.push(tokio::spawn(async move { bb.is_available() }));
        }
        let mut admitted = 0;
        for h in handles {
            if h.await.unwrap() {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 5, "admitted {admitted}, expected exactly half_open_max_calls");
    }
}