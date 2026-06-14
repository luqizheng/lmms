//! 性能基准测试
//! 
//! 运行: cargo bench --package common-http

use common_http::{CircuitBreaker, RateLimiter, RetryPolicy};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Benchmark {
    iterations: u64,
    total_time_ns: u64,
}

impl Benchmark {
    fn new(iterations: u64) -> Self {
        Self {
            iterations,
            total_time_ns: 0,
        }
    }

    fn run<F>(&mut self, mut f: F)
    where
        F: FnMut() -> (),
    {
        let start = Instant::now();
        for _ in 0..self.iterations {
            f();
        }
        self.total_time_ns = start.elapsed().as_nanos() as u64;
    }

    fn ops_per_second(&self) -> f64 {
        let total_secs = self.total_time_ns as f64 / 1_000_000_000.0;
        self.iterations as f64 / total_secs
    }

    fn ns_per_op(&self) -> f64 {
        self.total_time_ns as f64 / self.iterations as f64
    }
}

#[cfg(test)]
mod circuit_breaker_bench {
    use super::*;

    #[test]
    fn bench_circuit_breaker_record_success() {
        let cb = CircuitBreaker::new(100, Duration::from_secs(60));
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            cb.record_success();
        });
        
        println!("CircuitBreaker::record_success:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }

    #[test]
    fn bench_circuit_breaker_record_failure() {
        let cb = CircuitBreaker::new(100, Duration::from_secs(60));
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            cb.record_failure();
        });
        
        println!("CircuitBreaker::record_failure:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }

    #[test]
    fn bench_circuit_breaker_is_available() {
        let cb = CircuitBreaker::new(100, Duration::from_secs(60));
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = cb.is_available();
        });
        
        println!("CircuitBreaker::is_available:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }

    #[test]
    fn bench_circuit_breaker_state() {
        let cb = CircuitBreaker::new(100, Duration::from_secs(60));
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = cb.state();
        });
        
        println!("CircuitBreaker::state:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }
}

#[cfg(test)]
mod rate_limiter_bench {
    use super::*;

    #[test]
    fn bench_rate_limiter_try_acquire() {
        let limiter = RateLimiter::new(10_000, 1000);
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = limiter.try_acquire();
        });
        
        println!("RateLimiter::try_acquire:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }

    #[test]
    fn bench_rate_limiter_available_tokens() {
        let limiter = RateLimiter::new(10_000, 1000);
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = limiter.available_tokens();
        });
        
        println!("RateLimiter::available_tokens:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }
}

#[cfg(test)]
mod retry_policy_bench {
    use super::*;

    #[test]
    fn bench_retry_delay_calculation() {
        let policy = RetryPolicy::new(3);
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = policy.delay_for(1);
            let _ = policy.delay_for(2);
            let _ = policy.delay_for(3);
        });
        
        println!("RetryPolicy::delay_for:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }

    #[test]
    fn bench_retry_should_retry() {
        let policy = RetryPolicy::new(3);
        let mut bench = Benchmark::new(1_000_000);
        
        bench.run(|| {
            let _ = policy.should_retry(1, "connection timeout");
            let _ = policy.should_retry(1, "400 Bad Request");
        });
        
        println!("RetryPolicy::should_retry:");
        println!("  ops/s: {:.2}", bench.ops_per_second());
        println!("  ns/op: {:.2}", bench.ns_per_op());
    }
}
