#[cfg(feature = "axum")]
mod axum_middleware {
    use axum::{
        body::Body,
        extract::Request,
        http::{Response, StatusCode},
        middleware::Next,
        response::IntoResponse,
    };
    use std::sync::Arc;
    use std::time::Duration;

    use crate::circuit_breaker::CircuitBreaker;
    use crate::rate_limit::RateLimiter;

    /// 熔断中间件
    pub struct CircuitBreakerMiddleware {
        breaker: Arc<CircuitBreaker>,
    }

    impl CircuitBreakerMiddleware {
        pub fn new(threshold: u64, timeout: Duration) -> Self {
            Self {
                breaker: Arc::new(CircuitBreaker::new(threshold, timeout)),
            }
        }

        pub fn circuit_breaker(&self) -> Arc<CircuitBreaker> {
            self.breaker.clone()
        }

        pub async fn handle(&self, request: Request, next: Next) -> Result<Response<Body>, StatusCode> {
            if !self.breaker.is_available() {
                tracing::warn!("Circuit breaker open, rejecting request");
                return Err(StatusCode::SERVICE_UNAVAILABLE);
            }

            let response = next.run(request).await;
            
            if response.status().is_server_error() {
                self.breaker.record_failure();
            } else {
                self.breaker.record_success();
            }

            Ok(response)
        }
    }

    /// 限流中间件
    pub struct RateLimitMiddleware {
        limiter: Arc<RateLimiter>,
    }

    impl RateLimitMiddleware {
        pub fn new(capacity: u64, requests_per_second: u64) -> Self {
            Self {
                limiter: Arc::new(RateLimiter::new(capacity, requests_per_second)),
            }
        }

        pub fn limiter(&self) -> Arc<RateLimiter> {
            self.limiter.clone()
        }

        pub async fn handle(&self, _request: Request, next: Next) -> Result<Response<Body>, StatusCode> {
            if !self.limiter.try_acquire() {
                tracing::warn!("Rate limit exceeded");
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }

            Ok(next.run(_request).await)
        }
    }

    /// 默认配置
    pub struct DefaultMiddlewareConfig {
        pub circuit_breaker_threshold: u64,
        pub circuit_breaker_timeout: Duration,
        pub rate_limit_capacity: u64,
        pub rate_limit_rps: u64,
    }

    impl Default for DefaultMiddlewareConfig {
        fn default() -> Self {
            Self {
                circuit_breaker_threshold: 5,
                circuit_breaker_timeout: Duration::from_secs(60),
                rate_limit_capacity: 100,
                rate_limit_rps: 50,
            }
        }
    }

    impl DefaultMiddlewareConfig {
        pub fn production() -> Self {
            Self {
                circuit_breaker_threshold: 10,
                circuit_breaker_timeout: Duration::from_secs(30),
                rate_limit_capacity: 1000,
                rate_limit_rps: 500,
            }
        }

        pub fn circuit_breaker(&self) -> CircuitBreakerMiddleware {
            CircuitBreakerMiddleware::new(
                self.circuit_breaker_threshold,
                self.circuit_breaker_timeout,
            )
        }

        pub fn rate_limiter(&self) -> RateLimitMiddleware {
            RateLimitMiddleware::new(
                self.rate_limit_capacity,
                self.rate_limit_rps,
            )
        }
    }
}

#[cfg(feature = "axum")]
pub use axum_middleware::*;

#[cfg(not(feature = "axum"))]
mod no_axum {
    use std::time::Duration;

    pub struct DefaultMiddlewareConfig;

    impl Default for DefaultMiddlewareConfig {
        fn default() -> Self {
            Self
        }
    }

    impl DefaultMiddlewareConfig {
        pub fn production() -> Self {
            Self
        }
    }
}

#[cfg(not(feature = "axum"))]
pub use no_axum::*;

#[cfg(feature = "axum")]
#[cfg(test)]
mod tests {
    use super::axum_middleware::*;
    use axum::{body::Body, extract::Request, middleware::Next};

    #[test]
    fn test_default_config() {
        let config = DefaultMiddlewareConfig::default();
        assert_eq!(config.circuit_breaker_threshold, 5);
        assert_eq!(config.rate_limit_capacity, 100);
    }

    #[test]
    fn test_production_config() {
        let config = DefaultMiddlewareConfig::production();
        assert_eq!(config.circuit_breaker_threshold, 10);
        assert_eq!(config.rate_limit_capacity, 1000);
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks() {
        let limiter = RateLimitMiddleware::new(1, 1);
        
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let next = Next::default();
        
        let result = limiter.handle(req, next).await;
        assert!(result.is_ok());
        
        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();
        let result = limiter.handle(req, next).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), axum::http::StatusCode::TOO_MANY_REQUESTS);
    }
}
