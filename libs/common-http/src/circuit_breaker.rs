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

    // Open -> HalfOpen 跃迁由 is_available() 内的 CAS 直接完成，
// 不再单独抽出 helper（避免多个调用点各自非原子地清计数）。

    pub fn is_available(&self) -> bool {
        match self.state() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let opened = self.opened_at_ms.load(Ordering::SeqCst);
                if now_ms().saturating_sub(opened) < self.open_timeout_ms {
                    return false;
                }
                // 决策 review#1：Open -> HalfOpen 原子跃迁。
                // 旧实现先 store(2) 再 store(0) 再 fetch_add(1)，
                // 多个 worker 同时观察到 state=Open 时会各自从 0 占额，
                // 导致 half_open_max_calls 上限被突破。CAS 保证只
                // 一个 winner 重置计数并占第一个名额，loser 直接走
                // HalfOpen 占额分支看到 winner 已经 fetch_add 过的计数。
                match self
                    .state
                    .compare_exchange(1, 2, Ordering::SeqCst, Ordering::SeqCst)
                {
                    Ok(_) => {
                        self.half_open_calls.store(0, Ordering::SeqCst);
                        self.success_count.store(0, Ordering::SeqCst);
                        self.half_open_calls.fetch_add(1, Ordering::SeqCst)
                            < self.half_open_max_calls
                    }
                    Err(actual) => {
                        debug_assert!(
                            actual == 2,
                            "unexpected state during Open->HalfOpen race: {actual}"
                        );
                        self.half_open_calls.fetch_add(1, Ordering::SeqCst)
                            < self.half_open_max_calls
                    }
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

    // Phase 5-B review #1: Open -> HalfOpen transition race.
    // The previous test (above) only exercised HalfOpen -> HalfOpen admission:
    // only the first task observed state=Open with elapsed timeout, the others
    // arrived when state was already HalfOpen. This test forces ALL N threads
    // to simultaneously observe state=Open with elapsed timeout, by gating them
    // on an OS-level Barrier that releases them right after the open-timer
    // expires. Pre-fix bug: each thread independently called to_half_open()
    // then fetch_add(1) from a stale baseline, admitting many more than
    // half_open_max_calls. Post-fix: only the CAS winner resets counters and
    // claims a slot; losers fall through to the HalfOpen admission branch and
    // see the already-incremented counter.
    //
    // The race window is narrow (between the loser's state.load(SeqCst) and the
    // winner's state.store(2, SeqCst)). To make the test deterministic enough
    // to catch it, we run several rounds back-to-back and require that every
    // round admit exactly half_open_max_calls. On the buggy code the race
    // triggers on at least one round under realistic OS scheduling jitter.
    #[test]
    fn open_to_halfopen_transition_is_atomic() {
        use std::sync::Barrier;
        use std::thread;

        const THREADS: usize = 100;
        const ROUNDS: usize = 25;

        for round in 0..ROUNDS {
            let b = Arc::new(CircuitBreaker::new(3, Duration::from_millis(50), 5));
            for _ in 0..3 {
                b.record_failure();
            }
            assert_eq!(b.state(), CircuitState::Open);

            // Sleep just past the open-timer, keeping the race window open so
            // all threads can observe state=Open with elapsed timeout.
            std::thread::sleep(Duration::from_millis(60));

            let barrier = Arc::new(Barrier::new(THREADS));
            let mut handles = Vec::new();
            for _ in 0..THREADS {
                let bb = b.clone();
                let bar = barrier.clone();
                handles.push(thread::spawn(move || {
                    // Block until all threads are at the gate, then fire all
                    // calls at the same instant. Without the OS-level barrier,
                    // threads arrive at state.load() sequentially and only one
                    // sees Open.
                    bar.wait();
                    bb.is_available()
                }));
            }
            let mut admitted = 0;
            for h in handles {
                if h.join().unwrap() {
                    admitted += 1;
                }
            }
            assert_eq!(
                admitted, 5,
                "round {round}: race in Open->HalfOpen transition: admitted {admitted}, expected exactly half_open_max_calls"
            );
            assert_eq!(b.state(), CircuitState::HalfOpen);
        }
    }
}
