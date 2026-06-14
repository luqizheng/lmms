pub mod axum;
pub mod benchmarks;
pub mod client;
pub mod config;
pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;
pub mod service_discovery;

pub use axum::{CircuitBreakerMiddleware, DefaultMiddlewareConfig, RateLimitMiddleware};
pub use client::HttpClient;
pub use config::HttpDefaultsConfig;
pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use rate_limit::RateLimiter;
pub use retry::RetryPolicy;
pub use service_discovery::ConsulResolver;
